//! Draw shaders that are one shader up to their numbers (src/host/share.ts), as one pass over each shader's tokens.
//! A line-for-line twin: the same tokens, sharing keys, holes, rows and group WGSL text. JS regular expressions are
//! written out as scanners with the same leftmost, first-alternative (or greedy) choice. A shader this file does not
//! read exactly as JS would (non-ASCII outside a comment, a `$` that String.replace would interpret, a template
//! missing a line plumb() edits) refuses the whole call by name, and TypeScript shares.

use crate::wire::{Reader, WireError, Writer};
use crate::fx::{FxMap as HashMap, FxSet as HashSet};

const COPY_BITS: u32 = 16;
const PER_DRAW: &str = "LOFT_DETAIL_SLOT";
const EXACT: f64 = (1u32 << 24) as f64;
const ROWS_BINDING: u32 = 5;

/// A refusal: this input is outside what the twin reproduces exactly.
struct Refuse(String);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind { F32, U32, I32 }
impl Kind {
    fn name(self) -> &'static str { match self { Kind::F32 => "f32", Kind::U32 => "u32", Kind::I32 => "i32" } }
    fn of(s: &str) -> Option<Kind> { match s { "f32" => Some(Kind::F32), "u32" => Some(Kind::U32), "i32" => Some(Kind::I32), _ => None } }
}

struct Tok<'a> { text: &'a str, code: bool }

fn is_ident_start(b: u8) -> bool { b.is_ascii_alphabetic() || b == b'_' }
fn is_word(b: u8) -> bool { b.is_ascii_alphanumeric() || b == b'_' }
fn is_space(b: u8) -> bool { matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c) }

/// The TOKEN regex of share.ts, alternative by alternative.
fn tokenize(text: &str) -> Result<Vec<Tok<'_>>, Refuse> {
    let b = text.as_bytes();
    let n = b.len();
    let mut out = Vec::new();
    let mut i = 0;
    let digits = |mut j: usize| { while j < n && b[j].is_ascii_digit() { j += 1; } j };
    while i < n {
        let c = b[i];
        let end: usize;
        if is_space(c) {
            let mut j = i;
            while j < n && is_space(b[j]) { j += 1; }
            end = j;
        } else if c == b'/' && i + 1 < n && b[i + 1] == b'/' {
            let mut j = i;
            while j < n && b[j] != b'\n' { j += 1; }
            end = j;
        } else if c == b'/' && i + 1 < n && b[i + 1] == b'*' && text[i + 2..].contains("*/") {
            end = i + 2 + text[i + 2..].find("*/").unwrap() + 2;
        } else if is_ident_start(c) {
            let mut j = i + 1;
            while j < n && is_word(b[j]) { j += 1; }
            end = j;
        } else if c == b'0' && i + 2 < n && (b[i + 1] == b'x' || b[i + 1] == b'X') && b[i + 2].is_ascii_hexdigit() {
            let mut j = i + 2;
            while j < n && b[j].is_ascii_hexdigit() { j += 1; }
            if j < n && (b[j] == b'i' || b[j] == b'u') { j += 1; }
            end = j;
        } else if let Some(j) = float_at(b, i, &digits) {
            end = j;
        } else if c.is_ascii_digit() {
            // \d+[eE][+-]?\d+f?  then  \d+[iuf]?
            let d = digits(i);
            let mut e = None;
            if d < n && (b[d] == b'e' || b[d] == b'E') {
                let mut k = d + 1;
                if k < n && (b[k] == b'+' || b[k] == b'-') { k += 1; }
                let k2 = digits(k);
                if k2 > k { e = Some(if k2 < n && b[k2] == b'f' { k2 + 1 } else { k2 }); }
            }
            end = match e { Some(j) => j, None => if d < n && matches!(b[d], b'i' | b'u' | b'f') { d + 1 } else { d } };
        } else if c < 0x80 {
            end = i + 1;
        } else {
            return Err(Refuse("a non-ASCII character outside a comment".into()));
        }
        let t = &text[i..end];
        let code = !is_space(c) && !t.starts_with("//") && !t.starts_with("/*");
        out.push(Tok { text: t, code });
        i = end;
    }
    Ok(out)
}

