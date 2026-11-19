//! Checked E+ expressions to the render-kind tape. Layout changes and
//! unrepresentable queries need a fresh Tier 1 pipeline.
use crate::{Compiled, Prog};
use loft_abi::Tier0Opcode as Op;
use loft_ir::{Code, Diag, Res, Span};
use loft_parse::ast::*;
use std::collections::BTreeMap;
use std::fmt::Write;

use super::{Instruction, MAX_WORDS, VM_WGSL, allocate};

const CONSTANTS: usize = 2048;
pub const TAPE_BYTES: u64 = ((1 + MAX_WORDS + CONSTANTS * 4) * 4) as u64;

pub struct TraceEdit {
    words: Vec<u32>,
    constants: Vec<[f32; 4]>,
    row_reader: String,
}

fn unsupported(span: Span, reason: &str) -> Diag {
    Diag::new(Code::E405, span, reason, "use Tier 1 for this query")
}

impl TraceEdit {
    pub fn bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(TAPE_BYTES as usize);
        bytes.extend((self.words.len() as u32).to_le_bytes());
        bytes.extend(self.words.iter().flat_map(|w| w.to_le_bytes()));
        bytes.resize((1 + MAX_WORDS) * 4, 0);
        bytes.extend(
            self.constants
                .iter()
                .flatten()
                .flat_map(|w| w.to_le_bytes()),
        );
        bytes
    }

    /// Normalise the dirty query so the caller can verify the rest of the
    /// shader is unchanged before uploading the edited tape.
    pub fn shader(&self, tier1: &str) -> Result<String, String> {
        let start = tier1.find("\nfn q_trace(").ok_or("missing trace shader")?;
        let open = tier1[start..].find('{').ok_or("missing trace function")? + start;
        let mut depth = 1;
        let end = tier1[open + 1..]
            .bytes()
            .position(|c| {
                match c {
                    b'{' => depth += 1,
                    b'}' => depth -= 1,
                    _ => {}
                }
                depth == 0
            })
            .ok_or("unclosed trace function")?
            + open
            + 2;
        // Keep the interpreter's statements only. The render query supplies its
        // own typed row and point arguments.
        let vm = VM_WGSL
            .split_once("fn loft_tier0(row: u32) -> vec4f {")
            .unwrap()
            .1
            .split_once("@compute ")
            .unwrap()
            .0;
        let (head, cases) = vm.split_once("        switch ins >> 26u {\n").unwrap();
        let (cases, tail) = cases.split_once("            default:").unwrap();
        // The render query uses its existing row resource, plus invocation
        // arguments. Omit VM cases whose resources cannot be bound there.
        let allow = |id| matches!(id, 1..=11 | 13..=16 | 20..=22 | 28..=33 | 35..=37 | 55);
        let mut vm = format!("{head}        switch ins >> 26u {{\n");
        for line in cases.lines() {
            if let Some(id) = line
                .trim_start()
                .strip_prefix("case ")
                .and_then(|s| s.split_once("u:"))
                .and_then(|(id, _)| id.parse::<u32>().ok())
                && allow(id)
            {
                writeln!(vm, "{line}").unwrap();
            }
        }
        vm.push_str("            default:");
        vm.push_str(tail);
        let vm = vm
            .replace("    var trips: array<u32, 32>;\n", "")
            .replace("    var depth = 0u;\n", "")
            .replace("        let c = reg[imm & 31u];\n", "")
            .replace("tier_tape[0]", "bitcast<u32>(rows[base])")
            .replace("tier_tape[pc + 1u]", "bitcast<u32>(rows[base + pc + 1u])")
            .replace("tier_constants[imm]", "loft_edit_constant(base, imm)")
            .replace("tier_rows[row + imm]", "loft_edit_row(row, imm)")
            .replace("tier_input[imm]", "vec4f(point, 0.0)")
            .replace("tier_frame[imm]", "vec4f(0.0)");
        Ok(format!(
            r"{before}
fn loft_edit_constant(base: u32, slot: u32) -> vec4f {{
  let i = base + {const_base}u + slot * 4u;
  return vec4f(rows[i], rows[i + 1u], rows[i + 2u], rows[i + 3u]);
}}
{reader}
fn loft_edit_trace(row: Row, point: vec3f) -> vec4f {{
    let base = kp.n_rows * STRIDE;{vm}
fn q_trace(row: Row, point: vec3f) -> f32 {{ return loft_edit_trace(row, point).x; }}
{after}
",
            before = &tier1[..start],
            const_base = 1 + MAX_WORDS,
            reader = self.row_reader,
            after = &tier1[end..]
        ))
    }
}

pub fn authored_trace(compiled: &Compiled, name: &str) -> Res<TraceEdit> {
    let prog = compiled.prog();
    let kind = prog
        .kinds
        .get(name)
        .ok_or_else(|| unsupported(Span::NONE, "unknown edited kind"))?;
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
    let mut row_reader =
        String::from("fn loft_edit_row(row: Row, slot: u32) -> vec4f {\n  switch slot {\n");
    let mut types = vec![];
    let ck = crate::check::Ck::new(&prog, &mut types);
    for (slot, field) in kind.row.iter().enumerate().take(CONSTANTS) {
        let name = &field.name.name;
        let access = format!("row.r_{name}");
        let value = match ck.resolve(&field.ty, true)? {
            Ty::F32 | Ty::VecN(4) => format!("vec4f({access})"),
            Ty::VecN(2) => format!("vec4f({access}, 0., 0.)"),
            Ty::VecN(3) | Ty::Geo(..) => format!("vec4f({access}, 0.)"),
            _ => continue,
        };
        fields.insert(name.as_str(), slot as u16);
        writeln!(row_reader, "    case {slot}u: {{ return {value}; }}").unwrap();
    }
    row_reader.push_str("    default: { return vec4f(0.); }\n  }\n}\n");
    let mut lower = Lower {
        prog: &prog,
        types: &compiled.types,
        fields,
        row: &row.name.name,
        code: vec![],
        constants: vec![],
        depth: 0,
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
    Ok(TraceEdit {
        words,
        constants: lower.constants,
        row_reader,
    })
}

struct Lower<'a, 'p> {
    prog: &'p Prog<'a>,
    types: &'p [Ty],
    fields: BTreeMap<&'a str, u16>,
    row: &'a str,
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
        let mut operands = [0; 3];
        operands[..args.len()].copy_from_slice(args);
        self.code.push(Instruction {
            op,
            dst,
            args: operands,
            imm,
        });
        Ok(dst)
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
        self.depth += 1;
        for (i, stmt) in f.body.stmts.iter().enumerate() {
            match stmt {
                Stmt::Let {
                    mutable: false,
                    name,
                    value,
                    ..
                } => {
                    vars.insert(&name.name, self.expr(value, &vars)?);
                }
                Stmt::Return(Some(e), _) if i + 1 == f.body.stmts.len() => {
                    let out = self.expr(e, &vars)?;
                    self.depth -= 1;
                    return Ok(out);
                }
                _ => {
                    return Err(unsupported(
                        f.name.span,
                        "trace helper needs immutable lets and a final expression",
                    ));
                }
            }
        }
        Err(unsupported(f.name.span, "trace helper has no result"))
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
            ExprKind::Binary(op, a, b) => {
                if !matches!(
                    self.types[e.id as usize],
                    Ty::F32 | Ty::VecN(_) | Ty::Geo(..)
                ) {
                    return Err(refuse());
                }
                let op = match op {
                    BinOp::Add => Op::Add,
                    BinOp::Sub => Op::Sub,
                    BinOp::Mul => Op::Mul,
                    BinOp::Div => Op::Div,
                    _ => return Err(refuse()),
                };
                let a = self.expr(a, vars)?;
                let b = self.expr(b, vars)?;
                self.push(op, &[a, b], 0, span)
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
                if !matches!(self.types[a.id as usize], Ty::F32 | Ty::AbstractInt) {
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
