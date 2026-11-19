//! Draw shaders that are one shader up to their numbers, compiled once.
//!
//! Repeated parts (eight legs, five spikes, a left and a right claw) emit one shader per draw. Each used to be its
//! own module and pipeline: the same WGSL with different literals (radii, control points, colours, name IDs) and
//! table bases. Pipeline creation is most of a cold load (2026-11-15: 46 of 73 pipelines and 8 of 12 s were such
//! surfaces). Sharing moves differing numbers into rows. A draw picks its row from the instance index's high bits.
//!
//! The holes can be table bases, the per-draw detail slot, typed module constants, and some fragment literals.
//! The shader compiler folds and reassociates visible vertex literals. Moving those to f32 rows changed two
//! silhouette pixels in one of eight bench programs even though the numbers were identical. Kept literal, all
//! eight programs were byte-identical. Functions with control flow also keep their fragment literals: a flat
//! varying is nonuniform, and WGSL rejects derivatives under a branch on one.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use crate::{Error, files::to_json};

/// The inputs from an artefact that shader sharing reads, in producer order.
#[derive(Default)]
pub struct ShareInput {
    pub scene_wgsl: String,
    pub shaders: Vec<(String, String)>,
    pub draws: Vec<ShareDraw>,
    pub mixed_shaders: Vec<String>,
}

pub struct ShareDraw {
    pub shader: String,
    pub constants: Vec<(String, f64)>,
    pub instances: u32,
}

/// How a draw's shader is compiled and drawn after sharing.
pub struct Sharing<'a> {
    shaders: HashMap<&'a str, &'a str>,
    module: HashMap<&'a str, usize>,
    groups: Vec<(String, String)>,
    draws: Vec<SharedDraw>,
    /// Every group's rows, one float per hole. When empty, nothing is bound.
    pub rows: Vec<f32>,
}

pub struct SharedModule<'a> {
    pub key: &'a str,
    pub wgsl: &'a str,
}

pub struct SharedDraw {
    pub constants: Vec<(String, f64)>,
    pub instance_base: u32,
}

impl Sharing<'_> {
    /// The module key and WGSL, from the group if this shader was shared.
    pub fn module_of<'a>(&'a self, shader: &'a str) -> SharedModule<'a> {
        match self.module.get(shader) {
            Some(&index) => {
                let (key, wgsl) = &self.groups[index];
                SharedModule { key, wgsl }
            }
            None => SharedModule { key: shader, wgsl: self.shaders[shader] },
        }
    }

    /// Pipeline constants (any per-draw value removed), and what the host adds to every first instance.
    pub fn draw_of(&self, k: usize) -> &SharedDraw {
        &self.draws[k]
    }
}

/// The group 0 binding for the rows. PRELUDE_WGSL holds bindings 0 to 4.
pub const ROWS_BINDING: u32 = 5;
/// Instance bits for the copy. The row is above them.
const COPY_BITS: u32 = 16;
/// The override for a draw's level-of-detail slot is per draw, so it becomes a hole.
const PER_DRAW: &str = "LOFT_DETAIL_SLOT";
/// Larger u32 values would not survive the row's f32.
const EXACT: f64 = (1u32 << 24) as f64;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    F32,
    U32,
    I32,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::U32 => "u32",
            Self::I32 => "i32",
        }
    }
}

struct Tok<'a> {
    text: &'a str,
    code: bool,
    // TOKEN's [^] takes one UTF-16 unit. Keep both units in the key, but emit their source bytes once.
    unit: Option<u16>,
}

fn whitespace(c: char) -> bool {
    matches!(
        c, '\t'..='\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' |
        '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'
    )
}

fn word_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

fn ident(t: &str) -> bool {
    t.as_bytes().first().is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_')
}

fn word(t: &str) -> (&str, &str) {
    t.split_at(t.bytes().take_while(|&c| word_byte(c)).count())
}

fn digits(t: &str) -> usize {
    t.bytes().take_while(u8::is_ascii_digit).count()
}

/// Identifiers whose < opens a template list. Numbers inside are type sizes.
fn templated(t: &str) -> bool {
    matches!(t, "array" | "ptr" | "atomic" | "binding_array" | "var")
        || matches!(t.as_bytes(), [b'v', b'e', b'c', b'2'..=b'4'] | [b'm', b'a', b't', b'2'..=b'4', b'x', b'2'..=b'4'])
        || t.strip_prefix("texture_").is_some_and(|suffix| !suffix.is_empty() && suffix.bytes().all(word_byte))
}

