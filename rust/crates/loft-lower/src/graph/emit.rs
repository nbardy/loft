use super::{Arena, Expr, InputSpec, LibId, LoopId, Op, ParamSpec, Prim, Row, RowId, Table, TableId, Ty, lit};
use crate::Error;
use std::collections::HashMap;

/// How the backend spells leaves. Rows and tables include their identity for slot binding.
pub trait Bind {
    fn variable(&self, name: &str, ty: Ty) -> String;
    fn input(&self, spec: &InputSpec) -> String;
    fn param(&self, spec: &ParamSpec) -> String;
    fn row(&self, id: RowId, row: &Row, x: &str) -> String;
    fn table(&self, id: TableId, table: &Table, index: &str, lane: u32) -> String;
}

pub struct Emission {
    pub lines: Vec<String>,
    pub out: Vec<String>,
    pub libs: Vec<LibId>,
    pub names: HashMap<Expr, String>,
}

pub const fn wgsl_ty(ty: Ty) -> &'static str {
    match ty {
        Ty::F32 => "f32",
        Ty::V2 => "vec2<f32>",
        Ty::V3 => "vec3<f32>",
        Ty::V4 => "vec4<f32>",
        Ty::Bool => "bool",
        Ty::U32 => "u32",
        Ty::Kind(_) => unreachable!(),
    }
}

/// mix and select can take one scalar branch. WGSL requires both branches to have the result's type.
fn splat(fn_: Prim) -> bool {
    use Prim::*;
    matches!(fn_, Min | Max | Pow | Atan2 | Step | Clamp | Smoothstep | Mix | Select | Rem)
}

fn infix(fn_: Prim) -> Option<&'static str> {
    use Prim::*;
    Some(match fn_ {
        Add => "+",
        Sub => "-",
        Mul => "*",
        Div => "/",
        Rem => "%",
        Lt => "<",
        Gt => ">",
        Le => "<=",
        Ge => ">=",
        Eq => "==",
        Ne => "!=",
        And => "&&",
        Or => "||",
        Neg | Min | Max | Pow | Mod | Atan2 | Step | Abs | Sign | Floor | Fract | Sqrt | Exp | Log | Sin | Cos
        | Tan | Saturate | Normalize | Mix | Clamp | Smoothstep | Length | Distance | Dot | Cross | Not | Select
        | Ceil | Trunc | RoundEven | InverseSqrt | Exp2 | Log2 | Asin | Acos | Atan | Sinh | Cosh | Tanh => {
            return None;
        }
    })
}

impl Arena {
    pub fn wgsl_node<'a>(
        &self,
        e: Expr,
        reference: impl Fn(Expr) -> &'a str,
        bind: &impl Bind,
    ) -> Result<String, Error> {
        let n = &self[e];
        let args = |xs: &[Expr]| xs.iter().copied().map(&reference).collect::<Vec<_>>().join(", ");
        let ty = wgsl_ty(n.ty);
        Ok(match &n.op {
            Op::Const(v) if n.ty == Ty::Bool => (v[0] != 0.0).to_string(),
            Op::Const(v) if n.ty == Ty::F32 => lit::lit(v[0]),
            Op::Const(v) => format!("{ty}({})", v.iter().copied().map(lit::lit).collect::<Vec<_>>().join(", ")),
            Op::Var(name) => bind.variable(name, n.ty),
            Op::Input(spec) => bind.input(spec),
            Op::Param(spec) => bind.param(spec),
            Op::Row { row, x } => bind.row(*row, self.row_at(*row), reference(*x)),
            Op::Vec(xs) => format!("{ty}({})", args(xs)),
            Op::Get { a, sel } => format!("{}.{sel}", reference(*a)),
            Op::Lib { fun, args: xs } => format!("{}({})", self.lib_fn(*fun).name, args(xs)),
            Op::Fetch { table, index, lane } => bind.table(*table, self.table(*table), reference(*index), *lane),
            Op::Iterate { .. } => {
                return Err(Error::Internal("internal: a loop is emitted as statements, not as an expression".into()));
            }
            Op::Call { prim, args } => {
                let a: Vec<_> = args
                    .iter()
                    .map(|&e| {
                        let src = reference(e);
                        if splat(*prim) && n.ty != Ty::F32 && self[e].ty == Ty::F32 {
                            std::borrow::Cow::Owned(format!("{ty}({src})"))
                        } else {
                            std::borrow::Cow::Borrowed(src)
                        }
                    })
                    .collect();
                if let Some(op) = infix(*prim) {
                    format!("({} {op} {})", a[0], a[1])
                } else {
                    use Prim::*;
                    match prim {
                        Neg => format!("(-{})", a[0]),
                        Not => format!("(!{})", a[0]),
                        Mod => format!("({0} - {1} * floor({0} / {1}))", a[0], a[1]),
                        Select => format!("select({}, {}, {})", a[2], a[1], a[0]), // WGSL order is (false, true, condition).
                        RoundEven => format!("round({})", a[0]),
                        Min | Max | Pow | Atan2 | Step | Abs | Sign | Floor | Fract | Sqrt | Exp | Log | Sin | Cos
                        | Tan | Saturate | Normalize | Mix | Clamp | Smoothstep | Length | Distance | Dot | Cross
                        | Ceil | Trunc | InverseSqrt | Exp2 | Log2 | Asin | Acos | Atan | Sinh | Cosh | Tanh => {
                            format!("{}({})", prim.name(), a.join(", "))
                        }
                        Add | Sub | Mul | Div | Rem | Lt | Gt | Le | Ge | Eq | Ne | And | Or => {
                            unreachable!("infix above")
                        }
                    }
                }
            }
        })
    }

    /// One let per shared node and one for loop per iterate. Pre names are values an earlier emission computed,
    /// such as the loop invariants a scan hoisted.
    pub fn emit_wgsl(
        &self,
        roots: &[Expr],
        bind: &impl Bind,
        prefix: &str,
        pre: &HashMap<Expr, String>,
        comment: Option<&dyn Fn(Expr) -> Option<String>>,
    ) -> Result<Emission, Error> {
        let mut emit = Emitter { arena: self, bind, prefix, comment, names: pre.clone(), next_let: 0, next_loop: 0 };
        let mut lines = Vec::new();
        emit.emit(&self.topo(roots), &mut lines)?;
        let out = roots.iter().map(|&e| emit.reference(e).to_owned()).collect();
        // The caller places these definitions once at module scope.
        Ok(Emission { lines, out, libs: self.libs_of(roots)?, names: emit.names })
    }
}

