//! The closed-root projection of the port job. It feeds the same Arena reader as the lowering.
use super::{arena::probe_arena, has_cpu_twin};
use crate::{
    Error,
    graph::{Arena, Expr, LibFn, LibId},
    input::Input,
};
use loft_ir::graph::{Graph, Op, Ty};
use std::collections::HashMap;

fn literal(graph: &Graph, start: u32, ty: Ty) -> Vec<f64> {
    graph.lits[start as usize..start as usize + usize::from(ty.lits())]
        .iter()
        .copied()
        .map(f64::from_bits)
        .collect()
}

struct ClosedGraph<'a> {
    graph: &'a Graph,
    arena: Arena,
    libs: HashMap<u32, LibId>,
}

impl ClosedGraph<'_> {
    fn lib(&mut self, id: u32) -> LibId {
        if let Some(&lib) = self.libs.get(&id) {
            return lib;
        }
        let graph = self.graph;
        let raw = &graph.raws[id as usize];
        let lib = self.arena.add_lib(LibFn {
            name: graph.sym(raw.name).to_owned(),
            ret: raw.ret,
            wgsl: graph.sym(raw.text).to_owned(),
            deps: Box::new([]),
            reads: raw.reads.clone(),
            js: None,
            raw_compute: None,
        });
        self.libs.insert(id, lib);
        lib
    }

    fn read(&mut self, id: u32, known: &mut HashMap<u32, Expr>) -> Result<Expr, Error> {
        if let Some(&e) = known.get(&id) {
            return Ok(e);
        }
        let graph = self.graph;
        let n = &graph.nodes[id as usize];
        let args = graph.args_of(n).iter().map(|&arg| self.read(arg, known)).collect::<Result<Vec<_>, _>>()?;
        let e = match n.op {
            Op::Lit(i) => self.arena.constant(n.ty, &literal(graph, i, n.ty))?,
            Op::Time => self.arena.input("time", Ty::F32, &[0.0])?,
            Op::Input(i) => {
                let s = graph.inputs[i as usize];
                self.arena.input(graph.sym(s.name), s.ty, &literal(graph, s.def, s.ty))?
            }
            Op::Param(i) => {
                let p = graph.params[i as usize];
                let number = |i: u32| f64::from_bits(graph.lits[i as usize]);
                self.arena.param(graph.sym(p.name), number(p.def), Some(number(p.min)), Some(number(p.max)))
            }
            Op::Raw(i) => {
                let lib = self.lib(i);
                self.arena.lib(lib, &args)?
            }
            Op::Call(i) => {
                let f = &graph.fns[i as usize];
                let mut bound = f.binders.iter().copied().zip(args).collect();
                self.read(f.body, &mut bound)?
            }
            Op::Vec => self.arena.vec_of(n.ty, &args)?,
            Op::Swizzle(s) => {
                let sel: String = (0..s.len).map(|i| char::from(b"xyzw"[usize::from(s.comps >> (2 * i) & 3)])).collect();
                self.arena.get(args[0], &sel)?
            }
            Op::Prim(p) => self.arena.prim(p, &args)?,
            Op::Bind(_) => return Err(Error::Input("compileJs: an unbound binder reached the CPU reader".into())),
            Op::Grad | Op::Field(_) | Op::Make(_) | Op::Open(_) => {
                return Err(Error::Unported { module: "src/expr.ts compileJs: wire-only graph operation", lane: "G2" });
            }
            Op::Var(_) | Op::Fetch { .. } | Op::Loop { .. } => {
                return Err(Error::Internal("compileJs: an open node reached the closed-root probe".into()));
            }
        };
        known.insert(id, e);
        Ok(e)
    }
}

/// `probe.eval`: the closed roots in recorder order, evaluated at rest.
pub fn probe(input: &Input) -> Result<String, Error> {
    let graph = input.graph();
    let mut open = vec![false; graph.nodes.len()];
    for (k, n) in graph.nodes.iter().enumerate() {
        open[k] = match n.op {
            Op::Var(_) | Op::Fetch { .. } | Op::Loop { .. } => true,
            Op::Raw(i) => !has_cpu_twin(graph.sym(graph.raws[i as usize].name)),
            Op::Call(f) => open[graph.fns[f as usize].body as usize],
            Op::Lit(_) | Op::Time | Op::Param(_) | Op::Input(_) | Op::Bind(_) | Op::Grad | Op::Field(_) | Op::Make(_)
            | Op::Open(_) | Op::Swizzle(_) | Op::Vec | Op::Prim(_) => false,
        } || graph.args_of(n).iter().any(|&a| open[a as usize]);
    }
    let mut import = ClosedGraph { graph, arena: Arena::default(), libs: HashMap::new() };
    let mut known = HashMap::new();
    let roots = graph.roots.iter().enumerate().filter(|&(_, &id)| !open[id as usize])
        .map(|(k, &id)| Ok((k, import.read(id, &mut known)?)))
        .collect::<Result<Vec<_>, Error>>()?;
    probe_arena(&import.arena, &roots, graph.roots.len())
}