/// Numbers the shader compiler rewrites when constant (division, pow and smoothstep) stay literal.
/// This is a precaution. Once vertex literals stayed, none of the bench programs needed it.
fn folds(prev: &str, prev2: &str, call: Option<&(&str, usize)>) -> bool {
    if prev == "/" || (prev == "-" && prev2 == "/") {
        return true;
    }
    call.is_some_and(|&(name, arg)| (name == "pow" && arg == 1) || (name == "smoothstep" && arg < 2))
}

fn exponent(t: &str) -> usize {
    if !t.starts_with(['e', 'E']) {
        return 0;
    }
    let n = if t[1..].starts_with(['+', '-']) { 2 } else { 1 };
    let count = digits(&t[n..]);
    if count == 0 { 0 } else { n + count }
}

fn tokenize(mut text: &str) -> Vec<Tok<'_>> {
    // TOKEN = /\s+|\/\/[^\n]*|\/\*[\s\S]*?\*\/|[A-Za-z_][A-Za-z0-9_]*|0[xX][0-9a-fA-F]+[iu]?|(?:\d+\.\d*|\.\d+)(?:[eE][+-]?\d+)?f?|\d+[eE][+-]?\d+f?|\d+[iuf]?|[^]/gy
    let mut out = Vec::new();
    while let Some(c) = text.chars().next() {
        let space = text.len() - text.trim_start_matches(whitespace).len();
        let line = text.strip_prefix("//").map(|s| 2 + s.find('\n').unwrap_or(s.len()));
        let block = text.strip_prefix("/*").and_then(|s| s.find("*/").map(|end| 4 + end));
        let hex = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")).map_or(0, |s| {
            let n = s.bytes().take_while(u8::is_ascii_hexdigit).count();
            if n == 0 { 0 } else { 2 + n + usize::from(s[n..].starts_with(['i', 'u'])) }
        });
        let emit = if space > 0 {
            (space, false)
        } else if let Some(n) = line.or(block) {
            (n, false)
        } else if ident(text) {
            (word(text).0.len(), true)
        } else if hex > 0 {
            (hex, true)
        } else {
            let n = digits(text);
            let decimal = text[n..].strip_prefix('.').map_or(0, |s| {
                let rest = digits(s);
                if n + rest > 0 { n + 1 + rest } else { 0 }
            });
            let exp = exponent(&text[decimal.max(n)..]);
            if decimal > 0 || (n > 0 && exp > 0) {
                let end = decimal.max(n) + exp;
                (end + usize::from(text[end..].starts_with('f')), true)
            } else if n > 0 {
                (n + usize::from(text[n..].starts_with(['i', 'u', 'f'])), true)
            } else if c.len_utf16() == 2 {
                let mut units = [0; 2];
                c.encode_utf16(&mut units);
                out.push(Tok { text: &text[..c.len_utf8()], code: true, unit: Some(units[0]) });
                out.push(Tok { text: "", code: true, unit: Some(units[1]) });
                text = &text[c.len_utf8()..];
                continue;
            } else {
                (c.len_utf8(), true)
            }
        };
        let (token, rest) = text.split_at(emit.0);
        out.push(Tok { text: token, code: emit.1, unit: None });
        text = rest;
    }
    out
}

fn number_kind(t: &str) -> Option<Kind> {
    if t.ends_with('u') && !t.starts_with("0x") {
        return Some(Kind::U32);
    }
    if t.ends_with('i') && !t.starts_with("0x") {
        return Some(Kind::I32);
    }
    let n = digits(t);
    if t.ends_with('f')
        || (n > 0 && t[n..].starts_with(['.', 'e', 'E']))
        || t.strip_prefix('.').is_some_and(|s| digits(s) > 0)
    {
        return Some(Kind::F32);
    }
    None // An abstract int gets its type from context.
}

fn number_value(t: &str, kind: Kind) -> f64 {
    match kind {
        Kind::F32 => {
            let t = t.strip_suffix('f').unwrap_or(t);
            let value = match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                Some(hex) => hex_number(hex),
                None if t.is_empty() => 0.0,
                None if t == "Infinity" || t.starts_with(|c: char| c.is_ascii_digit() || c == '.') => {
                    t.parse::<f64>().unwrap_or(f64::NAN)
                }
                None => f64::NAN,
            };
            // JavaScript Number rounds to f64 before Math.fround rounds again.
            f64::from(value as f32)
        }
        Kind::U32 | Kind::I32 => t[..digits(t)].parse::<f64>().unwrap_or(f64::NAN),
    }
}

