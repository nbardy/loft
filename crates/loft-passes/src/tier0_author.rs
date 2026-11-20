//! Checked render queries lower to the frozen tape. Residuals stage certified
//! combinations before the first position-dependent sample.
use crate::{Compiled, Prog, check::Ck};
use loft_abi::Tier0Opcode as Op;
use loft_ir::{Code, Diag, Res, Span};
use loft_parse::ast::*;
use std::collections::BTreeMap;
use std::fmt::Write;

use super::plan::{HEADER, Plan, TREES};
use super::{Instruction, MAX_WORDS, VM_OPERATIONS, allocate};

pub(super) const CONSTANTS: usize = 2048;
pub const TAPE_BYTES: u64 = ((HEADER + MAX_WORDS * 3 + CONSTANTS * 4) * 4) as u64;

pub struct TraceEdit {
    pub(super) words: Vec<u32>,
    pub(super) constants: Vec<[f32; 4]>,
    pub(super) proof_constants: Vec<Option<[f64; 4]>>,
    pub(super) plan: Plan,
    pub(super) shader: String,
    pub(super) slots: Vec<usize>,
}

pub(super) fn unsupported(span: Span, reason: &str) -> Diag {
    Diag::new(Code::E405, span, reason, "use Tier 1 for this query")
}

impl TraceEdit {
    pub fn bytes(&self) -> Vec<u8> {
        let mut header = [0; HEADER];
        header[0] = self.words.len() as u32;
        header[2] = (HEADER + self.words.len()) as u32;
        header[3] = header[2] + self.constants.len() as u32 * 4;
        let mut bytes = vec![0; HEADER * 4];
        bytes.extend(self.words.iter().flat_map(|w| w.to_le_bytes()));
        bytes.extend(
            self.constants
                .iter()
                .flatten()
                .flat_map(|v| v.to_le_bytes()),
        );
        self.plan.encode(&mut header, &mut bytes);
        for (out, word) in bytes.chunks_exact_mut(4).zip(header) {
            out.copy_from_slice(&word.to_le_bytes());
        }
        bytes
    }

    pub fn shader(&self, _specialized: &str) -> Result<String, String> {
        Ok(self.shader.clone())
    }

    pub fn describe(&self) -> String {
        let registers = live_registers(&self.words);
        let ids: Vec<_> = self.words.iter().map(|word| word >> 26).collect();
        let plan = self.plan.describe();
        format!("{plan}, opcodes {ids:?}, live vec4 registers {registers}")
    }
}

pub(super) fn live_registers(words: &[u32]) -> usize {
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
        proof_constants: vec![],
        depth: 0,
        certificates,
    };
    let point_reg = lower.push(Op::LoadInput, &[], 0, point.name.span)?;
    let out = lower.function(f, BTreeMap::from([(point.name.name.as_str(), point_reg)]))?;
    lower.push(Op::Ret, &[out], 0, f.name.span)?;
    let words = allocate(&lower.code).map_err(|err| {
        Diag::new(
            Code::E602,
            f.name.span,
            format!("trace tape cannot be allocated: {err:?}"),
            "simplify the query or use its Tier 1 shader",
        )
    })?;
    let plan = Plan::new(
        &lower.code,
        &words,
        &mut lower.constants,
        &mut lower.proof_constants,
    );
    Ok(TraceEdit {
        words,
        proof_constants: lower.proof_constants,
        constants: lower.constants,
        slots,
        plan,
        shader: String::new(),
    })
}

