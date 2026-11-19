//! Import the recorder's decoded job. The decoder checked references, arity and topological order.

use super::{Arena, Expr, LibFn, LibId, Node as ExprNode, Op as ExprOp, Row, RowId, Table, TableId, Ty, fresh};
use crate::Error;
use loft_ir::graph::{Graph, Kernel, Op, OutShape, Read};

#[derive(Clone, Copy, Debug)]
pub enum KernelRef {
    Row(RowId),
    Table(TableId),
    Compute,
}

pub struct Imported {
    pub arena: Arena,
    /// The expression for each wire node, including any function or table body it belongs to.
    pub nodes: Vec<Expr>,
    pub roots: Vec<Expr>,
    pub kernels: Vec<KernelRef>,
}

impl Arena {
    pub fn from_wire(graph: &Graph) -> Result<Imported, Error> {
        let mut arena = Self::default();
        let kernels = graph
            .kernels
            .iter()
            .map(|k| {
                Ok(match *k {
                    Kernel::Rows { label, at, len } => {
                        let samples =
                            graph.rows[at as usize..(at + len) as usize].iter().copied().map(f32::from_bits).collect();
                        KernelRef::Row(arena.add_row(Row { label: graph.sym(label).into(), samples })?)
                    }
                    Kernel::Table { name, lanes, entries, role, .. } => {
                        KernelRef::Table(arena.add_table(Table { name: graph.sym(name).into(), lanes, entries, role }))
                    }
                    Kernel::Compute(_) => KernelRef::Compute,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        for raw in &graph.raws {
            let name = graph.sym(raw.name);
            arena.add_lib(LibFn {
                name: name.into(),
                ret: raw.ret,
                wgsl: graph.sym(raw.text).into(),
                deps: raw.deps.iter().map(|&d| LibId(d as usize)).collect(),
                reads: raw.reads.clone(),
                js: None,
                raw_compute: raw_compute(graph, name, &raw.reads),
            });
        }
        let mut nodes = Vec::with_capacity(graph.nodes.len());
        let mut loops: Vec<Option<Vec<Expr>>> = vec![None; graph.loops.len()];
        let mut args = Vec::new();
        for node in &graph.nodes {
            let get = |id: u32| nodes[id as usize];
            let number = |id: u32| f64::from_bits(graph.lits[id as usize]);
            let numbers = |id, ty: Ty| (0..u32::from(ty.lits())).map(|k| number(id + k)).collect::<Vec<_>>();
            args.clear();
            if !matches!(node.op, Op::Lit(_)) {
                args.extend(graph.args_of(node).iter().map(|&id| get(id)));
            }
            let e = match node.op {
                Op::Lit(at) => arena.constant(node.ty, &numbers(at, node.ty))?,
                Op::Time => arena.input("time", Ty::F32, &[0.0])?,
                Op::Param(id) => {
                    let p = graph.params[id as usize];
                    arena.param(graph.sym(p.name), number(p.def), Some(number(p.min)), Some(number(p.max)))
                }
                Op::Input(id) => {
                    let p = graph.inputs[id as usize];
                    arena.input(graph.sym(p.name), p.ty, &numbers(p.def, p.ty))?
                }
                // The recorder renames bound variables on the wire. Their identity is the node, even if a free
                // variable happens to spell its own name the same way.
                Op::Bind(name) => {
                    fresh(&mut arena.nodes, ExprNode { ty: node.ty, op: ExprOp::Var(graph.sym(name).into()) })
                }
                Op::Var(name) => arena.variable(graph.sym(name), node.ty),
                Op::Call(id) => {
                    let f = &graph.fns[id as usize];
                    let by = f.binders.iter().map(|&id| get(id)).zip(args.iter().copied()).collect();
                    arena.replace(&[get(f.body)], &by)?[0]
                }
                Op::Fetch { kernel, lane } => match kernels[kernel as usize] {
                    KernelRef::Row(id) => arena.row(id, args[0])?,
                    KernelRef::Table(id) => arena.fetch(id, args[0], u32::from(lane))?,
                    KernelRef::Compute => unreachable!("wire decoder validates fetch kernels"),
                },
                Op::Loop { decl, k } => {
                    let saved = &mut loops[decl as usize];
                    if saved.is_none() {
                        let l = &graph.loops[decl as usize];
                        let init: Vec<_> = l.init.iter().map(|&id| get(id)).collect();
                        let state = l.state.iter().map(|&id| get(id)).collect();
                        let next = l.next.iter().map(|&id| get(id)).collect();
                        *saved = Some(arena.finish_loop(
                            get(l.count).into(),
                            &init,
                            state,
                            get(l.index),
                            next,
                            l.stop.map(get),
                        )?);
                    }
                    saved.as_ref().expect("imported loop")[usize::from(k)]
                }
                Op::Raw(id) => arena.lib(LibId(id as usize), &args)?,
                Op::Vec => arena.vec_of(node.ty, &args)?,
                Op::Swizzle(sel) => {
                    let text: String =
                        (0..sel.len).map(|k| char::from(b"xyzw"[usize::from((sel.comps >> (2 * k)) & 3)])).collect();
                    arena.get(args[0], &text)?
                }
                Op::Prim(prim) => arena.prim(prim, &args)?,
                Op::Grad => return Err(Error::Unported { module: "wire Grad", lane: "G2" }),
                Op::Field(_) | Op::Make(_) | Op::Open(_) => {
                    return Err(Error::Unported { module: "wire typed records in src/expr.ts", lane: "G1" });
                }
            };
            nodes.push(e);
        }
        let roots = graph.roots.iter().map(|&id| nodes[id as usize]).collect();
        Ok(Imported { arena, nodes, roots, kernels })
    }
}

/// The legacy wire has no raw-compute flag. The handle in compute.ts names the sole output it reads and its shape.
fn raw_compute(graph: &Graph, name: &str, reads: &[Read]) -> Option<String> {
    let [Read::Output { kernel, output }] = *reads else { return None };
    let Kernel::Compute(c) = &graph.kernels[kernel as usize] else {
        unreachable!("wire decoder validates output reads")
    };
    let out = &c.writes[output as usize];
    let suffix = match out.shape {
        OutShape::Buffer { .. } => "_fetch",
        OutShape::Texture { .. } => "_sample",
    };
    (name.strip_suffix(suffix) == Some(graph.sym(out.symbol))).then(|| graph.sym(out.key).into())
}