fn hex_number(hex: &str) -> f64 {
    // Number accepts hexadecimal integer syntax. Keep a guard bit and the nonzero tail for ties to even.
    if hex.is_empty() || !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
        return f64::NAN;
    }
    let hex = hex.trim_start_matches('0');
    if hex.is_empty() {
        return 0.0;
    }
    let end = hex.len().min(15);
    let high = u64::from_str_radix(&hex[..end], 16).unwrap();
    let bits = high.ilog2() + 1;
    if hex.len() > end + 256 {
        return f64::INFINITY;
    }
    let exp = (bits + (hex.len() - end) as u32 * 4).saturating_sub(53);
    if bits <= 53 {
        return high as f64;
    }
    let shift = bits - 53;
    let midpoint = 1u64 << (shift - 1);
    let tail = high & ((1 << shift) - 1);
    let top = high >> shift;
    let round_up =
        tail > midpoint || (tail == midpoint && (!top.is_multiple_of(2) || hex[end..].bytes().any(|b| b != b'0')));
    (top + u64::from(round_up)) as f64 * 2f64.powi(exp as i32)
}

/// A scene table accessor like fn t_name(i: u32) -> vec4<f32> { return BUFFER[BASEu + i]; }.
struct Accessor<'a> {
    buffer: &'a str,
    base: f64,
}

fn accessors(scene_wgsl: &str) -> HashMap<&str, Accessor<'_>> {
    scene_wgsl
        .match_indices("fn ")
        .filter_map(|(at, head)| {
            let (name, rest) = word(&scene_wgsl[at + head.len()..]);
            if !name.starts_with("t_") || name.len() == 2 {
                return None;
            }
            let rest = rest.strip_prefix("(i: u32) -> vec4<f32> { return ")?;
            let (buffer, rest) = word(rest);
            if !buffer.starts_with("loft_") || buffer.len() == 5 {
                return None;
            }
            let rest = rest.strip_prefix('[')?;
            let (base, rest) = rest.split_at(digits(rest));
            rest.strip_prefix("u + i]; }")?;
            Some((name, Accessor { buffer, base: base.parse().ok()? }))
        })
        .collect()
}

/// The templates a shareable shader must have (draw/surface.ts, draw/relief.ts). plumb edits these lines.
fn vs_of(wgsl: &str) -> Option<(&str, &str, &str)> {
    wgsl.match_indices("@vertex fn vs(").find_map(|(at, head)| {
        let rest = &wgsl[at + head.len()..];
        let line = rest.split_once('\n').map_or(rest, |(line, _)| line);
        line.rmatch_indices(") -> ").find_map(|(end, arrow)| {
            let tail = &rest[end + arrow.len()..];
            let (out, after) = word(tail);
            if out.is_empty() || !after.starts_with(" {") {
                return None;
            }
            let params = &rest[..end];
            Some((&wgsl[at..at + head.len() + end + arrow.len() + out.len() + 2], params, out))
        })
    })
}

fn instance(params: &str) -> Option<(&str, &str)> {
    params.match_indices("@builtin(instance_index) ").find_map(|(at, head)| {
        let (name, rest) = word(&params[at + head.len()..]);
        if name.is_empty() || !rest.starts_with(": u32") {
            return None;
        }
        Some((&params[at..at + head.len() + name.len() + ": u32".len()], name))
    })
}

fn fs_of<'a>(wgsl: &'a str, out: &str) -> Option<(&'a str, &'a str)> {
    wgsl.match_indices("@fragment fn fs(").find_map(|(at, head)| {
        let (arg, rest) = word(&wgsl[at + head.len()..]);
        let rest = rest.strip_prefix(": ")?.strip_prefix(out)?;
        if arg.is_empty() || rest.as_bytes().first().is_some_and(|&b| word_byte(b)) {
            return None;
        }
        let end = wgsl.len() - rest.len() + rest.find('{')? + 1;
        Some((&wgsl[at..end], arg))
    })
}

/// A place in a shader that may hold a different number in another member of its group.
enum Slot<'a> {
    Literal { at: usize, kind: Kind },
    Base { at: usize, buffer: &'a str },
    Const { name: &'a str, kind: Kind, from: usize, to: usize },
}

type Span = std::ops::RangeInclusive<usize>;

/// The tokens, the sharing key, and the slot values in the same order throughout a group.
struct Read<'a> {
    toks: Vec<Tok<'a>>,
    key: Vec<u16>,
    slots: Vec<Slot<'a>>,
    values: Vec<f64>,
    per_draw: Option<Span>,
}

