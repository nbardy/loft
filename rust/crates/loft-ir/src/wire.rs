//! The job wire (plan §3.2), pinned here so the recorder (lane 1.2) and Rust agree. All words little-endian.
//!
//! ```text
//! 0   u32 magic "LFTW", u32 WIRE, u32 program (Program order), u32 n_spans = 11
//! 16  11 × (u32 byte offset, u32 byte length), in `SPANS` order; each span lies after this table, starts at a
//!     multiple of 8 and holds whole elements
//!     nodes      u32×4  [op:8 | ty:8 | flags:8 | 0, args offset, n args, imm]   (op codes: graph::Op)
//!     args       u32    operand node ids; each below its node's own id
//!     lits       f64    authored numbers, exact bits, finite
//!     rows       f32    supplied samples          blobs  u8   supplied binary assets
//!     syms       u8     UTF-8 text of every symbol, concatenated
//!     sym_ends   u32    end byte of each symbol in syms (sym i = syms[end(i-1)..end(i)])
//!     sites      u32×3  (file sym, line, col)
//!     node_sites u32    one per node: 0 none, k = sites[k-1]
//!     decls      u32×4  [tag:8 | arity:24, name sym or u32::MAX, roots offset, n roots]   (tags: graph::Tag)
//!     roots      u32    node ids
//! ```
//! Leaf flags: 0 time, 1 param (imm = Param decl), 2 input (imm = Input decl), 3 binder (imm = binder id).
//! Fetch: imm = Kernel decl, flags = lane. Loop: imm = Loop decl, flags = component. Raw: imm = source sym.
//! Swizzle: imm = len (bits 0..3, 1..=4) | comps << 3 (2 bits each). Lit: imm = first lit; a vector takes width lits.
use crate::error::{Error, Fault, Internal, Meter, SceneFault, Span, WireFault};
use crate::graph::{Decl, Graph, Leaf, Node, Op, Prim, Program, Sel, Site, Tag, Ty};

pub const MAGIC: u32 = u32::from_le_bytes(*b"LFTW");
/// Internal, never persisted: the recorder and this decoder change together. The binding reports it at load.
pub const WIRE: u32 = 1;
/// Quota checked before any pass: 16.7M nodes, 30x Titan's recorded count.
pub const MAX_NODES: u64 = 1 << 24;

const SPANS: [(Span, usize); 11] = [
    (Span::Nodes, 16),
    (Span::Args, 4),
    (Span::Lits, 8),
    (Span::Rows, 4),
    (Span::Blobs, 1),
    (Span::Syms, 1),
    (Span::SymEnds, 4),
    (Span::Sites, 12),
    (Span::NodeSites, 4),
    (Span::Decls, 16),
    (Span::Roots, 4),
];
const TABLE_END: usize = 16 + 8 * SPANS.len();

/// A decoded job: the program asked for and its validated graph.
#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    pub program: Program,
    pub graph: Graph,
}

fn bad(span: Span, at: usize, what: Fault) -> Error {
    Error::Internal(Internal::Wire(WireFault { span, at: at as u64, what }))
}

fn words(b: &[u8]) -> Box<[u32]> {
    b.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c)).collect()
}

/// A `len`-element range at `off` inside a span of `total` elements, or a `Ref` fault.
fn within(off: u32, len: u32, total: usize, span: Span, at: usize) -> Result<(), Error> {
    match off.checked_add(len) {
        Some(end) if end as usize <= total => Ok(()),
        Some(_) | None => Err(bad(span, at, Fault::Ref { found: off })),
    }
}