pub(super) fn shader_queries(prog: &Prog<'_>, kind: &KindDecl, tape: &TraceEdit) -> Res<String> {
    let mut out = format!(
        "struct TierContext {{ base: u32, constant: u32, plan: u32, end: u32, split: u32, row: array<vec4f, {}>, clock: vec4f }}\n",
        tape.slots.len().max(1),
    );
    out.push_str("fn tier_stage(row: Row, base: u32) -> TierContext {\n  var ctx: TierContext;\n  ctx.base = base;\n  ctx.constant = base + tier_meta(base, 2u);\n  ctx.plan = tier_meta(base, 3u);\n  ctx.end = tier_meta(base, 4u);\n  ctx.split = tier_meta(base, 5u);\n  ctx.clock = vec4f(frame.eye.w);\n");
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
    let (_, runner) = VM_OPERATIONS.split_once("fn tier_run(").expect("shared VM");
    out.push_str("fn tier_run_cached(");
    out.push_str(&runner.replace("tier_decoded(ctx, pc)", "tier_decoded_cached(ctx, pc)"));
    out.push_str(include_str!("tier0/render.wgsl"));

    let mut evals = vec![
        (
            "sphere".to_owned(),
            "shape: TierField",
            "tier_sphere({point} - shape.center, shape.extent.x)".to_owned(),
        ),
        (
            "box".to_owned(),
            "shape: TierField",
            "tier_box({point} - shape.center, shape.extent)".to_owned(),
        ),
    ];
    for (shape, code) in TREES.iter().enumerate() {
        let mut leaves = 0;
        let mut joins = 0;
        let expr = tree_expression(&mut code.bytes(), &mut leaves, &mut joins);
        writeln!(
            out,
            "struct TierTree{shape} {{ mask: u32, {} }}",
            (0..leaves)
                .map(|i| format!("f{i}: TierField"))
                .collect::<Vec<_>>()
                .join(", ")
        )
        .unwrap();
        let fields = (0..leaves)
            .map(|i| format!("tier_field(ctx, {}u)", 6 + i * 5))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(out, "fn tier_tree_{shape}(ctx: TierContext) -> TierTree{shape} {{ return TierTree{shape}(tier_meta(ctx.base, 5u), {fields}); }}").unwrap();
        writeln!(
            out,
            "fn tier_distance_{shape}(s: TierTree{shape}, p: vec3f) -> f32 {{ return {expr}; }}"
        )
        .unwrap();
        evals.push((
            format!("tree{shape}"),
            "",
            format!("tier_distance_{shape}(shape, {{point}})"),
        ));
    }
    for (i, (suffix, arg, eval)) in evals.iter().enumerate() {
        let arg = if i >= 2 {
            format!("shape: TierTree{}", i - 2)
        } else {
            (*arg).to_owned()
        };
        emit_queries(&mut out, suffix, &arg, eval, eval, "", true);
    }
    let sample = "tier_eval(ctx, {point}, staged).x";
    let shade = "tier_eval_cached(ctx, {point}, staged).x";
    emit_queries(
        &mut out,
        "tape",
        "ctx: TierContext",
        sample,
        shade,
        "let staged = tier_prepare",
        false,
    );

    for (name, args, ret) in [
        (
            "q_trace",
            "point: vec3f",
            "tier_eval(ctx, point, tier_prepare(ctx)).x",
        ),
        (
            "loft_edit_march",
            "ro: vec3f, rd: vec3f, span: vec2f",
            "tier_march_tape(ctx, ro, rd, span, row.loft_lip)",
        ),
        (
            "loft_edit_normal_ao",
            "p: vec3f, t: f32",
            "tier_shade_tape(ctx, p, t)",
        ),
    ] {
        writeln!(out, "fn {name}(row: Row, {args}) -> {} {{ let ctx = tier_stage(row, kp.n_rows * STRIDE); switch tier_meta(ctx.base, 1u) {{",
            if name == "loft_edit_normal_ao" { "vec4f" } else { "f32" }).unwrap();
        writeln!(out, "case 0u: {{ return {ret}; }}").unwrap();
        for (i, (suffix, _, eval)) in evals.iter().enumerate() {
            let setup = if i < 2 {
                "tier_field(ctx, 6u)".to_owned()
            } else {
                format!("tier_tree_{}(ctx)", i - 2)
            };
            let call = match name {
                "q_trace" => eval.replace("{point}", "point"),
                "loft_edit_march" => {
                    format!("tier_march_{suffix}(shape, ro, rd, span, row.loft_lip)")
                }
                _ => format!("tier_shade_{suffix}(shape, p, t)"),
            };
            writeln!(
                out,
                "case {}u: {{ let shape = {setup}; return {call}; }}",
                i + 1
            )
            .unwrap();
        }
        let invalid = if name == "loft_edit_normal_ao" {
            "vec4f(bitcast<f32>(0x7fc00000u))"
        } else {
            "bitcast<f32>(0x7fc00000u)"
        };
        writeln!(out, "default: {{ return {invalid}; }} }} }}").unwrap();
    }
    Ok(out)
}