#[derive(Default)]
struct Function<'a> {
    flow: bool,
    calls: HashSet<&'a str>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Statement {
    Const,
    Case,
}

struct Const {
    kind: Kind,
    from: usize,
}

fn reached<'a>(entry: &'a str, fns: &HashMap<&'a str, Function<'a>>) -> HashSet<&'a str> {
    let mut seen = HashSet::new();
    let mut stack = vec![entry];
    while let Some(f) = stack.pop() {
        if let Some(fun) = fns.get(f)
            && seen.insert(f)
        {
            stack.extend(&fun.calls);
        }
    }
    seen
}

fn entries(wgsl: &str) -> impl Iterator<Item = &str> {
    wgsl.match_indices('@').filter_map(|(at, _)| {
        let rest = &wgsl[at + 1..];
        let rest = ["vertex", "fragment", "compute"].into_iter().find_map(|stage| rest.strip_prefix(stage))?;
        let tail = rest.trim_start_matches(whitespace);
        if tail.len() == rest.len() {
            return None;
        }
        let rest = tail.strip_prefix("fn")?;
        let tail = rest.trim_start_matches(whitespace);
        let (name, _) = word(tail);
        (tail.len() != rest.len() && !name.is_empty()).then_some(name)
    })
}

/// The shader's sharing key, or why it cannot be shared.
fn read<'a>(wgsl: &'a str, tables: &HashMap<&str, Accessor<'a>>) -> Result<Read<'a>, String> {
    let toks = tokenize(wgsl);
    let code: Vec<_> = toks.iter().enumerate().filter_map(|(i, t)| t.code.then_some(i)).collect();
    let at = |k: usize| code.get(k).map_or("", |&i| toks[i].text);
    // Pass 1: declared names, functions and where numbers are context, not values.
    let mut declared = HashMap::new();
    let mut fn_of = Vec::with_capacity(code.len());
    let mut fixed = Vec::with_capacity(code.len());
    let mut fns = HashMap::<_, Function<'_>>::new();
    let (mut depth, mut fun, mut template, mut paren, mut attribute, mut statement) = (0, None, 0, 0, -1, None);
    // The innermost call is last, with the index of the argument the token is in.
    let mut calls = Vec::new();
    for k in 0..code.len() {
        let t = at(k);
        let prev = at(k.wrapping_sub(1));
        if matches!(t, "fn" | "let" | "var" | "const" | "override" | "struct" | "alias") {
            let mut n = k + 1;
            if at(n) == "<" {
                let mut d = 1;
                n += 1;
                while d > 0 && n < code.len() {
                    d += i32::from(at(n) == "<") - i32::from(at(n) == ">");
                    n += 1;
                }
            }
            let name = at(n);
            if ident(name) {
                let index = declared.len();
                declared.entry(name).or_insert(index);
            }
            if t == "fn" && depth == 0 {
                fun = Some(name);
                fns.insert(name, Function::default());
            }
        }
        match t {
            "const" | "override" if statement.is_none() => statement = Some(Statement::Const),
            "case" => statement = Some(Statement::Case),
            "@" => attribute = paren,
            "<" if templated(prev) => template += 1,
            ">" if template > 0 => template -= 1,
            "(" => {
                paren += 1;
                calls.push((if ident(prev) { prev } else { "" }, 0));
            }
            ")" => {
                paren -= 1;
                calls.pop();
                if attribute == paren {
                    attribute = -1;
                }
            }
            "," => {
                if let Some((_, arg)) = calls.last_mut() {
                    *arg += 1;
                }
            }
            _ => {}
        }
        fixed.push(
            template > 0
                || statement.is_some()
                || attribute >= 0
                || depth == 0
                || fun.is_none()
                || folds(prev, at(k.wrapping_sub(2)), calls.last()),
        );
        fn_of.push(if depth > 0 { fun } else { None });
        if let Some(name) = fun
            && depth > 0
        {
            let f = fns.get_mut(name).unwrap();
            f.flow |= matches!(t, "if" | "for" | "while" | "loop" | "switch");
            if ident(t) && at(k + 1) == "(" {
                f.calls.insert(t);
            }
        }
        match t {
            ";" if statement == Some(Statement::Const) => statement = None,
            ":" if statement == Some(Statement::Case) => statement = None,
            "{" => depth += 1,
            "}" => {
                depth -= 1;
                if depth == 0 {
                    fun = None;
                }
            }
            _ => {}
        }
    }

    if !vs_of(wgsl).is_some_and(|(_, params, out)| instance(params).is_some() && fs_of(wgsl, out).is_some())
        || !fns.contains_key("vs")
        || !fns.contains_key("fs")
    {
        return Err("not the vs(instance_index) and fs(VOut) template".into());
    }
    // Only vs and fs set loft_row. A tracer's depth-only fs_depth could otherwise read row zero,
    // so a shader with another entry point compiles on its own.
    let entries: Vec<_> = entries(wgsl).collect();
    if entries.iter().any(|&e| e != "vs" && e != "fs") {
        return Err(format!("has an entry point besides vs and fs ({})", entries.join(", ")));
    }

    // Keep literals reached by the vertex stage (geometry) or by a fragment function with control flow (uniformity).
    let vertex = reached("vs", &fns);
    let fragment = reached("fs", &fns);
    let branchy = |k: usize| fn_of[k].is_some_and(|f| fragment.contains(f) && fns[f].flow);
    let guarded = |k: usize| fn_of[k].is_none_or(|f| vertex.contains(f) || branchy(k));

    // Module-scope const NAME: T = NUMBER; can become a hole when every use is a value inside a function.
    let mut consts = HashMap::new();
    for (k, function) in fn_of.iter().enumerate().take(code.len().saturating_sub(6)) {
        if at(k) != "const" || function.is_some() || at(k + 2) != ":" || at(k + 4) != "=" || at(k + 6) != ";" {
            continue;
        }
        if let Some(kind) = number_kind(at(k + 5))
            && kind.name() == at(k + 3)
        {
            consts.insert(at(k + 1), Const { kind, from: k });
        }
    }
    for (k, &fixed) in fixed.iter().enumerate() {
        if let Some(c) = consts.get(at(k))
            && k != c.from + 1
            && (fixed || branchy(k))
        {
            consts.remove(at(k));
        }
    }
    // A surviving declaration has exactly one number token. Index its offset for the per-token pass.
    let const_by_number: HashMap<_, _> = consts.values().map(|c| (c.from + 5, c)).collect();

    let mut per_draw = None;
    let mut key = Vec::new();
    let mut slots = Vec::new();
    let mut values = Vec::new();
    for (k, &i) in code.iter().enumerate() {
        let t = at(k);
        if k > 0 {
            key.push(u16::from(b' '));
        }
        if let Some(c) = const_by_number.get(&k) {
            key.push(u16::from(b'#'));
            key.extend(c.kind.name().encode_utf16());
            slots.push(Slot::Const { name: at(c.from + 1), kind: c.kind, from: code[c.from], to: code[c.from + 6] });
            values.push(number_value(t, c.kind));
            continue;
        }
        if t.starts_with(|c: char| c.is_ascii_digit() || c == '.')
            && let Some(kind) = number_kind(t)
            && !fixed[k]
            && !guarded(k)
        {
            let value = number_value(t, kind);
            if kind == Kind::F32 || value < EXACT {
                key.push(u16::from(b'#'));
                key.extend(kind.name().encode_utf16());
                slots.push(Slot::Literal { at: i, kind });
                values.push(value);
                continue;
            }
        }
        if let Some(table) = tables.get(t)
            && at(k + 1) == "("
            && !fixed[k]
        {
            key.extend("T:".encode_utf16());
            key.extend(table.buffer.encode_utf16());
            slots.push(Slot::Base { at: i, buffer: table.buffer });
            values.push(table.base);
            continue;
        }
        if t == "override" && at(k + 1) == PER_DRAW {
            let end = (k..code.len())
                .find(|&end| at(end) == ";")
                .ok_or_else(|| format!("unterminated {PER_DRAW} override"))?;
            per_draw = Some(i..=code[end]);
        }
        if let Some(local) = declared.get(t)
            && !consts.contains_key(t)
        {
            key.push(u16::from(b'L'));
            key.extend(local.to_string().encode_utf16());
        } else if let Some(unit) = toks[i].unit {
            key.push(unit);
        } else {
            key.extend(t.encode_utf16());
        }
    }
    Ok(Read { toks, key, slots, values, per_draw })
}