/// `(?:\d+\.\d*|\.\d+)(?:[eE][+-]?\d+)?f?` at i: its end.
fn float_at(b: &[u8], i: usize, digits: &dyn Fn(usize) -> usize) -> Option<usize> {
    let n = b.len();
    let mut j;
    let d = digits(i);
    if d > i && d < n && b[d] == b'.' {
        j = digits(d + 1);
    } else if b[i] == b'.' && i + 1 < n && b[i + 1].is_ascii_digit() {
        j = digits(i + 1);
    } else {
        return None;
    }
    if j < n && (b[j] == b'e' || b[j] == b'E') {
        let mut k = j + 1;
        if k < n && (b[k] == b'+' || b[k] == b'-') { k += 1; }
        let k2 = digits(k);
        if k2 > k { j = k2; }
    }
    if j < n && b[j] == b'f' { j += 1; }
    Some(j)
}

/// share.ts `numberKind`, on any token.
fn number_kind(t: &str) -> Option<Kind> {
    if t.ends_with('u') && !t.starts_with("0x") { return Some(Kind::U32); }
    if t.ends_with('i') && !t.starts_with("0x") { return Some(Kind::I32); }
    if float_prefix(t) || t.ends_with('f') { return Some(Kind::F32); }
    None
}

/// FLOAT: `^(?:\d+\.\d*|\.\d+|\d+[eE])`.
fn float_prefix(t: &str) -> bool {
    let b = t.as_bytes();
    let d = b.iter().take_while(|c| c.is_ascii_digit()).count();
    if d > 0 && d < b.len() && (b[d] == b'.' || b[d] == b'e' || b[d] == b'E') { return true; }
    b.len() >= 2 && b[0] == b'.' && b[1].is_ascii_digit()
}

/// JS `Number(s)` for the strings a token can be (decimal, exponent, hex); NaN otherwise.
fn js_number(s: &str) -> f64 {
    let t = s.trim_matches(|c: char| c.is_ascii_whitespace());
    if t.is_empty() { return 0.0; }
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        if !h.is_empty() && h.bytes().all(|c| c.is_ascii_hexdigit()) {
            return h.bytes().fold(0.0, |a, c| a * 16.0 + (c as char).to_digit(16).unwrap() as f64);
        }
        return f64::NAN;
    }
    let body = t.strip_prefix(['+', '-']).unwrap_or(t);
    if body == "Infinity" { return if t.starts_with('-') { f64::NEG_INFINITY } else { f64::INFINITY }; }
    // StrDecimalLiteral: digits [. digits] [e sign digits], or . digits [e ...]
    let b = body.as_bytes();
    let mut i = 0;
    let d0 = b.iter().take_while(|c| c.is_ascii_digit()).count();
    i += d0;
    let mut frac = 0;
    if i < b.len() && b[i] == b'.' { i += 1; frac = b[i..].iter().take_while(|c| c.is_ascii_digit()).count(); i += frac; }
    if d0 == 0 && frac == 0 { return f64::NAN; }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') { i += 1; }
        let e = b[i..].iter().take_while(|c| c.is_ascii_digit()).count();
        if e == 0 { return f64::NAN; }
        i += e;
    }
    if i != b.len() { return f64::NAN; }
    t.parse::<f64>().unwrap_or(f64::NAN)
}

/// JS `parseInt(s, 10)`.
fn parse_int(s: &str) -> f64 {
    let t = s.trim_start_matches(|c: char| c.is_ascii_whitespace());
    let (neg, body) = match t.as_bytes().first() { Some(b'-') => (true, &t[1..]), Some(b'+') => (false, &t[1..]), _ => (false, t) };
    let d: String = body.chars().take_while(|c| c.is_ascii_digit()).collect();
    if d.is_empty() { return f64::NAN; }
    let v = d.parse::<f64>().unwrap_or(f64::NAN);
    if neg { -v } else { v }
}

fn number_value(t: &str, kind: Kind) -> f64 {
    match kind {
        Kind::F32 => js_number(t.strip_suffix('f').unwrap_or(t)) as f32 as f64,
        _ => parse_int(t),
    }
}

/// A table accessor of the scene WGSL.
#[derive(Clone)]
struct Accessor { buffer: String, base: f64 }