/// Decodes and validates a job: every bound, tag, quota and type rule, before any pass sees it.
pub fn decode(buf: &[u8]) -> Result<Job, Error> {
    let head = |i: usize| buf.get(4 * i..4 * i + 4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]));
    let h = |i: usize| head(i).ok_or_else(|| bad(Span::Header, 0, Fault::Truncated));
    (h(0)? == MAGIC).then_some(()).ok_or_else(|| bad(Span::Header, 0, Fault::Magic))?;
    let wire = h(1)?;
    (wire == WIRE).then_some(()).ok_or_else(|| bad(Span::Header, 1, Fault::Version { found: wire }))?;
    let code = h(2)?;
    let program = [Program::Scene, Program::Solve, Program::Evaluate, Program::Check]
        .get(code as usize)
        .copied()
        .ok_or_else(|| bad(Span::Header, 2, Fault::Program { found: code }))?;
    let n = h(3)?;
    (n as usize == SPANS.len()).then_some(()).ok_or_else(|| bad(Span::Header, 3, Fault::SpanCount { found: n }))?;
    let mut spans: [&[u8]; 11] = [&[]; 11];
    for (i, (span, size)) in SPANS.iter().enumerate() {
        let (off, len) = (h(4 + 2 * i)? as usize, h(5 + 2 * i)? as usize);
        let fits =
            off >= TABLE_END && off % 8 == 0 && len % size == 0 && off.checked_add(len).is_some_and(|e| e <= buf.len());
        spans[i] = fits.then(|| &buf[off..off + len]).ok_or_else(|| bad(*span, 0, Fault::Bounds))?;
    }
    let [nodes_b, args_b, lits_b, rows_b, blobs_b, syms_b, ends_b, sites_b, nsites_b, decls_b, roots_b] = spans;

    let n_nodes = nodes_b.len() / 16;
    if n_nodes as u64 > MAX_NODES {
        return Err(Error::Budget { meter: Meter::Nodes, limit: MAX_NODES, spent: n_nodes as u64 });
    }
    let args = words(args_b);
    let lits: Box<[u64]> = lits_b.as_chunks::<8>().0.iter().map(|c| u64::from_le_bytes(*c)).collect();
    if let Some(i) = lits.iter().position(|b| !f64::from_bits(*b).is_finite()) {
        return Err(bad(Span::Lits, i, Fault::Lit));
    }
    let syms = std::str::from_utf8(syms_b).map_err(|e| bad(Span::Syms, e.valid_up_to(), Fault::Utf8))?;
    let sym_ends = words(ends_b);
    let mut prev = 0u32;
    for (i, e) in sym_ends.iter().enumerate() {
        if *e < prev || !syms.is_char_boundary(*e as usize) {
            return Err(bad(Span::SymEnds, i, Fault::Length));
        }
        prev = *e;
    }
    let n_syms = sym_ends.len();
    let sym = |i: u32, span: Span, at: usize| -> Result<u32, Error> {
        ((i as usize) < n_syms).then_some(i).ok_or_else(|| bad(span, at, Fault::Sym { found: i }))
    };
    let site_words = words(sites_b);
    let sites = site_words
        .as_chunks::<3>()
        .0
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let f = sym(w[0], Span::Sites, i)?;
            let start = f.checked_sub(1).map_or(0, |p| sym_ends[p as usize]);
            Ok(Site { file: syms[start as usize..sym_ends[f as usize] as usize].into(), line: w[1], col: w[2] })
        })
        .collect::<Result<Box<[Site]>, Error>>()?;
    let node_sites = words(nsites_b);
    if node_sites.len() != n_nodes {
        return Err(bad(Span::NodeSites, node_sites.len(), Fault::Length));
    }
    if let Some(i) = node_sites.iter().position(|k| *k as usize > sites.len()) {
        return Err(bad(Span::NodeSites, i, Fault::Ref { found: node_sites[i] }));
    }
    let roots = words(roots_b);
    if let Some(i) = roots.iter().position(|r| *r as usize >= n_nodes) {
        return Err(bad(Span::Roots, i, Fault::Ref { found: roots[i] }));
    }
    let decl_words = words(decls_b);
    let decls = decl_words
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let tag = *Tag::ALL
                .get((w[0] & 0xff) as usize)
                .ok_or_else(|| bad(Span::Decls, i, Fault::Tag { found: w[0] & 0xff }))?;
            let name = match w[1] {
                u32::MAX => None,
                s => Some(sym(s, Span::Decls, i)?),
            };
            within(w[2], w[3], roots.len(), Span::Decls, i)?;
            Ok(Decl { tag, arity: w[0] >> 8, name, roots: w[2], n: w[3] })
        })
        .collect::<Result<Box<[Decl]>, Error>>()?;

    let decl_of = |imm: u32, want: Tag, at: usize| -> Result<&Decl, Error> {
        let d = decls.get(imm as usize).ok_or_else(|| bad(Span::Nodes, at, Fault::Ref { found: imm }))?;
        (d.tag == want).then_some(d).ok_or_else(|| bad(Span::Nodes, at, Fault::Tag { found: imm }))
    };
    let node_words = words(nodes_b);
    let mut nodes: Vec<Node> = Vec::with_capacity(n_nodes);
    for (i, w) in node_words.as_chunks::<4>().0.iter().enumerate() {
        let (code, ty_code, flags) = ((w[0] & 0xff) as u8, ((w[0] >> 8) & 0xff) as u8, ((w[0] >> 16) & 0xff) as u8);
        if w[0] >> 24 != 0 {
            return Err(bad(Span::Nodes, i, Fault::Flags { found: (w[0] >> 24) as u8 }));
        }
        let ty = Ty::from_code(ty_code).ok_or_else(|| bad(Span::Nodes, i, Fault::Ty { code: ty_code }))?;
        let (a, n, imm) = (w[1], w[2], w[3]);
        within(a, n, args.len(), Span::Nodes, i)?;
        let operands = &args[a as usize..(a + n) as usize];
        if let Some(k) = operands.iter().position(|x| *x as usize >= i) {
            return Err(bad(Span::Args, a as usize + k, Fault::Ref { found: operands[k] }));
        }
        let no_flags =
            |op: Op| if flags == 0 { Ok(op) } else { Err(bad(Span::Nodes, i, Fault::Flags { found: flags })) };
        let arity = |want: bool| if want { Ok(()) } else { Err(bad(Span::Nodes, i, Fault::Arity { found: n })) };
        let op = match code {
            0 => {
                let width = ty
                    .width()
                    .or((ty == Ty::Bool).then_some(1))
                    .ok_or_else(|| bad(Span::Nodes, i, Fault::Ty { code: ty_code }))?;
                within(imm, u32::from(width), lits.len(), Span::Nodes, i)
                    .map_err(|_| bad(Span::Nodes, i, Fault::Lit))?;
                arity(n == 0)?;
                no_flags(Op::Lit(imm))?
            }
            1 => {
                arity(n == 0)?;
                Op::Leaf(match flags {
                    0 => Leaf::Time,
                    1 => decl_of(imm, Tag::Param, i).map(|_| Leaf::Param(imm))?,
                    2 => decl_of(imm, Tag::Input, i).map(|_| Leaf::Input(imm))?,
                    3 => Leaf::Bind(imm),
                    f => return Err(bad(Span::Nodes, i, Fault::Flags { found: f })),
                })
            }
            2 => decl_of(imm, Tag::Fn, i).and_then(|d| arity(n == d.arity)).and_then(|_| no_flags(Op::Call(imm)))?,
            3 => decl_of(imm, Tag::Kernel, i).and(arity(n == 1)).map(|_| Op::Fetch { kernel: imm, lane: flags })?,
            4 => {
                let d = decl_of(imm, Tag::Loop, i)?;
                arity(n == 0)?;
                (u32::from(flags) < d.arity)
                    .then_some(Op::Loop { decl: imm, k: flags })
                    .ok_or_else(|| bad(Span::Nodes, i, Fault::Flags { found: flags }))?
            }
            5 => arity(n == 2).and_then(|_| no_flags(Op::Grad))?,
            6 => sym(imm, Span::Nodes, i).and_then(|_| no_flags(Op::Raw(imm)))?,
            7 => {
                let len = (imm & 7) as u8;
                let ok = (1..=4).contains(&len) && imm >> (3 + 2 * u32::from(len)) == 0;
                ok.then_some(()).ok_or_else(|| bad(Span::Nodes, i, Fault::Arity { found: imm }))?;
                arity(n == 1)?;
                no_flags(Op::Swizzle(Sel { len, comps: (imm >> 3) as u8 }))?
            }
            8..=15 => return Err(bad(Span::Nodes, i, Fault::Op { code })),
            16..=255 => {
                let p =
                    *Prim::ALL.get(usize::from(code - 16)).ok_or_else(|| bad(Span::Nodes, i, Fault::Op { code }))?;
                arity(p.takes(n as usize))?;
                no_flags(Op::Prim(p))?
            }
        };
        nodes.push(Node { op, ty, args: a, n });
    }
    let graph = Graph {
        nodes: nodes.into(),
        args,
        lits,
        rows: words(rows_b),
        blobs: blobs_b.into(),
        syms: syms.into(),
        sym_ends,
        sites,
        node_sites,
        decls,
        roots,
    };
    for (i, d) in graph.decls.iter().enumerate() {
        layout(&graph, d).map_err(|_| bad(Span::Decls, i, Fault::Layout))?;
    }
    for i in 0..graph.nodes.len() {
        typed(&graph, i).map_err(|(want, got)| Error::Scene {
            site: graph.site(i as u32).cloned(),
            fault: SceneFault::Type { want, got },
        })?;
    }
    Ok(Job { program, graph })
}