/// The first member's WGSL, with each hole read from the draw's row.
fn group_wgsl(first: &Read<'_>, holes: &[usize], per_draw_hole: Option<usize>, start: usize, width: usize) -> String {
    let read = |j: usize, kind| match kind {
        Kind::F32 => format!("loft_k({j}u)"),
        Kind::U32 | Kind::I32 => format!("{}(loft_k({j}u))", kind.name()),
    };
    let toks = &first.toks;
    let mut replace = HashMap::new();
    let mut rename = HashMap::new();
    let mut buffers = Vec::new();
    for (j, &s) in holes.iter().enumerate() {
        match first.slots[s] {
            Slot::Literal { at, kind } => {
                replace.insert(at, read(j, kind));
            }
            // t_name(i) becomes loft_tab_BUFFER(base, i), with the base read from the row.
            Slot::Base { at, buffer } => {
                replace.insert(at, format!("loft_tab_{buffer}({}, ", read(j, Kind::U32)));
                let next = (at + 1..toks.len()).find(|&n| toks[n].code).unwrap();
                replace.insert(next, String::new());
                if !buffers.contains(&buffer) {
                    buffers.push(buffer);
                }
            }
            Slot::Const { name, kind, from, to } => {
                replace.extend((from..=to).map(|i| (i, String::new())));
                rename.insert(name, read(j, kind));
            }
        }
    }
    if let Some(j) = per_draw_hole {
        replace.extend(first.per_draw.clone().unwrap().map(|i| (i, String::new())));
        rename.insert(PER_DRAW, read(j, Kind::U32));
    }
    let body: String = toks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            replace.get(&i).or_else(|| t.code.then(|| rename.get(t.text)).flatten()).map_or(t.text, String::as_str)
        })
        .collect();
    let mut out = format!("@group(0) @binding({ROWS_BINDING}) var<storage, read> loft_rows: array<f32>;\n");
    // Set from the instance index by the vertex stage and from the flat varying by the fragment stage.
    out.push_str("var<private> loft_row: u32;\nfn loft_k(j: u32) -> f32 { return loft_rows[loft_row + j]; }\n");
    for b in buffers {
        writeln!(out, "fn loft_tab_{b}(base: u32, i: u32) -> vec4<f32> {{ return {b}[base + i]; }}").unwrap();
    }
    out.push_str(&plumb(&body, start, width));
    out
}