/// ACCESSOR: `fn (t_\w+)\(i: u32\) -> vec4<f32> \{ return (loft_\w+)\[(\d+)u \+ i\]; \}` (later names win).
fn accessors(scene: &str) -> HashMap<String, Accessor> {
    let mut out = HashMap::default();
    let b = scene.as_bytes();
    let mut from = 0;
    while let Some(p) = scene[from..].find("fn t_") {
        let i = from + p;
        from = i + 1;
        let mut j = i + 5;
        while j < b.len() && is_word(b[j]) { j += 1; }
        if j == i + 5 { continue; }
        let name = &scene[i + 3..j];
        let rest = &scene[j..];
        let Some(r) = rest.strip_prefix("(i: u32) -> vec4<f32> { return loft_") else { continue };
        let w = r.bytes().take_while(|&c| is_word(c)).count();
        if w == 0 { continue; }
        // \w+ is greedy, then must meet `[`; a shorter \w+ would be followed by a word character, so this is the match.
        let buffer = format!("loft_{}", &r[..w]);
        let Some(r2) = r[w..].strip_prefix('[') else { continue };
        let d = r2.bytes().take_while(|c| c.is_ascii_digit()).count();
        if d == 0 { continue; }
        let Some(_) = r2[d..].strip_prefix("u + i]; }") else { continue };
        out.insert(name.to_string(), Accessor { buffer, base: r2[..d].parse::<f64>().unwrap() });
        from = j;
    }
    out
}

/// VS: `@vertex fn vs\(([^\n]*)\) -> (\w+) \{`: (whole match start, end, params, out type).
fn vs_match(w: &str) -> Option<(usize, usize, String, String)> {
    let mut from = 0;
    while let Some(p) = w[from..].find("@vertex fn vs(") {
        let i = from + p;
        let s = i + "@vertex fn vs(".len();
        let line_end = w[s..].find('\n').map(|e| s + e).unwrap_or(w.len());
        let line = &w[s..line_end];
        // Greedy [^\n]*: the last `) -> W {` on the line.
        let mut best = None;
        let mut q = 0;
        while let Some(k) = line[q..].find(") -> ") {
            let at = q + k;
            let r = &line[at + 5..];
            let wl = r.bytes().take_while(|&c| is_word(c)).count();
            if wl > 0 && r[wl..].starts_with(" {") { best = Some((at, wl)); }
            q = at + 1;
        }
        if let Some((at, wl)) = best {
            let params = line[..at].to_string();
            let out = line[at + 5..at + 5 + wl].to_string();
            return Some((i, s + at + 5 + wl + 2, params, out));
        }
        from = i + 1;
    }
    None
}

/// INSTANCE: `@builtin\(instance_index\) (\w+): u32` in params: (start, end, name).
fn instance_match(params: &str) -> Option<(usize, usize, String)> {
    let mut from = 0;
    while let Some(p) = params[from..].find("@builtin(instance_index) ") {
        let i = from + p;
        let s = i + "@builtin(instance_index) ".len();
        let wl = params[s..].bytes().take_while(|&c| is_word(c)).count();
        if wl > 0 && params[s + wl..].starts_with(": u32") { return Some((i, s + wl + 5, params[s..s + wl].to_string())); }
        from = i + 1;
    }
    None
}

/// fsOf(out): `@fragment fn fs\((\w+): OUT\b[^{]*\{`: (start, end, the input's name).
fn fs_match(w: &str, out: &str) -> Option<(usize, usize, String)> {
    let b = w.as_bytes();
    let mut from = 0;
    while let Some(p) = w[from..].find("@fragment fn fs(") {
        let i = from + p;
        let s = i + "@fragment fn fs(".len();
        let wl = w[s..].bytes().take_while(|&c| is_word(c)).count();
        let after = s + wl;
        if wl > 0 && w[after..].starts_with(": ") && w[after + 2..].starts_with(out) {
            let e = after + 2 + out.len();
            let boundary = e >= b.len() || !is_word(b[e]);
            if boundary {
                if let Some(brace) = w[e..].find('{') { return Some((i, e + brace + 1, w[s..after].to_string())); }
            }
        }
        from = i + 1;
    }
    None
}

/// A place in one shader that may hold a different number in another member of its group.
#[derive(Clone, Debug)]
enum Slot {
    Literal { at: usize, ty: Kind },
    Base { at: usize, buffer: String },
    Const { ty: Kind, name: String, from: usize, to: usize },
}