/// A decl's roots have its kind's layout (`graph::Decl`).
fn layout(g: &Graph, d: &Decl) -> Result<(), ()> {
    let r = g.roots_of(d);
    let ty = |k: usize| r.get(k).map(|n| g.nodes[*n as usize].ty).ok_or(());
    let op = |k: usize| r.get(k).map(|n| g.nodes[*n as usize].op);
    let binder = |k: usize| matches!(op(k), Some(Op::Leaf(Leaf::Bind(_)))).then_some(()).ok_or(());
    let lit = |k: usize| matches!(op(k), Some(Op::Lit(_))).then_some(()).ok_or(());
    let a = d.arity as usize;
    match d.tag {
        Tag::Fn => (r.len() > a).then_some(()).ok_or(()).and((0..a).try_for_each(binder)),
        Tag::Loop => {
            let stop = match r.len().checked_sub(3 * a + 2) {
                Some(0) => false,
                Some(1) => true,
                Some(_) | None => return Err(()),
            };
            (ty(0)? == Ty::F32).then_some(()).ok_or(())?;
            for k in 0..a {
                binder(1 + a + k)?;
                let t = ty(1 + k)?;
                (ty(1 + a + k)? == t && ty(2 + 2 * a + k)? == t).then_some(()).ok_or(())?;
            }
            binder(1 + 2 * a)?;
            (ty(1 + 2 * a)? == Ty::F32 && (!stop || ty(2 + 3 * a)? == Ty::Bool)).then_some(()).ok_or(())
        }
        Tag::Param => (r.len() == 3)
            .then_some(())
            .ok_or(())
            .and((0..3).try_for_each(|k| lit(k).and(ty(k)).and_then(|t| (t == Ty::F32).then_some(()).ok_or(())))),
        Tag::Input => {
            (r.len() == 1).then_some(()).ok_or(()).and(lit(0)).and(ty(0)).and_then(|t| t.width().map(|_| ()).ok_or(()))
        }
        Tag::Kernel | Tag::Draw | Tag::Pass | Tag::Medium | Tag::Check | Tag::Solve | Tag::Camera => Ok(()),
    }
}

