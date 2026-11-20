use super::{C, Compiled, Curve, Env, FetchAt, Leaf, Loop, RowAt, RowSource, Scalar, Step, Value, at, Twin, math};
use crate::{
    Error,
    graph::{Arena, Expr, LoopId, Op, Ty},
};
use std::collections::HashMap;
use std::fmt::Write;

struct Compiler<'a> {
    arena: &'a Arena,
    rows: RowSource<'a>,
    comps: HashMap<Expr, Vec<C>>,
    code: Compiled<'a>,
}

/// Compile expressions to a CPU reader that can run against successive frames.
pub fn compile_js<'a>(arena: &'a Arena, roots: &[Expr], rows: RowSource<'a>) -> Result<Compiled<'a>, Error> {
    let mut compiler = Compiler {
        arena,
        rows,
        comps: HashMap::new(),
        code: Compiled { steps: Vec::new(), tables: Vec::new(), need_curves: false, slots: 0, roots: Vec::new() },
    };
    compiler.emit_list(&arena.topo(roots))?;
    compiler.code.roots = roots.iter().map(|&e| (arena[e].ty, compiler.comps[&e].clone())).collect();
    Ok(compiler.code)
}

impl<'a> Compiler<'a> {
    fn emit_list(&mut self, list: &[Expr]) -> Result<(), Error> {
        for &e in list {
            if self.comps.contains_key(&e) {
                continue;
            }
            if let Op::Iterate { lo, .. } = self.arena[e].op {
                self.loop_(lo)?;
            } else {
                let value = self.node(e)?;
                self.comps.insert(e, value);
            }
        }
        Ok(())
    }

    fn loop_(&mut self, lo: LoopId) -> Result<(), Error> {
        let l = self.arena.loop_at(lo);
        let count = self.comps[&l.count][0];
        let init: Vec<_> = l.init.iter().flat_map(|e| self.comps[e].iter().copied()).collect();
        let mut slots = Vec::with_capacity(init.len());
        for &binder in &l.state {
            let n = usize::from(self.arena[binder].ty.lits());
            let slot = self.code.local(n);
            slots.extend(slot..slot + n);
            self.comps.insert(binder, (slot..slot + n).map(C::Local).collect());
        }
        let index = self.code.local(1);
        self.comps.insert(l.index, vec![C::Local(index)]);
        let outer = std::mem::take(&mut self.code.steps);
        self.emit_list(&l.inner)?;
        let next = l.next.iter().flat_map(|e| self.comps[e].iter().copied()).collect();
        let stop = l.stop.map(|e| self.comps[&e][0]);
        let body = std::mem::replace(&mut self.code.steps, outer);
        for (result, binder) in l.results.iter().zip(&l.state) {
            let comps = self.comps.remove(binder).expect("loop state components");
            self.comps.insert(*result, comps);
        }
        for e in l.inner.iter().chain([&l.index]) {
            self.comps.remove(e);
        }
        self.code.steps.push(Step::Loop(Loop { count, state: slots.into_iter().zip(init).collect(), index, body, next, stop }));
        Ok(())
    }