struct Read<'a> {
    toks: Vec<Tok<'a>>,
    key: String,
    slots: Vec<Slot>,
    values: Vec<f64>,
    per_draw: Option<(usize, usize)>,
}

struct FnInfo { flow: bool, calls: Vec<String> }

/// share.ts `read`: the sharing key, or why the shader cannot be shared (Ok(Err(reason))).
fn read<'a>(wgsl: &'a str, tables: &HashMap<String, Accessor>) -> Result<Result<Read<'a>, String>, Refuse> {
    let toks = tokenize(wgsl)?;
    let code: Vec<usize> = toks.iter().enumerate().filter(|(_, t)| t.code).map(|(i, _)| i).collect();
    let nc = code.len();
    let at = |k: isize| -> &str { if k >= 0 && (k as usize) < nc { toks[code[k as usize]].text } else { "" } };
    let mut declared: HashMap<&str, usize> = HashMap::default();
    let mut fn_of: Vec<Option<&str>> = Vec::with_capacity(nc);
    let mut fixed: Vec<bool> = Vec::with_capacity(nc);
    let mut fns: HashMap<&str, FnInfo> = HashMap::default();
    let (mut depth, mut template, mut paren, mut attribute): (i64, i64, i64, i64) = (0, 0, 0, -1);
    let mut cur: Option<&str> = None;
    #[derive(PartialEq)]
    enum St { None, Const, Case }
    let mut statement = St::None;
    let mut calls: Vec<(&str, i64)> = vec![];
    const DECLARES: [&str; 7] = ["fn", "let", "var", "const", "override", "struct", "alias"];
    for k in 0..nc as isize {
        let t = at(k);
        if DECLARES.contains(&t) {
            let mut n = k + 1;
            if at(n) == "<" {
                let mut d = 0;
                loop {
                    if at(n) == "<" { d += 1; }
                    if at(n) == ">" { d -= 1; }
                    n += 1;
                    if !(d > 0 && (n as usize) < nc) { break; }
                }
            }
            let name = at(n);
            if !name.is_empty() && is_ident_start(name.as_bytes()[0]) && !declared.contains_key(name) { let sz = declared.len(); declared.insert(name, sz); }
            if t == "fn" && depth == 0 { cur = Some(name); fns.insert(name, FnInfo { flow: false, calls: vec![] }); }
        }
        if (t == "const" || t == "override") && statement == St::None { statement = St::Const; }
        if t == "case" { statement = St::Case; }
        if t == "@" { attribute = paren; }
        if t == "<" && templated(at(k - 1)) { template += 1; } else if t == ">" && template > 0 { template -= 1; }
        if t == "(" {
            paren += 1;
            let prev = at(k - 1);
            calls.push((if !prev.is_empty() && is_ident_start(prev.as_bytes()[0]) { prev } else { "" }, 0));
        }
        if t == ")" { paren -= 1; calls.pop(); if attribute == paren { attribute = -1; } }
        if t == "," { if let Some(c) = calls.last_mut() { c.1 += 1; } }
        let folds = {
            let (prev, prev2) = (at(k - 1), at(k - 2));
            if prev == "/" || (prev == "-" && prev2 == "/") { true } else {
                match calls.last() { None => false, Some(&(name, arg)) => (name == "pow" && arg == 1) || (name == "smoothstep" && arg < 2) }
            }
        };
        fixed.push(template > 0 || statement != St::None || attribute >= 0 || depth == 0 || cur.is_none() || folds);
        fn_of.push(if depth > 0 { cur } else { None });
        if let (Some(f), true) = (cur, depth > 0) {
            let info = fns.get_mut(f).unwrap();
            if matches!(t, "if" | "for" | "while" | "loop" | "switch") { info.flow = true; }
            if !t.is_empty() && is_ident_start(t.as_bytes()[0]) && at(k + 1) == "(" && !info.calls.iter().any(|c| c == t) { info.calls.push(t.to_string()); }
        }
        if t == ";" && statement == St::Const { statement = St::None; }
        if t == ":" && statement == St::Case { statement = St::None; }
        if t == "{" { depth += 1; }
        if t == "}" { depth -= 1; if depth == 0 && cur.is_some() { cur = None; } }
    }
    let Some((_, _, params, out)) = vs_match(wgsl) else { return Ok(Err("not the vs(instance_index) and fs(VOut) template".into())) };
    if instance_match(&params).is_none() || fs_match(wgsl, &out).is_none() || !fns.contains_key("vs") || !fns.contains_key("fs") {
        return Ok(Err("not the vs(instance_index) and fs(VOut) template".into()));
    }
    if entry_points(wgsl).iter().any(|e| e != "vs" && e != "fs") { return Ok(Err("has an entry point besides vs and fs".into())); }
    let reached = |entry: &str| -> HashSet<String> {
        let mut seen = HashSet::default();
        let mut stack = vec![entry.to_string()];
        while let Some(f) = stack.pop() {
            if seen.contains(&f) || !fns.contains_key(f.as_str()) { continue; }
            stack.extend(fns[f.as_str()].calls.iter().cloned());
            seen.insert(f);
        }
        seen
    };
    let vertex = reached("vs");
    let fragment = reached("fs");
    let branchy = |k: usize| -> bool { match fn_of[k] { Some(f) => fragment.contains(f) && fns[f].flow, None => false } };
    let guarded = |k: usize| -> bool { match fn_of[k] { None => true, Some(f) => vertex.contains(f) || branchy(k) } };

    // Module-scope `const NAME: T = NUMBER;` (a Map: a later declaration of a name replaces the earlier one).
    struct Cst { ty: Kind, from: usize, to: usize, num: usize }
    let mut consts: HashMap<&str, Cst> = HashMap::default();
    let mut k = 0usize;
    while k + 6 < nc {
        let ki = k as isize;
        if at(ki) == "const" && fn_of[k].is_none() && at(ki + 2) == ":" && at(ki + 4) == "=" && at(ki + 6) == ";" {
            let ty = Kind::of(at(ki + 3));
            if let Some(ty) = ty { if number_kind(at(ki + 5)) == Some(ty) { consts.insert(at(ki + 1), Cst { ty, from: k, to: k + 6, num: k + 5 }); } }
        }
        k += 1;
    }
    for k in 0..nc {
        let name = at(k as isize);
        if let Some(c) = consts.get(name) {
            if k != c.from + 1 && (fixed[k] || branchy(k)) { consts.remove(name); }
        }
    }
    let const_by_number: HashMap<usize, &str> = consts.iter().map(|(name, c)| (c.num, *name)).collect();
    let mut per_draw = None;
    let mut key = String::with_capacity(wgsl.len());
    let mut slots = vec![];
    let mut values = vec![];
    let mut first_part = true;
    let mut part = |key: &mut String, p: &str| { if !first_part { key.push(' '); } first_part = false; key.push_str(p); };
    for k in 0..nc {
        let t = at(k as isize);
        if let Some(name) = const_by_number.get(&k) {
            let c = &consts[name];
            part(&mut key, &format!("#{}", c.ty.name()));
            slots.push(Slot::Const { ty: c.ty, name: at(c.from as isize + 1).to_string(), from: code[c.from], to: code[c.to] });
            values.push(number_value(t, c.ty));
            continue;
        }
        let kind = if t.starts_with(|c: char| c.is_ascii_digit() || c == '.') { number_kind(t) } else { None };
        if let Some(kind) = kind {
            if !fixed[k] && !guarded(k) && (kind == Kind::F32 || number_value(t, kind) < EXACT) {
                part(&mut key, &format!("#{}", kind.name()));
                slots.push(Slot::Literal { at: code[k], ty: kind });
                values.push(number_value(t, kind));
                continue;
            }
        }
        if let Some(table) = tables.get(t) {
            if at(k as isize + 1) == "(" && !fixed[k] {
                part(&mut key, &format!("T:{}", table.buffer));
                slots.push(Slot::Base { at: code[k], buffer: table.buffer.clone() });
                values.push(table.base);
                continue;
            }
        }
        if t == "override" && at(k as isize + 1) == PER_DRAW {
            let mut end = k;
            while at(end as isize) != ";" {
                end += 1;
                if end >= nc { return Err(Refuse("an override of LOFT_DETAIL_SLOT without its `;`".into())); }
            }
            per_draw = Some((code[k], code[end]));
        }
        match declared.get(t) {
            Some(&local) if !consts.contains_key(t) => part(&mut key, &format!("L{local}")),
            _ => part(&mut key, t),
        }
    }
    Ok(Ok(Read { toks, key, slots, values, per_draw }))
}