/// Node `i`'s recorded type follows its operation's rule. Err is (wanted, given).
fn typed(g: &Graph, i: usize) -> Result<(), (Ty, Ty)> {
    let node = &g.nodes[i];
    let arg = |k: usize| g.nodes[g.args_of(node)[k] as usize].ty;
    let rec = node.ty;
    let want = match node.op {
        Op::Lit(_) => rec,
        Op::Leaf(Leaf::Time | Leaf::Param(_)) => Ty::F32,
        Op::Leaf(Leaf::Input(d)) => g.nodes[g.roots_of(&g.decls[d as usize])[0] as usize].ty,
        Op::Leaf(Leaf::Bind(_)) => rec,
        Op::Prim(p) => {
            // Most operators take at most 4 operands: no allocation per node on a 1M-node job.
            let mut few = [Ty::F32; 4];
            let n = node.n as usize;
            match few.get_mut(..n) {
                Some(slot) => {
                    slot.iter_mut().enumerate().for_each(|(k, t)| *t = arg(k));
                    p.result(slot, rec)?
                }
                None => p.result(&(0..n).map(arg).collect::<Vec<_>>(), rec)?,
            }
        }
        Op::Swizzle(s) => {
            let w = arg(0).width().filter(|w| *w > 1).ok_or((Ty::V4, arg(0)))?;
            let fits = (0..s.len).all(|k| (s.comps >> (2 * k)) & 3 < w);
            let out = Ty::from_code(s.len - 1).ok_or((rec, rec))?;
            if fits { out } else { return Err((Ty::from_code(w - 1).unwrap_or(rec), arg(0))) }
        }
        Op::Call(f) => {
            let d = &g.decls[f as usize];
            let r = g.roots_of(d);
            for (k, binder) in r.iter().take(d.arity as usize).enumerate() {
                let b = g.nodes[*binder as usize].ty;
                (b == arg(k)).then_some(()).ok_or((b, arg(k)))?;
            }
            match &r[d.arity as usize..] {
                [one] => g.nodes[*one as usize].ty,
                _ => match rec {
                    Ty::Rec(_) => rec,
                    Ty::F32 | Ty::V2 | Ty::V3 | Ty::V4 | Ty::Bool => Ty::Rec(0),
                },
            }
        }
        Op::Fetch { .. } => (arg(0) == Ty::F32).then_some(Ty::V4).ok_or((Ty::F32, arg(0)))?,
        Op::Loop { decl, k } => g.nodes[g.roots_of(&g.decls[decl as usize])[1 + usize::from(k)] as usize].ty,
        Op::Grad => match (arg(0), arg(1).width()) {
            (Ty::F32, Some(_)) => arg(1),
            (Ty::F32, None) => return Err((Ty::F32, arg(1))),
            (f, _) => return Err((Ty::F32, f)),
        },
        Op::Raw(_) => rec,
    };
    if want == rec { Ok(()) } else { Err((want, rec)) }
}