struct Emitter<'a, B> {
    arena: &'a Arena,
    bind: &'a B,
    prefix: &'a str,
    comment: Option<&'a dyn Fn(Expr) -> Option<String>>,
    names: HashMap<Expr, String>,
    next_let: usize,
    next_loop: usize,
}

impl<B: Bind> Emitter<'_, B> {
    fn reference(&self, e: Expr) -> &str {
        self.names.get(&e).map_or("undefined", String::as_str)
    }

    fn emit(&mut self, list: &[Expr], lines: &mut Vec<String>) -> Result<(), Error> {
        for &e in list {
            if self.names.contains_key(&e) {
                continue;
            }
            let n = &self.arena[e];
            if let Op::Iterate { lo, .. } = n.op {
                self.emit_loop(lo, lines)?;
                continue;
            }
            let src = self.arena.wgsl_node(e, |x| self.reference(x), self.bind)?;
            if matches!(n.op, Op::Const(_) | Op::Var(_)) {
                self.names.insert(e, src);
            } else {
                let name = format!("{}{}", self.prefix, self.next_let);
                self.next_let += 1;
                let ty = wgsl_ty(n.ty);
                let mut line = format!("let {name}: {ty} = {src};");
                if let Some(note) = self.comment.and_then(|f| f(e)) {
                    // A block comment: some emitters (compute.ts) join statements onto one line.
                    line.push_str(&format!("   /* {} */", note.replace("*/", "* /")));
                }
                lines.push(line);
                self.names.insert(e, name);
            }
        }
        Ok(())
    }

    fn emit_loop(&mut self, lo: LoopId, lines: &mut Vec<String>) -> Result<(), Error> {
        let l = self.arena.loop_at(lo);
        let name = format!("{}L{}", self.prefix, self.next_loop);
        self.next_loop += 1;
        for (j, &e) in l.init.iter().enumerate() {
            lines.push(format!("var {name}s{j}: {} = {};", wgsl_ty(self.arena[e].ty), self.reference(e)));
        }
        lines.push(format!("for (var {name}i = 0u; {name}i < u32({}); {name}i++) {{", self.reference(l.count)));
        for (j, v) in l.state.iter().enumerate() {
            self.names.insert(*v, format!("{name}s{j}"));
        }
        self.names.insert(l.index, format!("f32({name}i)"));
        let mut body = Vec::new();
        self.emit(&l.inner, &mut body)?;
        // Read all next values before assigning any, since a step can swap two state variables.
        for (j, &e) in l.next.iter().enumerate() {
            body.push(format!("let {name}n{j}: {} = {};", wgsl_ty(self.arena[e].ty), self.reference(e)));
        }
        // The next-state lets have their own names but still occupy emission ordinals.
        self.next_let += l.next.len();
        for j in 0..l.next.len() {
            body.push(format!("{name}s{j} = {name}n{j};"));
        }
        if let Some(stop) = l.stop {
            body.push(format!("if ({}) {{ break; }}", self.reference(stop)));
        }
        lines.extend(body.into_iter().map(|line| format!("  {line}")));
        lines.push("}".into());
        // The body's names are scoped to the loop.
        for e in l.inner.iter().chain(&l.state).chain([&l.index]) {
            self.names.remove(e);
        }
        for (j, &e) in l.results.iter().enumerate() {
            self.names.insert(e, format!("{name}s{j}"));
        }
        Ok(())
    }
}