/// The entry points: `@(?:vertex|fragment|compute)\s+fn\s+(\w+)`.
fn entry_points(w: &str) -> Vec<String> {
    let b = w.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while let Some(p) = w[i..].find('@') {
        let s = i + p + 1;
        i = s;
        let word = ["vertex", "fragment", "compute"].iter().find(|x| w[s..].starts_with(**x));
        let Some(word) = word else { continue };
        let mut j = s + word.len();
        let sp = b[j..].iter().take_while(|&&c| is_space(c)).count();
        if sp == 0 { continue; }
        j += sp;
        if !w[j..].starts_with("fn") { continue; }
        j += 2;
        let sp2 = b[j..].iter().take_while(|&&c| is_space(c)).count();
        if sp2 == 0 { continue; }
        j += sp2;
        let wl = b[j..].iter().take_while(|&&c| is_word(c)).count();
        if wl == 0 { continue; }
        out.push(w[j..j + wl].to_string());
        i = j + wl;
    }
    out
}

/// TEMPLATED: identifiers whose `<` opens a template list.
fn templated(t: &str) -> bool {
    matches!(t, "array" | "vec2" | "vec3" | "vec4" | "ptr" | "atomic" | "binding_array" | "var")
        || (t.len() == 6 && t.starts_with("mat") && (b'2'..=b'4').contains(&t.as_bytes()[3]) && t.as_bytes()[4] == b'x' && (b'2'..=b'4').contains(&t.as_bytes()[5]))
        || (t.starts_with("texture_") && t.len() > 8 && t.bytes().all(is_word))
}