/// vs computes the row from its instance index and passes it on. fs reads it back.
fn plumb(wgsl: &str, start: usize, width: usize) -> String {
    let (vs_head, params, out_type) = vs_of(wgsl).unwrap();
    let (instance_head, ii) = instance(params).unwrap();
    let struct_prefix = format!("struct {out_type} {{");
    let (at, _) = wgsl.match_indices(&struct_prefix).next().unwrap();
    let tail = &wgsl[at + struct_prefix.len()..];
    let fields = tail.split_once('}').unwrap().0;
    let next = fields
        .match_indices("@location(")
        .filter_map(|(at, head)| {
            let tail = &fields[at + head.len()..];
            let (n, tail) = tail.split_at(digits(tail));
            tail.strip_prefix(')')?;
            n.parse::<u64>().ok()
        })
        .max()
        .map_or(0, |n| n + 1);
    let previous = &wgsl[at..at + struct_prefix.len() + fields.len() + 1];
    let mut out = wgsl.replacen(
        previous,
        &format!("struct {out_type} {{{fields}  @location({next}) @interpolate(flat) loft_row: u32,\n}}"),
        1,
    );
    let params = params.replacen(instance_head, "@builtin(instance_index) loft_instance: u32", 1);
    let mask = (1u32 << COPY_BITS) - 1;
    out = out.replacen(vs_head, &format!(
        "@vertex fn vs({params}) -> {out_type} {{\n  loft_row = {start}u + (loft_instance >> {COPY_BITS}u) * {width}u;\n  let {ii} = loft_instance & {mask}u;"
    ), 1);
    // Each return X; of vs passes the row on. The first line that closes vs is part of the templates' layout.
    let from = out.find("@vertex fn vs(").unwrap();
    let end = out[from..].find("\n}").map_or(out.len() - 1, |end| from + end);
    let mut vs = String::new();
    let code = &out[from..end];
    let mut last = 0;
    for (at, head) in code.match_indices("return ") {
        let tail = &code[at + head.len()..];
        let (value, tail) = word(tail);
        if !value.is_empty() && tail.starts_with(';') {
            vs.push_str(&code[last..at]);
            write!(vs, "{value}.loft_row = loft_row;\n  return {value};").unwrap();
            last = at + head.len() + value.len() + 1;
        }
    }
    vs.push_str(&code[last..]);
    out.replace_range(from..end, &vs);
    let (fs_head, arg) = fs_of(&out, out_type).unwrap();
    out.replacen(fs_head, &format!("{fs_head}\n  loft_row = {arg}.loft_row;"), 1)
}

