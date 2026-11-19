//! Checked render queries lower to the frozen tape. Residuals stage certified
//! combinations before the first position-dependent sample.
use crate::{Compiled, Prog, check::Ck};
use loft_abi::Tier0Opcode as Op;
use loft_ir::{Code, Diag, Res, Span};
use loft_parse::ast::*;
use std::collections::BTreeMap;
use std::fmt::Write;

use super::{Instruction, MAX_WORDS, VM_OPERATIONS, allocate};

const CONSTANTS: usize = 2048;
const HEADER: usize = 12;
pub const TAPE_BYTES: u64 = ((HEADER + MAX_WORDS + CONSTANTS * 4) * 4) as u64;

#[derive(Clone, Copy, Debug)]
pub(super) enum Operand {
    Zero,
    Constant(u16),
    Row(u16),
    Frame(u16),
}

impl Operand {
    fn encode(self) -> [u32; 2] {
        match self {
            Self::Zero => [0, 0],
            Self::Constant(i) => [1, u32::from(i)],
            Self::Row(i) => [2, u32::from(i)],
            Self::Frame(i) => [3, u32::from(i)],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Field {
    Sphere { center: Operand, radius: Operand },
    Box { center: Operand, half: Operand },
    Plane { axis: u16, offset: Operand },
}

impl Field {
    fn encode(self) -> [u32; 5] {
        let (op, center, extent) = match self {
            Self::Sphere { center, radius } => (Op::SdSphere as u32, center, radius),
            Self::Box { center, half } => (Op::SdBox as u32, center, half),
            Self::Plane { axis, offset } => (7 + u32::from(axis), Operand::Zero, offset),
        };
        let [ck, cs] = center.encode();
        let [ek, es] = extent.encode();
        [op, ck, cs, ek, es]
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Join {
    Min,
    Max,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Residual {
    Tape,
    CompactTape,
    Field(Field),
    Pair(Join, Field, Field),
}

#[derive(Clone, Copy)]
enum Value {
    Unknown,
    Uniform(Operand),
    Point(Operand),
    Coordinate(u16),
    Distance(Field),
    Pair(Join, Field, Field),
}

impl Residual {
    fn from_code(code: &[Instruction], constants: &mut Vec<[f32; 4]>) -> Self {
        let mut values = vec![Value::Unknown; code.len()];
        let mut loops = Vec::<(usize, u32)>::new();
        let mut pc = 0;
        // Only values proved independent of both the sample position and the
        // row/frame data can be folded. The original tape still records loops.
        while let Some(ins) = code.get(pc) {
            let [a, b, _] = ins.args.map(|i| values[i as usize]);
            let constant = |v| match v {
                Value::Uniform(Operand::Constant(i)) => Some(constants[usize::from(i)]),
                _ => None,
            };
            let value = match (ins.op, a, b) {
                (Op::LoadConst, ..) => Value::Uniform(Operand::Constant(ins.imm)),
                (Op::LoadRow, ..) => Value::Uniform(Operand::Row(ins.imm)),
                (Op::LoadFrame, ..) => Value::Uniform(Operand::Frame(ins.imm)),
                (Op::LoadInput, ..) => Value::Point(Operand::Zero),
                (Op::Mov, a, _) => a,
                (Op::Sub, Value::Point(Operand::Zero), Value::Uniform(center)) => {
                    Value::Point(center)
                }
                (Op::SdSphere, Value::Point(center), Value::Uniform(radius)) => {
                    Value::Distance(Field::Sphere { center, radius })
                }
                (Op::SdBox, Value::Point(center), Value::Uniform(half)) => {
                    Value::Distance(Field::Box { center, half })
                }
                (Op::Splat, Value::Point(Operand::Zero), _) if ins.imm < 3 => {
                    Value::Coordinate(ins.imm)
                }
                (Op::Sub, Value::Coordinate(axis), Value::Uniform(offset)) => {
                    Value::Distance(Field::Plane { axis, offset })
                }
                (Op::Min, Value::Distance(a), Value::Distance(b)) => Value::Pair(Join::Min, a, b),
                (Op::Max, Value::Distance(a), Value::Distance(b)) => Value::Pair(Join::Max, a, b),
                (Op::Ret, Value::Distance(field), _) => return Self::Field(field),
                (Op::Ret, Value::Pair(join, a, b), _) => return Self::Pair(join, a, b),
                (Op::Ret, ..) => return Self::Tape,
                (Op::LoopBegin, a, _) => {
                    let Some(trips) = constant(a) else {
                        return Self::Tape;
                    };
                    loops.push((pc, trips[0] as u32));
                    Value::Unknown
                }
                (Op::LoopEnd, ..) => {
                    let Some((start, trips)) = loops.last_mut() else {
                        return Self::Tape;
                    };
                    *trips -= 1;
                    if *trips != 0 {
                        pc = *start + 1;
                        continue;
                    }
                    loops.pop();
                    Value::Unknown
                }
                (op @ (Op::Add | Op::Sub | Op::Mul | Op::Div), a, b) => {
                    match (constant(a), constant(b)) {
                        (Some(a), Some(b)) => {
                            let v = std::array::from_fn(|i| match op {
                                Op::Add => a[i] + b[i],
                                Op::Sub => a[i] - b[i],
                                Op::Mul => a[i] * b[i],
                                Op::Div => a[i] / b[i],
                                _ => unreachable!("classified arithmetic"),
                            });
                            if constants.len() == CONSTANTS || !v.into_iter().all(f32::is_finite) {
                                return Self::Tape;
                            }
                            let slot = constants.len() as u16;
                            constants.push(v);
                            Value::Uniform(Operand::Constant(slot))
                        }
                        _ => Value::Unknown,
                    }
                }
                _ => Value::Unknown,
            };
            values[ins.dst as usize] = value;
            pc += 1;
        }
        Self::Tape
    }

    fn encode(self) -> [u32; HEADER - 1] {
        let (mode, a, b) = match self {
            Self::Tape => (0, [0; 5], [0; 5]),
            Self::CompactTape => (3, [0; 5], [0; 5]),
            Self::Field(field) => {
                let field = field.encode();
                (field[0], field, [0; 5])
            }
            Self::Pair(join, a, b) => (
                match join {
                    Join::Min => Op::Min as u32,
                    Join::Max => Op::Max as u32,
                },
                a.encode(),
                b.encode(),
            ),
        };
        let [a0, a1, a2, a3, a4] = a;
        let [b0, b1, b2, b3, b4] = b;
        [mode, a0, a1, a2, a3, a4, b0, b1, b2, b3, b4]
    }
}

pub struct TraceEdit {
    pub(super) words: Vec<u32>,
    pub(super) constants: Vec<[f32; 4]>,
    pub(super) unfolded: usize,
    pub(super) residual: Residual,
    pub(super) shader: String,
    pub(super) slots: Vec<usize>,
}

pub(super) fn unsupported(span: Span, reason: &str) -> Diag {
    Diag::new(Code::E405, span, reason, "use Tier 1 for this query")
}

impl TraceEdit {
    pub fn bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(TAPE_BYTES as usize);
        let header = std::iter::once(self.words.len() as u32).chain(self.residual.encode());
        bytes.extend(
            header
                .chain(self.words.iter().copied())
                .flat_map(u32::to_le_bytes),
        );
        bytes.resize((HEADER + MAX_WORDS) * 4, 0);
        bytes.extend(
            self.constants
                .iter()
                .flatten()
                .flat_map(|v| v.to_le_bytes()),
        );
        bytes
    }

    pub fn shader(&self, _specialized: &str) -> Result<String, String> {
        Ok(self.shader.clone())
    }

    pub fn describe(&self) -> String {
        let registers = live_registers(&self.words);
        let ids: Vec<_> = self.words.iter().map(|word| word >> 26).collect();
        let residual = match self.residual {
            Residual::Field(field) => format!("{field:?}"),
            residual @ (Residual::Pair(..) | Residual::CompactTape | Residual::Tape) => {
                format!("{residual:?}")
            }
        };
        format!("{residual}, opcodes {ids:?}, live vec4 registers {registers}")
    }
}

fn live_registers(words: &[u32]) -> usize {
    words
        .iter()
        .map(|&word| Op::unpack(word).expect("validated tape"))
        .filter(|(op, ..)| super::operands(*op).1)
        .map(|(_, dst, ..)| usize::from(dst) + 1)
        .max()
        .unwrap_or(0)
}

/// A checked library closure, identified by its existing source certificate.
/// An author may define a different function with the same name.
pub(super) struct Certificates(BTreeMap<&'static str, Op>);

impl Certificates {
    pub(super) fn is_primitive(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }

    pub(super) fn new(p: &Prog<'_>) -> Res<Self> {
        let mut ops = BTreeMap::new();
        for (name, op, hash) in [
            (
                "sphere",
                Op::SdSphere,
                "f8de200152a8ad78550c80e959ea5d93e4bd9e0e4921063373e07fbaf5368add",
            ),
            (
                "box_sdf",
                Op::SdBox,
                "103768bf7d29ffc6232109015b9f5dc3419951aeba89bac263e4b8bbf2c22035",
            ),
        ] {
            if let Some(&f) = p.fns.get(name) {
                // parse_all has already attached the same library proof after
                // checking the closure. Normalize away that synthesized axiom
                // only when constructing the source certificate.
                let mut source = f.clone();
                source
                    .attrs
                    .retain(|a| a.name.name != "lipschitz" || a.name.span != f.name.span);
                if crate::cache::certificate(p, &source)? == hash {
                    ops.insert(name, op);
                }
            }
        }
        Ok(Self(ops))
    }
}

pub fn authored_trace(compiled: &Compiled, name: &str) -> Res<TraceEdit> {
    let p = compiled.prog();
    let certificates = Certificates::new(&p)?;
    let kind = p
        .kinds
        .get(name)
        .ok_or_else(|| unsupported(Span::NONE, "unknown edited kind"))?;
    let mut tape = lower_trace(&p, &compiled.types, kind, &certificates)?;
    tape.shader = super::session::render_shader(compiled, &p, kind, &tape)?;
    Ok(tape)
}

pub(super) fn lower_trace(
    prog: &Prog<'_>,
    types: &[Ty],
    kind: &KindDecl,
    certificates: &Certificates,
) -> Res<TraceEdit> {
    let f = kind
        .query(QueryKind::Trace)
        .ok_or_else(|| unsupported(kind.name.span, "the kind has no trace query"))?;
    let [row, point] = f.params.as_slice() else {
        return Err(unsupported(
            f.name.span,
            "only trace queries are editable here",
        ));
    };
    let mut fields = BTreeMap::new();
    let mut slots = Vec::new();
    let mut unused = vec![];
    let ck = Ck::new(prog, &mut unused);
    for (field, decl) in kind.row.iter().enumerate() {
        if matches!(
            ck.resolve(&decl.ty, true)?,
            Ty::F32 | Ty::VecN(2..=4) | Ty::Geo(..)
        ) {
            let slot = u16::try_from(slots.len())
                .ok()
                .filter(|&i| usize::from(i) < CONSTANTS)
                .ok_or_else(|| unsupported(decl.name.span, "trace tape row limit"))?;
            fields.insert(decl.name.name.as_str(), slot);
            slots.push(field);
        }
    }
    let mut lower = Lower {
        prog,
        types,
        fields,
        row: &row.name.name,
        code: vec![],
        constants: vec![],
        depth: 0,
        certificates,
    };
    let point_reg = lower.push(Op::LoadInput, &[], 0, point.name.span)?;
    let out = lower.function(f, BTreeMap::from([(point.name.name.as_str(), point_reg)]))?;
    lower.push(Op::Ret, &[out], 0, f.name.span)?;
    let unfolded = lower.constants.len();
    let residual = Residual::from_code(&lower.code, &mut lower.constants);
    let words = allocate(&lower.code).map_err(|err| {
        Diag::new(
            Code::E602,
            f.name.span,
            format!("trace tape cannot be allocated: {err:?}"),
            "simplify the query or use its Tier 1 shader",
        )
    })?;
    let residual = match residual {
        Residual::Tape
            if live_registers(&words) <= 4
                && !lower.code.iter().any(|ins| ins.op == Op::LoopBegin) =>
        {
            Residual::CompactTape
        }
        residual => residual,
    };
    Ok(TraceEdit {
        words,
        unfolded,
        constants: lower.constants,
        slots,
        residual,
        shader: String::new(),
    })
}

pub(super) fn shader_queries(prog: &Prog<'_>, kind: &KindDecl, tape: &TraceEdit) -> Res<String> {
    let mut out = format!(
        "struct TierContext {{ base: u32, row: array<vec4f, {}>, clock: vec4f }}\n",
        tape.slots.len().max(1),
    );
    let const_base = HEADER + MAX_WORDS;
    writeln!(
        out,
        "const TIER_CONST: u32 = {const_base}u; const TIER_HEADER: u32 = {HEADER}u;"
    )
    .unwrap();
    out.push_str("fn tier_stage(row: Row, base: u32) -> TierContext {\n  var ctx: TierContext;\n  ctx.base = base;\n  ctx.clock = vec4f(frame.eye.w);\n");
    let mut types = vec![];
    let ck = Ck::new(prog, &mut types);
    for (slot, &field) in tape.slots.iter().enumerate() {
        let decl = &kind.row[field];
        let access = format!("row.r_{}", decl.name.name);
        let value = match ck.resolve(&decl.ty, true)? {
            Ty::F32 | Ty::VecN(4) => format!("vec4f({access})"),
            Ty::VecN(2) => format!("vec4f({access}, 0., 0.)"),
            Ty::VecN(3) | Ty::Geo(..) => format!("vec4f({access}, 0.)"),
            _ => unreachable!("checked tape row slots"),
        };
        writeln!(out, "  ctx.row[{slot}u] = {value};").unwrap();
    }
    out.push_str("  return ctx;\n}\n");
    out.push_str(VM_OPERATIONS);
    out.push_str(&compact_vm());
    out.push_str(include_str!("tier0/render.wgsl"));
    for (suffix, arg, eval) in [
        (
            "sphere",
            "shape: TierField",
            "tier_sphere({point} - shape.center, shape.extent.x)",
        ),
        (
            "box",
            "shape: TierField",
            "tier_box({point} - shape.center, shape.extent)",
        ),
        (
            "pair",
            "pair: TierPair",
            "tier_pair_distance(pair, {point})",
        ),
        (
            "plane",
            "shape: TierField",
            "({point})[shape.op - 7u] - shape.extent.x",
        ),
        (
            "compact",
            "ctx: TierContext",
            "tier_eval_compact(ctx, {point}).x",
        ),
        ("tape", "ctx: TierContext", "tier_eval(ctx, {point}).x"),
    ] {
        let march = eval.replace("{point}", "ro + rd * t");
        let shade = eval.replace("{point}", "q");
        write!(
            out,
            r"
fn tier_march_{suffix}({arg}, ro: vec3f, rd: vec3f, span: vec2f, lip: f32) -> f32 {{
    var t = max(span.x, 0.0);
    let step = 1.0 / lip;
    for (var i = 0u; i < 192u; i++) {{
        let d = {march};
        if d < 1e-4 + 2e-4 * t {{ return t; }}
        t += d * step;
        if t > span.y {{ break; }}
    }}
    return -1.0;
}}
fn tier_shade_{suffix}({arg}, p: vec3f, t: f32) -> vec4f {{
    let e = 5e-4 * max(1.0, 0.25 * t);
    var g = vec3f(0.0);
    var n = vec3f(0.0, 1.0, 0.0);
    var occ = 0.0;
    var w = 1.0;
    for (var i = 0u; i < 9u; i++) {{
        var q = p + LOFT_TETRA[min(i, 3u)] * e;
        var h = 0.0;
        if i >= 4u {{
            if i == 4u {{ n = normalize(g); }}
            h = 0.02 + 0.06 * f32(i - 4u);
            q = p + n * h;
        }}
        let d = {shade};
        if i < 4u {{ g += LOFT_TETRA[i] * d; }} else {{ occ += (h - d) * w; w *= 0.8; }}
    }}
    return vec4f(n, clamp(1.0 - 2.0 * occ, 0.0, 1.0));
}}
"
        )
        .unwrap();
    }
    Ok(out)
}

// Keep the opcode arithmetic shared with the ABI VM. The straight-line
// variant retains only the same four named register values across each step.
fn compact_vm() -> String {
    let (_, eval) = VM_OPERATIONS.split_once("fn tier_eval(").expect("VM entry");
    let (cases, _) = eval
        .split_once("            case 53u:")
        .expect("VM loop handlers");
    let (_, cases) = cases.split_once("            case 0u:").expect("VM cases");
    let cases = cases
        .split_once('\n')
        .expect("VM nop")
        .1
        .replace("reg[dst] =", "result =");
    format!(
        r"
fn tier_pick(a: vec4f, b: vec4f, c: vec4f, d: vec4f, i: u32) -> vec4f {{
    return select(select(a, b, (i & 1u) != 0u), select(c, d, (i & 1u) != 0u), (i & 2u) != 0u);
}}
fn tier_eval_compact(ctx: TierContext, point: vec3f) -> vec4f {{
    var r0 = vec4f(0.0);
    var r1 = vec4f(0.0);
    var r2 = vec4f(0.0);
    var r3 = vec4f(0.0);
    let count = tier_count(ctx);
    for (var pc = 0u; pc < count; pc++) {{
        let ins = tier_word(ctx, pc);
        let dst = (ins >> 21u) & 3u;
        let imm = ins & 2047u;
        let a = tier_pick(r0, r1, r2, r3, ins >> 16u);
        let b = tier_pick(r0, r1, r2, r3, ins >> 11u);
        let c = tier_pick(r0, r1, r2, r3, imm);
        var result: vec4f;
        switch ins >> 26u {{
            case 0u: {{ continue; }}
{cases}            case 55u: {{ return a; }}
            default: {{ return vec4f(bitcast<f32>(0x7fc00000u)); }}
        }}
        r0 = select(r0, result, dst == 0u);
        r1 = select(r1, result, dst == 1u);
        r2 = select(r2, result, dst == 2u);
        r3 = select(r3, result, dst == 3u);
    }}
    return vec4f(bitcast<f32>(0x7fc00000u));
}}
"
    )
}

struct Lower<'a, 'p> {
    prog: &'p Prog<'a>,
    types: &'p [Ty],
    fields: BTreeMap<&'a str, u16>,
    row: &'a str,
    certificates: &'a Certificates,
    code: Vec<Instruction>,
    constants: Vec<[f32; 4]>,
    depth: usize,
}

impl<'a> Lower<'a, '_> {
    fn push(&mut self, op: Op, args: &[u32], imm: u16, span: Span) -> Res<u32> {
        if self.code.len() == MAX_WORDS {
            return Err(unsupported(span, "trace tape instruction limit"));
        }
        let dst = self.code.len() as u32;
        self.emit(op, dst, args, imm, span)?;
        Ok(dst)
    }

    fn emit(&mut self, op: Op, dst: u32, args: &[u32], imm: u16, span: Span) -> Res<()> {
        if self.code.len() == MAX_WORDS {
            return Err(unsupported(span, "trace tape instruction limit"));
        }
        let mut operands = [0; 3];
        operands[..args.len()].copy_from_slice(args);
        self.code.push(Instruction {
            op,
            dst,
            args: operands,
            imm,
        });
        Ok(())
    }

    fn constant(&mut self, value: [f32; 4], span: Span) -> Res<u32> {
        if self.constants.len() == CONSTANTS || !value.into_iter().all(f32::is_finite) {
            return Err(unsupported(span, "trace tape constant limit"));
        }
        let i = self.constants.len() as u16;
        self.constants.push(value);
        self.push(Op::LoadConst, &[], i, span)
    }

    fn function(&mut self, f: &'a FnDecl, mut vars: BTreeMap<&'a str, u32>) -> Res<u32> {
        if f.raw.is_some() || self.depth >= 32 {
            return Err(unsupported(f.name.span, "raw or recursive trace helper"));
        }
        let [body @ .., Stmt::Return(Some(value), _)] = f.body.stmts.as_slice() else {
            return Err(unsupported(
                f.name.span,
                "trace helper needs a final expression",
            ));
        };
        self.depth += 1;
        self.statements(body, &mut vars, 1, f.name.span)?;
        let result = self.expr(value, &vars);
        self.depth -= 1;
        result
    }

    fn statements(
        &mut self,
        statements: &'a [Stmt],
        vars: &mut BTreeMap<&'a str, u32>,
        repetitions: u32,
        span: Span,
    ) -> Res<()> {
        for statement in statements {
            match statement {
                Stmt::Let {
                    name,
                    value,
                    mutable,
                    ..
                } => {
                    let value = self.expr(value, vars)?;
                    let register = if *mutable {
                        self.push(Op::Mov, &[value], 0, name.span)?
                    } else {
                        value
                    };
                    vars.insert(&name.name, register);
                }
                Stmt::Assign { target, op, value } => {
                    let ExprKind::Name(name) = &target.kind else {
                        return Err(unsupported(
                            target.span,
                            "trace assignment requires a local register",
                        ));
                    };
                    if !matches!(
                        self.types[target.id as usize],
                        Ty::F32 | Ty::VecN(_) | Ty::Geo(..)
                    ) {
                        return Err(unsupported(
                            target.span,
                            "trace assignment requires float or vector data",
                        ));
                    }
                    let dst = *vars.get(name.as_str()).ok_or_else(|| {
                        unsupported(target.span, "trace assignment requires a local variable")
                    })?;
                    let value = self.expr(value, vars)?;
                    let (operation, args): (_, &[u32]) = match op {
                        None => (Op::Mov, &[value]),
                        Some(BinOp::Add) => (Op::Add, &[dst, value]),
                        Some(BinOp::Sub) => (Op::Sub, &[dst, value]),
                        Some(BinOp::Mul) => (Op::Mul, &[dst, value]),
                        Some(BinOp::Div) => (Op::Div, &[dst, value]),
                        _ => {
                            return Err(unsupported(
                                target.span,
                                "assignment operator cannot be encoded in a trace tape",
                            ));
                        }
                    };
                    self.emit(operation, dst, args, 0, target.span)?;
                }
                Stmt::For {
                    comptime: false,
                    var,
                    lo,
                    hi,
                    body,
                    span,
                } => {
                    let integer = |e: &Expr| match e.kind {
                        ExprKind::Num { value, .. }
                            if (0.0..=2048.0).contains(&value) && value.fract() == 0.0 =>
                        {
                            Some(value as u32)
                        }
                        _ => None,
                    };
                    let lo = integer(lo).ok_or_else(|| {
                        unsupported(lo.span, "trace loop bounds must be small integer literals")
                    })?;
                    let hi = integer(hi).ok_or_else(|| {
                        unsupported(hi.span, "trace loop bounds must be small integer literals")
                    })?;
                    let trips = hi.saturating_sub(lo);
                    let total = repetitions.saturating_mul(trips);
                    if total > 2048 {
                        return Err(unsupported(*span, "trace loop trip budget exceeded"));
                    }
                    if trips == 0 {
                        continue;
                    }
                    let start = self.constant([lo as f32; 4], *span)?;
                    let index = self.push(Op::Mov, &[start], 0, *span)?;
                    let one = self.constant([1.; 4], *span)?;
                    let count = self.constant([trips as f32; 4], *span)?;
                    let entry = self.push(Op::LoopBegin, &[count], 0, *span)?;
                    let mut local = vars.clone();
                    local.insert(&var.name, index);
                    self.statements(&body.stmts, &mut local, total, *span)?;
                    self.emit(Op::Add, index, &[index, one], 0, *span)?;
                    let exit = self.push(Op::LoopEnd, &[], entry as u16, *span)?;
                    self.code[entry as usize].imm = exit as u16;
                }
                _ => {
                    return Err(unsupported(
                        span,
                        "statement cannot be encoded in an authored trace tape",
                    ));
                }
            }
        }
        Ok(())
    }

    fn expr(&mut self, e: &'a Expr, vars: &BTreeMap<&'a str, u32>) -> Res<u32> {
        let span = e.span;
        let refuse = || {
            unsupported(
                span,
                "expression cannot be encoded in an authored trace tape",
            )
        };
        match &e.kind {
            ExprKind::Name(n) => {
                if let Some(&reg) = vars.get(n.as_str()) {
                    return Ok(reg);
                }
                if n == "time" {
                    return self.push(Op::LoadFrame, &[], 0, span);
                }
                Err(refuse())
            }
            ExprKind::Num { value, suffix, .. } => {
                let value = if matches!(suffix, NumSuffix::Deg) {
                    value.to_radians()
                } else {
                    *value
                };
                self.constant([value as f32; 4], span)
            }
            ExprKind::Unary(UnOp::Neg, x) => {
                let x = self.expr(x, vars)?;
                self.push(Op::Neg, &[x], 0, span)
            }
            ExprKind::Binary(operator, a, b) => {
                if ![a.as_ref(), b.as_ref()].iter().all(|arg| {
                    matches!(
                        self.types[arg.id as usize],
                        Ty::F32 | Ty::VecN(_) | Ty::Geo(..)
                    )
                }) {
                    return Err(refuse());
                }
                let op = match operator {
                    BinOp::Add => Op::Add,
                    BinOp::Sub => Op::Sub,
                    BinOp::Mul => Op::Mul,
                    BinOp::Div => Op::Div,
                    BinOp::Lt | BinOp::Gt => Op::Lt,
                    BinOp::Le | BinOp::Ge => Op::Le,
                    BinOp::Eq => Op::Eq,
                    _ => return Err(refuse()),
                };
                let a = self.expr(a, vars)?;
                let b = self.expr(b, vars)?;
                let args = if matches!(operator, BinOp::Gt | BinOp::Ge) {
                    [b, a]
                } else {
                    [a, b]
                };
                self.push(op, &args, 0, span)
            }
            ExprKind::Bool(value) => self.constant([f32::from(*value); 4], span),
            ExprKind::If {
                condition,
                then,
                otherwise,
            } if then.body.stmts.is_empty() && otherwise.body.stmts.is_empty() => {
                let test = self.expr(condition, vars)?;
                let yes = self.expr(&then.value, vars)?;
                let no = self.expr(&otherwise.value, vars)?;
                self.push(Op::Select, &[no, yes, test], 0, span)
            }
            ExprKind::Member(base, field) => {
                if matches!(&base.kind, ExprKind::Name(n) if n == self.row) {
                    let slot = *self.fields.get(field.name.as_str()).ok_or_else(refuse)?;
                    return self.push(Op::LoadRow, &[], slot, span);
                }
                let lane = match field.name.as_str() {
                    "x" | "r" => 0,
                    "y" | "g" => 1,
                    "z" | "b" => 2,
                    "w" | "a" => 3,
                    "xyz" | "rgb" => return self.expr(base, vars),
                    _ => return Err(refuse()),
                };
                let a = self.expr(base, vars)?;
                self.push(Op::Splat, &[a], lane, span)
            }
            ExprKind::Ctor { args, .. } if (2..=4).contains(&args.len()) => {
                if args
                    .iter()
                    .any(|a| !matches!(self.types[a.value.id as usize], Ty::F32 | Ty::AbstractInt))
                {
                    return Err(refuse());
                }
                let mut regs = vec![];
                for (i, arg) in args.iter().enumerate() {
                    let v = self.expr(&arg.value, vars)?;
                    let mask = self.constant(std::array::from_fn(|j| f32::from(j == i)), span)?;
                    regs.push(self.push(Op::Mul, &[v, mask], 0, span)?);
                }
                let mut out = regs[0];
                for &v in &regs[1..] {
                    out = self.push(Op::Add, &[out, v], 0, span)?;
                }
                Ok(out)
            }
            ExprKind::Ctor { args, .. } if args.len() == 1 => {
                let a = &args[0].value;
                if !matches!(self.types[a.id as usize], Ty::F32 | Ty::AbstractInt)
                    && !(matches!(self.types[e.id as usize], Ty::F32)
                        && matches!(a.kind, ExprKind::Name(_))
                        && matches!(self.types[a.id as usize], Ty::I32 | Ty::U32))
                {
                    return Err(refuse());
                }
                self.expr(a, vars)
            }
            ExprKind::Call { callee, args } => {
                if args.iter().any(|a| a.name.is_some()) {
                    return Err(refuse());
                }
                let regs = args
                    .iter()
                    .map(|a| self.expr(&a.value, vars))
                    .collect::<Res<Vec<_>>>()?;
                if let Some(&op) = self.certificates.0.get(callee.name.as_str()) {
                    return self.push(op, &regs, 0, span);
                }
                if let Some(&f) = self.prog.fns.get(callee.name.as_str()) {
                    if f.params.len() != regs.len() {
                        return Err(refuse());
                    }
                    return self.function(
                        f,
                        f.params
                            .iter()
                            .map(|p| p.name.name.as_str())
                            .zip(regs)
                            .collect(),
                    );
                }
                let code = match (callee.name.as_str(), regs.as_slice()) {
                    ("f32", [a])
                        if matches!(args[0].value.kind, ExprKind::Name(_))
                            && matches!(
                                self.types[args[0].value.id as usize],
                                Ty::I32 | Ty::U32
                            ) =>
                    {
                        return Ok(*a);
                    }
                    ("length", [a])
                        if matches!(
                            self.types[args[0].value.id as usize],
                            Ty::VecN(3) | Ty::Geo(..)
                        ) =>
                    {
                        let d = self.push(Op::LenNorm, &[*a], 0, span)?;
                        return self.push(Op::Splat, &[d], 3, span);
                    }
                    ("min", [_, _]) => Op::Min,
                    ("max", [_, _]) => Op::Max,
                    ("abs", [_]) => Op::Abs,
                    ("sin", [_]) => Op::Sin,
                    ("cos", [_]) => Op::Cos,
                    ("sqrt", [_]) => Op::Sqrt,
                    ("exp", [_]) => Op::Exp,
                    ("log", [_]) => Op::Log,
                    ("floor", [_]) => Op::Floor,
                    ("fract", [_]) => Op::Fract,
                    ("pow", [_, _]) => Op::Pow,
                    ("atan2", [_, _]) => Op::Atan2,
                    ("fma", [_, _, _]) => Op::Fma,
                    ("mix", [_, _, _]) => Op::Mix,
                    ("clamp", [_, _, _]) => Op::Clamp,
                    ("step", [_, _]) => Op::Step,
                    ("smoothstep", [_, _, _]) => Op::Smoothstep,
                    ("cross", [_, _]) => Op::Cross,
                    ("normalize", [a])
                        if matches!(
                            self.types[args[0].value.id as usize],
                            Ty::VecN(3) | Ty::Geo(..)
                        ) =>
                    {
                        return self.push(Op::LenNorm, &[*a], 0, span);
                    }
                    ("dot", [_, _])
                        if args.iter().all(|a| {
                            matches!(self.types[a.value.id as usize], Ty::VecN(3) | Ty::Geo(..))
                        }) =>
                    {
                        Op::Dot3
                    }
                    _ => return Err(refuse()),
                };
                self.push(code, &regs, 0, span)
            }
            _ => Err(refuse()),
        }
    }
}