/// String.replace(string, string) without `$` patterns (the caller refuses text with `$`): the first occurrence.
fn replace_first(s: &str, from: &str, to: &str) -> String {
    match s.find(from) { Some(i) => format!("{}{}{}", &s[..i], to, &s[i + from.len()..]), None => s.to_string() }
}

fn group_wgsl(first: &Read, holes: &[usize], per_draw_hole: Option<usize>, start: usize, width: usize) -> Result<String, Refuse> {
    let read = |j: usize, ty: Kind| -> String { if ty == Kind::F32 { format!("loft_k({j}u)") } else { format!("{}(loft_k({j}u))", ty.name()) } };
    let toks = &first.toks;
    let next_code = |i: usize| { let mut n = i + 1; while !toks[n].code { n += 1; } n };
    let mut replace: Vec<Option<String>> = vec![None; toks.len()];
    let mut rename: HashMap<String, String> = HashMap::default();
    let mut buffers: Vec<String> = vec![];
    for (j, &s) in holes.iter().enumerate() {
        match &first.slots[s] {
            Slot::Literal { at, ty } => replace[*at] = Some(read(j, *ty)),
            Slot::Base { at, buffer } => {
                replace[*at] = Some(format!("loft_tab_{}({}, ", buffer, read(j, Kind::U32)));
                replace[next_code(*at)] = Some(String::new());
                if !buffers.contains(buffer) { buffers.push(buffer.clone()); }
            }
            Slot::Const { ty, name, from, to, .. } => {
                for r in replace.iter_mut().take(*to + 1).skip(*from) { *r = Some(String::new()); }
                rename.insert(name.clone(), read(j, *ty));
            }
        }
    }
    if let Some(h) = per_draw_hole {
        let (from, to) = first.per_draw.unwrap();
        for r in replace.iter_mut().take(to + 1).skip(from) { *r = Some(String::new()); }
        rename.insert(PER_DRAW.to_string(), read(h, Kind::U32));
    }
    let mut body = String::new();
    for (i, t) in toks.iter().enumerate() {
        match &replace[i] {
            Some(r) => body.push_str(r),
            None => if t.code { body.push_str(rename.get(t.text).map(|s| s.as_str()).unwrap_or(t.text)) } else { body.push_str(t.text) },
        }
    }
    let mut lines = vec![
        format!("@group(0) @binding({ROWS_BINDING}) var<storage, read> loft_rows: array<f32>;"),
        "var<private> loft_row: u32;".to_string(),
        "fn loft_k(j: u32) -> f32 { return loft_rows[loft_row + j]; }".to_string(),
    ];
    for b in &buffers { lines.push(format!("fn loft_tab_{b}(base: u32, i: u32) -> vec4<f32> {{ return {b}[base + i]; }}")); }
    lines.push(plumb(&body, start, width)?);
    Ok(lines.join("\n"))
}