pub fn share(a: &ShareInput) -> Sharing<'_> {
    try_share(a).unwrap_or_else(|e| panic!("{e}"))
}

/// Fallible form for an artefact supplied by the scene.
pub fn try_share(a: &ShareInput) -> Result<Sharing<'_>, Error> {
    let tables = accessors(&a.scene_wgsl);
    let shaders: HashMap<_, _> = a.shaders.iter().map(|(name, wgsl)| (name.as_str(), wgsl.as_str())).collect();
    let mut seen = HashSet::new();
    let used: Vec<_> = a.draws.iter().map(|d| d.shader.as_str()).filter(|&s| seen.insert(s)).collect();
    let mut reads = HashMap::new();
    let mut by_key = Vec::<Vec<&str>>::new();
    let mut keys = HashMap::new();
    for s in used {
        // A precision-specific module can have different literal slots. Both variants keep their own complete shader.
        if a.mixed_shaders.iter().any(|name| name == s) {
            continue;
        }
        let wgsl = shaders.get(s).ok_or_else(|| Error::Internal(format!("share: shader \"{s}\" is missing")))?;
        // Unshareable shaders compile on their own, as before sharing.
        let Ok(mut r) = read(wgsl, &tables) else { continue };
        if a.draws.iter().any(|d| d.shader == s && d.instances > (1 << COPY_BITS)) {
            continue;
        }
        let group = *keys.entry(std::mem::take(&mut r.key)).or_insert_with(|| {
            by_key.push(Vec::new());
            by_key.len() - 1
        });
        by_key[group].push(s);
        reads.insert(s, r);
    }

    let mut module = HashMap::new();
    let mut groups = Vec::new();
    let mut draws: Vec<_> =
        a.draws.iter().map(|d| SharedDraw { constants: d.constants.clone(), instance_base: 0 }).collect();
    let mut rows = Vec::new();
    for members in by_key {
        if members.len() < 2 {
            continue;
        }
        let first = &reads[members[0]];
        let holes: Vec<_> =
            (0..first.slots.len()).filter(|&j| members.iter().any(|s| reads[s].values[j] != first.values[j])).collect();
        let per_draw_hole = first.per_draw.as_ref().map(|_| holes.len());
        let width = holes.len() + usize::from(per_draw_hole.is_some());
        let start = rows.len();
        let mut row_of = HashMap::new();
        for (k, d) in a.draws.iter().enumerate() {
            if !members.contains(&d.shader.as_str()) {
                continue;
            }
            let values = &reads[d.shader.as_str()].values;
            let mut row: Vec<_> = holes.iter().map(|&j| values[j]).collect();
            if per_draw_hole.is_some() {
                // A draw without a slot gets the override's declared default, 0u (cellsWgsl).
                let value = d.constants.iter().find(|(name, _)| name == PER_DRAW).map_or(0.0, |(_, value)| *value);
                row.push(value);
                draws[k].constants.retain(|(name, _)| name != PER_DRAW);
            }
            // This is plain JSON.stringify(row), so its key uses null for a non-finite number.
            let id = to_json(&row.iter().map(|&x| x.is_finite().then_some(x)).collect::<Vec<_>>());
            let next = row_of.len();
            let index = *row_of.entry(id).or_insert_with(|| {
                rows.extend(row.into_iter().map(|x| x as f32));
                next
            });
            draws[k].instance_base = (index as u32) << COPY_BITS;
        }
        if row_of.len() > 1 << (32 - COPY_BITS) {
            return Err(Error::Internal(format!(
                "share: {} rows in one group overflow the instance index",
                row_of.len()
            )));
        }
        let group = groups.len();
        let key = format!("shared {group} ({})", members.join(", "));
        let wgsl = group_wgsl(first, &holes, per_draw_hole, start, width);
        groups.push((key, wgsl));
        module.extend(members.into_iter().map(|s| (s, group)));
    }
    Ok(Sharing { shaders, module, groups, draws, rows })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use serde::Deserialize;

    use super::*;
    use crate::files::tests::references;

    #[test]
    fn sharing_matches_ts_chunks_and_rows() {
        // Catches different grouping, slot selection, row dedupe/order, and draws addressing the wrong row.
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Compiled {
            scene_wgsl: String,
            shaders: BTreeMap<String, String>,
            draws: Vec<Draw>,
            mixed: Option<Mixed>,
        }
        #[derive(Deserialize)]
        struct Draw {
            shader: String,
            constants: BTreeMap<String, f64>,
            instances: u32,
        }
        #[derive(Deserialize)]
        struct Mixed {
            shaders: BTreeMap<String, String>,
        }
        #[derive(Deserialize)]
        struct Manifest {
            chunks: Vec<String>,
            modules: Vec<Module>,
            frame: Frame,
            #[serde(rename = "renderPipelines")]
            render_pipelines: Vec<Pipeline>,
        }
        #[derive(Deserialize)]
        struct Module {
            label: String,
            chunks: Vec<usize>,
        }
        #[derive(Deserialize)]
        struct Frame {
            passes: Vec<Pass>,
        }
        #[derive(Deserialize)]
        struct Pass {
            commands: Vec<Command>,
        }
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
        enum Command {
            Fixed,
            Planned {
                pipeline: usize,
                draw: usize,
                #[serde(default)]
                instance_base: u32,
            },
        }
        #[derive(Deserialize)]
        struct Pipeline {
            vertex: Stage,
        }
        #[derive(Deserialize)]
        struct Stage {
            constants: BTreeMap<String, f64>,
        }

        // Straight decimal -> f32 would land on the other side of the first midpoint.
        assert_eq!(number_value("1.0000000596046448", Kind::F32), 1.0);

        let references = references();
        let mut modules = 0;
        let mut row_floats = 0;
        for dir in &references {
            let compile = dir.with_file_name("compile.json");
            let a: Compiled = serde_json::from_slice(&fs::read(&compile).unwrap()).unwrap();
            let a = ShareInput {
                scene_wgsl: a.scene_wgsl,
                shaders: a.shaders.into_iter().collect(),
                mixed_shaders: a.mixed.into_iter().flat_map(|m| m.shaders.into_keys()).collect(),
                draws: a
                    .draws
                    .into_iter()
                    .map(|d| ShareDraw {
                        shader: d.shader,
                        constants: d.constants.into_iter().collect(),
                        instances: d.instances,
                    })
                    .collect(),
            };
            let shared = try_share(&a).unwrap();
            let manifest: Manifest = serde_json::from_slice(&fs::read(dir.join("manifest.json")).unwrap()).unwrap();
            let groups: HashMap<_, _> = shared.groups.iter().map(|(key, wgsl)| (key.as_str(), wgsl)).collect();
            let mut seen = HashSet::new();
            for module in manifest.modules {
                if let Some((key, _)) = module.label.strip_prefix("shader \"").and_then(|s| s.rsplit_once('"'))
                    && key.starts_with("shared ")
                {
                    let name = &manifest.chunks[*module.chunks.last().unwrap()];
                    let text = fs::read_to_string(dir.join("wgsl").join(format!("{name}.wgsl"))).unwrap();
                    let rust = groups.get(key).unwrap_or_else(|| panic!("{}: missing {key}", dir.display()));
                    assert_eq!(rust.as_bytes(), text.as_bytes(), "{}: {key}", dir.display());
                    seen.insert(key.to_owned());
                    modules += 1;
                }
            }
            assert_eq!(seen.len(), groups.len(), "{}: unexpected shared modules", dir.display());
            for cmd in manifest.frame.passes.into_iter().flat_map(|pass| pass.commands) {
                if let Command::Planned { pipeline, draw, instance_base } = cmd {
                    let actual = shared.draw_of(draw);
                    let constants: BTreeMap<_, _> = actual.constants.iter().cloned().collect();
                    assert_eq!(actual.instance_base, instance_base, "{}: draw {draw} row", dir.display());
                    assert_eq!(
                        constants,
                        manifest.render_pipelines[pipeline].vertex.constants,
                        "{}: draw {draw} constants",
                        dir.display()
                    );
                }
            }
            let expected = dir.join("bin/loft rows.bin");
            if expected.exists() || !shared.rows.is_empty() {
                let rows = fs::read(expected).unwrap();
                let actual: Vec<_> = shared.rows.iter().flat_map(|v| v.to_le_bytes()).collect();
                assert_eq!(actual, rows, "{}: loft rows", dir.display());
                row_floats += actual.len() / 4;
            }
        }
        println!(
            "{modules} shared modules and {row_floats} row floats matched across {} TS artefacts",
            references.len()
        );
    }
}