    fn node(&mut self, e: Expr) -> Result<Vec<C>, Error> {
        let node = &self.arena[e];
        let width = usize::from(node.ty.lits());
        let comps = &self.comps;
        Ok(match &node.op {
            Op::Const(values) => values.iter().map(|&v| {
                C::Known(if node.ty == Ty::Bool { Scalar::Bool(v != 0.0) } else { Scalar::Number(v) })
            }).collect(),
            Op::Var(name) => self.code.emit(width, |dst| Step::Leaf(dst, width, Leaf::Var(name))),
            Op::Input(s) => {
                let mut def = [0.0; 4];
                for (out, &v) in def.iter_mut().zip(&s.def) {
                    *out = math::source_number(v);
                }
                self.code.emit(width, |dst| Step::Leaf(dst, width, Leaf::Input(&s.name, def)))
            }
            Op::Param(s) => self.code.emit(1, |dst| Step::Leaf(dst, 1, Leaf::Param(&s.name, math::source_number(s.def)))),
            Op::Row { row, x } => {
                let source = match self.rows {
                    RowSource::Own => Curve::Own(&self.arena.row_at(*row).samples),
                    RowSource::Table(slot) => {
                        self.code.need_curves = true;
                        Curve::Table(slot(*row) * super::CURVE_N * 4)
                    }
                };
                self.code.emit(4, |dst| Step::Row(dst, source, RowAt::new(comps[x][0])))
            }
            Op::Fetch { table, index, lane } => {
                let t = self.arena.table(*table);
                let slot = self.code.tables.iter().position(|&(id, _)| id == *table).unwrap_or_else(|| {
                    let k = self.code.tables.len();
                    self.code.tables.push((*table, &t.name));
                    k
                });
                self.code.emit(4, |dst| Step::Fetch(dst, slot, FetchAt::new(comps[index][0], t.lanes, *lane)))
            }
            Op::Lib { fun, args } => {
                let fun = self.arena.lib_fn(*fun);
                if let Some(raw) = &fun.raw_compute {
                    // A pose map can compile its optional CPU reader. Refuse only if it runs.
                    self.code.emit(width, |_| Step::RawCompute(raw))
                } else {
                    let args = args.iter().map(|a| comps[a].clone()).collect();
                    if let Some(js) = fun.js {
                        self.code.emit(width, |dst| Step::Library(dst, node.ty, js, args))
                    } else if let Some(twin) = Twin::by_name(&fun.name) {
                        self.code.emit(width, |dst| Step::Twin(dst, width, twin, args))
                    } else {
                        return Err(Error::Scene(format!(
                            "library function \"{}\" is GPU-only and cannot be evaluated on the CPU", fun.name
                        )));
                    }
                }
            }
            Op::Vec(args) => {
                let c: Vec<_> = args.iter().flat_map(|a| comps[a].iter().copied()).collect();
                if c.len() == 1 { vec![c[0]; width] } else { c }
            }
            Op::Get { a, sel } => sel.bytes().map(|ch| {
                let i = b"xyzw".iter().position(|&x| x == ch).expect("validated swizzle");
                at(&comps[a], i)
            }).collect(),
            Op::Call { prim, args } => self.code.call(*prim, &args.iter().map(|a| comps[a].clone()).collect::<Vec<_>>()),
            Op::Iterate { .. } => unreachable!("a loop is emitted as statements"),
        })
    }
}

pub(super) fn probe_arena(arena: &Arena, closed: &[(usize, Expr)], total: usize) -> Result<String, Error> {
    if closed.is_empty() {
        return Ok("closed 0\n".to_owned());
    }
    let subset: Vec<_> = closed.iter().map(|&(_, r)| r).collect();
    let leaves = arena.leaves(&subset)?;
    let env = Env {
        inputs: leaves.inputs.into_iter().map(|s| {
            let v = if s.def.len() == 1 { Value::Number(s.def[0]) } else { Value::Vector(s.def.to_vec()) };
            (s.name.as_str(), v)
        }).collect(),
        params: leaves.params.into_iter().map(|s| (s.name.as_str(), s.def)).collect(),
        ..Env::default()
    };
    let values = compile_js(arena, &subset, RowSource::Own)?.run(&env)?;
    let mut out = format!("closed {} of {}\n", closed.len(), total);
    for ((k, _), v) in closed.iter().zip(values) {
        write!(out, "{k}: ").expect("write to String");
        match v {
            Value::Number(x) => write!(out, "{:016x}", x.to_bits()).expect("write to String"),
            Value::Bool(b) => write!(out, "{b}").expect("write to String"),
            Value::Vector(v) => {
                for (i, x) in v.into_iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write!(out, "{:016x}", x.to_bits()).expect("write to String");
                }
            }
            Value::Undefined => return Err(Error::Internal("a closed root evaluated to undefined".into())),
        }
        out.push('\n');
    }
    Ok(out)
}