/// share.ts `plumb`.
fn plumb(wgsl: &str, start: usize, width: usize) -> Result<String, Refuse> {
    let missing = || Refuse("a shared template without a line plumb() edits".into());
    let (vs0, vs1, params, out_ty) = vs_match(wgsl).ok_or_else(missing)?;
    let vs_head = &wgsl[vs0..vs1];
    let (i0, i1, ii) = instance_match(&params).ok_or_else(missing)?;
    let head = format!("struct {out_ty} {{");
    let si = wgsl.find(&head).ok_or_else(missing)?;
    let body_start = si + head.len();
    let close = wgsl[body_start..].find('}').ok_or_else(missing)?;
    let inner = &wgsl[body_start..body_start + close];
    let whole = &wgsl[si..body_start + close + 1];
    let mut next: i64 = -1;
    let mut q = 0;
    while let Some(p) = inner[q..].find("@location(") {
        let s = q + p + "@location(".len();
        let d = inner[s..].bytes().take_while(|c| c.is_ascii_digit()).count();
        if d > 0 && inner[s + d..].starts_with(')') { next = next.max(inner[s..s + d].parse::<i64>().unwrap_or(i64::MAX)); }
        q = s;
    }
    let next = next + 1;
    let mut out = replace_first(wgsl, whole, &format!("struct {out_ty} {{{inner}  @location({next}) @interpolate(flat) loft_row: u32,\n}}"));
    let new_params = format!("{}@builtin(instance_index) loft_instance: u32{}", &params[..i0], &params[i1..]);
    out = replace_first(&out, vs_head, &format!(
        "@vertex fn vs({new_params}) -> {out_ty} {{\n  loft_row = {start}u + (loft_instance >> {COPY_BITS}u) * {width}u;\n  let {ii} = loft_instance & {}u;",
        (1u32 << COPY_BITS) - 1));
    let from = out.find("@vertex fn vs(").ok_or_else(missing)?;
    let end = from + out[from..].find("\n}").ok_or_else(missing)?;
    let vs = returns(&out[from..end]);
    out = format!("{}{}{}", &out[..from], vs, &out[end..]);
    let (f0, f1, fin) = fs_match(&out, &out_ty).ok_or_else(missing)?;
    let fs = out[f0..f1].to_string();
    Ok(replace_first(&out, &fs, &format!("{fs}\n  loft_row = {fin}.loft_row;")))
}

/// `.replace(/return (\w+);/g, (_, v) => `${v}.loft_row = loft_row;\n  return ${v};`)`.
fn returns(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len() + 64);
    let mut i = 0;
    let mut copied = 0;
    while i < b.len() {
        if s[i..].starts_with("return ") {
            let j = i + 7;
            let wl = b[j..].iter().take_while(|&&c| is_word(c)).count();
            if wl > 0 && j + wl < b.len() && b[j + wl] == b';' {
                let v = &s[j..j + wl];
                out.push_str(&s[copied..i]);
                out.push_str(&format!("{v}.loft_row = loft_row;\n  return {v};"));
                i = j + wl + 1;
                copied = i;
                continue;
            }
        }
        i += 1;
    }
    out.push_str(&s[copied..]);
    out
}

/// `JSON.stringify(row)` as a key: numbers by value (-0 as 0), every non-finite one as `null`.
fn row_key(row: &[f64]) -> Vec<u64> {
    row.iter().map(|&v| if !v.is_finite() { u64::MAX } else if v == 0.0 { 0 } else { v.to_bits() }).collect()
}