fn tree_expression(
    chars: &mut impl Iterator<Item = u8>,
    leaves: &mut usize,
    joins: &mut usize,
) -> String {
    match chars.next().expect("prewarmed tree shape") {
        b'x' => {
            let field = *leaves;
            *leaves += 1;
            format!("tier_field_distance(s.f{field}, p)")
        }
        b'(' => {
            let a = tree_expression(chars, leaves, joins);
            let b = tree_expression(chars, leaves, joins);
            assert_eq!(chars.next(), Some(b')'));
            let join = *joins;
            *joins += 1;
            format!("tier_join({a}, {b}, (s.mask & {}u) != 0u)", 1 << join)
        }
        _ => unreachable!("prewarmed tree alphabet"),
    }
}

fn emit_queries(
    out: &mut String,
    suffix: &str,
    arg: &str,
    march: &str,
    shade: &str,
    stage: &str,
    fused: bool,
) {
    let march = march.replace("{point}", "ro + rd * t");
    let setup = if stage.is_empty() {
        String::new()
    } else {
        format!("{stage}(ctx);")
    };
    let setup_cached = if stage.is_empty() {
        String::new()
    } else {
        format!("{stage}_cached(ctx);")
    };
    write!(
        out,
        r"
fn tier_march_{suffix}({arg}, ro: vec3f, rd: vec3f, span: vec2f, lip: f32) -> f32 {{
    {setup}
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
    {setup_cached}
    let e = 5e-4 * max(1.0, 0.25 * t);
    var g = vec3f(0.0);
"
    )
    .unwrap();

    // A fused field has no sample scratch bank. Issue its independent taps
    // before the ordered reduction, then take the AO taps along that normal.
    if fused {
        for i in 0..4 {
            let call = shade.replace("{point}", &format!("p + LOFT_TETRA[{i}] * e"));
            writeln!(out, "    let d{i} = {call};").unwrap();
        }
        for i in 0..4 {
            writeln!(out, "    g += LOFT_TETRA[{i}] * d{i};").unwrap();
        }
    } else {
        let call = shade.replace("{point}", "p + LOFT_TETRA[i] * e");
        writeln!(
            out,
            "    for (var i = 0u; i < 4u; i++) {{ g += LOFT_TETRA[i] * {call}; }}"
        )
        .unwrap();
    }
    out.push_str(
        "    let n = normalize(g);
    var occ = 0.0;
    var w = 1.0;
",
    );
    if fused {
        for i in 0..5 {
            let call = shade.replace("{point}", &format!("p + n * h{i}"));
            writeln!(
                out,
                "    let h{i} = 0.02 + 0.06 * f32({i}u); let ao{i} = {call};"
            )
            .unwrap();
        }
        for i in 0..5 {
            writeln!(out, "    occ += (h{i} - ao{i}) * w; w *= 0.8;").unwrap();
        }
    } else {
        let call = shade.replace("{point}", "p + n * h");
        writeln!(out, "    for (var i = 0u; i < 5u; i++) {{ let h = 0.02 + 0.06 * f32(i); occ += (h - {call}) * w; w *= 0.8; }}").unwrap();
    }
    out.push_str(
        "    return vec4f(n, clamp(1.0 - 2.0 * occ, 0.0, 1.0));
}
",
    );
}

struct Lower<'a, 'p> {
    prog: &'p Prog<'a>,
    types: &'p [Ty],
    fields: BTreeMap<&'a str, u16>,
    row: &'a str,
    certificates: &'a Certificates,
    code: Vec<Instruction>,
    constants: Vec<[f32; 4]>,
    proof_constants: Vec<Option<[f64; 4]>>,
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

    fn constant(&mut self, proof: [f64; 4], span: Span) -> Res<u32> {
        let value = proof.map(|x| x as f32);
        if self.constants.len() == CONSTANTS || !value.into_iter().all(f32::is_finite) {
            return Err(unsupported(span, "trace tape constant limit"));
        }
        let i = self.constants.len() as u16;
        self.constants.push(value);
        self.proof_constants.push(Some(proof));
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
                    let start = self.constant([f64::from(lo); 4], *span)?;
                    let index = self.push(Op::Mov, &[start], 0, *span)?;
                    let one = self.constant([1.; 4], *span)?;
                    let count = self.constant([f64::from(trips); 4], *span)?;
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
                self.constant([value; 4], span)
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
            ExprKind::Bool(value) => self.constant([if *value { 1. } else { 0. }; 4], span),
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
                    let mask =
                        self.constant(std::array::from_fn(|j| if j == i { 1. } else { 0. }), span)?;
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