pub fn run(r: &mut Reader, w: &mut Writer) -> Result<bool, WireError> {
    let scene = r.str()?;
    let nused = r.usize()?;
    let mut used: Vec<(String, Option<String>)> = Vec::with_capacity(nused);
    for _ in 0..nused {
        let name = r.str()?;
        // 1: eligible (not a precision-specific module, no draw with too many instances), then its text.
        let eligible = r.u32()? == 1;
        let text = r.str()?;
        used.push((name, if eligible { Some(text) } else { None }));
    }
    let ndraws = r.usize()?;
    let mut draws: Vec<(usize, f64)> = Vec::with_capacity(ndraws);
    for _ in 0..ndraws { let s = r.usize()?; let v = r.f64()?; draws.push((s, v)); }
    r.done()?;
    if scene.contains('$') || used.iter().any(|(_, t)| t.as_deref().is_some_and(|t| t.contains('$'))) {
        w.u32(0); w.str("a `$` in shader text (String.replace would read it as a pattern)"); return Ok(false);
    }
    match share(&scene, &used, &draws) {
        Ok(Shared::Done { groups, draw_out, rows }) => {
            w.u32(groups.len() as u32);
            for (key, wgsl, members) in &groups { w.str(key); w.str(wgsl); w.u32s(&members.iter().map(|&m| m as u32).collect::<Vec<_>>()); }
            for (drop, base) in &draw_out { w.u32(*drop as u32); w.f64(*base); }
            w.u32(rows.len() as u32);
            for v in &rows { w.f32(*v as f32); }
            Ok(true)
        }
        Ok(Shared::Overflow(n)) => { w.u32(1); w.u32(n as u32); Ok(false) }
        Err(Refuse(why)) => { w.u32(0); w.str(&why); Ok(false) }
    }
}

enum Shared {
    Done { groups: Vec<(String, String, Vec<usize>)>, draw_out: Vec<(bool, f64)>, rows: Vec<f64> },
    Overflow(usize),
}

fn share(scene: &str, used: &[(String, Option<String>)], draws: &[(usize, f64)]) -> Result<Shared, Refuse> {
    let tables = accessors(scene);
    let mut reads: Vec<Option<Read>> = Vec::with_capacity(used.len());
    for (_, text) in used {
        reads.push(match text { Some(t) => read(t, &tables)?.ok(), None => None });
    }
    // byKey: insertion order of first appearance.
    let mut order: Vec<String> = vec![];
    let mut by_key: HashMap<String, Vec<usize>> = HashMap::default();
    for (s, r) in reads.iter().enumerate() {
        if let Some(r) = r {
            by_key.entry(r.key.clone()).or_insert_with(|| { order.push(r.key.clone()); vec![] }).push(s);
        }
    }
    let mut groups = vec![];
    let mut draw_out: Vec<(bool, f64)> = vec![(false, 0.0); draws.len()];
    let mut rows: Vec<f64> = vec![];
    let mut group = 0;
    for key in &order {
        let members = &by_key[key];
        if members.len() < 2 { continue; }
        let first = reads[members[0]].as_ref().unwrap();
        let holes: Vec<usize> = (0..first.slots.len())
            .filter(|&j| members.iter().any(|&s| reads[s].as_ref().unwrap().values[j] != first.values[j]))
            .collect();
        let per = first.per_draw.is_some();
        let per_draw_hole = if per { Some(holes.len()) } else { None };
        let width = holes.len() + per as usize;
        let start = rows.len();
        let mut row_of: HashMap<Vec<u64>, usize> = HashMap::default();
        for (k, &(s, slot_value)) in draws.iter().enumerate() {
            if !members.contains(&s) { continue; }
            let values = &reads[s].as_ref().unwrap().values;
            let mut row: Vec<f64> = holes.iter().map(|&j| values[j]).collect();
            if per { row.push(slot_value); }
            let id = row_key(&row);
            let n = row_of.len();
            let idx = *row_of.entry(id).or_insert_with(|| { rows.extend_from_slice(&row); n });
            draw_out[k] = (per, idx as f64 * (1u64 << COPY_BITS) as f64);
        }
        if row_of.len() > 1 << (32 - COPY_BITS) { return Ok(Shared::Overflow(row_of.len())); }
        let names: Vec<&str> = members.iter().map(|&m| used[m].0.as_str()).collect();
        let gkey = format!("shared {} ({})", group, names.join(", "));
        group += 1;
        let wgsl = group_wgsl(first, &holes, per_draw_hole, start, width)?;
        groups.push((gkey, wgsl, members.clone()));
    }
    Ok(Shared::Done { groups, draw_out, rows })
}
