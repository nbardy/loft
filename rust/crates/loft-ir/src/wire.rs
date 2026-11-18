//! The job wire (plan §3.2), as the recorder writes it (src/record/wire.ts `pack`). All words little-endian.
//!
//! ```text
//! 0   u32 magic "LFTW", u32 WIRE, u32 program (Program order), u32 n_spans = 11
//! 16  11 spans in this order, each: u32 byte length, u32 0, the bytes, zeros to a multiple of 8; nothing after
//!     nodes     u32×4  [op:8 | ty:8 | flags:16, args offset, n args, imm]
//!     args      u32    operand node ids; each below its node's own id
//!     lits      f64    authored numbers, exact bits, finite
//!     rows      f32    supplied samples          blobs  u8   supplied binary assets
//!     syms      u8     UTF-8 text of every symbol, concatenated
//!     symAt     u32    n_syms + 1 offsets: symbol k = syms[symAt[k]..symAt[k+1]]; symAt[0] = 0, last = syms length
//!     sites     u32×3  (file sym, line, col)
//!     siteOf    u32    one per node: 0 none, k = sites[k-1]
//!     decls     u32    [program root count R, then per decl: tag, n roots, n words, words...]
//!     roots     u32    node ids: the R program roots, then each decl's roots in decl order
//! ```
//! Ops (`op` byte; flags and imm 0 unless named; a node with no args has args offset 0):
//! ```text
//!   0 Lit    imm = first lit, n = ty.lits()        6 Fetch  imm = Kernel ordinal, flags = lane, args [index f32]
//!   1 Time                                         7 Loop   imm = Loop ordinal, flags = state component
//!   2 Param  imm = Param ordinal                   8 Grad   args [f, x]
//!   3 Input  imm = Input ordinal                   9 Raw    imm = Fn ordinal of a raw function, args its arguments
//!   4 Bind   imm = name sym; flags 1 a binder of   10 Swizzle imm = comps (2 bits each) | len << 8, args [vector]
//!            a loop or graph function, 0 free      11 Vec   args 1..=4 parts
//!   5 Call   imm = Fn ordinal of a graph function  16+i Prim::ALL[i]; 12..=15 refused
//! ```
//! An ordinal counts decls of one tag in decl order. Decls (tag = `Tag` order; words, then roots):
//! ```text
//!   0 Fn      graph [0, name, ret ty, n, binder node × n]   roots [body]
//!             raw   [1, name, ret ty, text sym, n deps, dep Fn ordinal × n deps]  roots []   (each dep a raw Fn before it)
//!   1 Loop    [n, has stop, state binder × n, index binder] roots [count, init × n, next × n, stop?]
//!   2 Kernel  table [0, name, lanes, entries, role]  rows [1, label, rows offset, length]    roots []
//!   8 Param   [name, def lit, min lit, max lit]      9 Input [name, ty, first def lit]       roots []
//!   3 Draw, 4 Pass, 5 Medium, 6 Check, 7 Solve, 10 Camera: no layout pinned yet (lanes 2.1, 2.3, 3.1); refused.
//! ```
//! A Call or Loop node comes after every root and binder of its decl, so the job is acyclic through decls too.
use crate::error::{Error, Fault, Internal, Meter, SceneFault, Span, WireFault};
use crate::graph::{Graph, GraphFn, Input, Kernel, Loop, Node, Op, Param, Prim, Program, RawFn, Role, Sel, Site, Ty};

pub const MAGIC: u32 = u32::from_le_bytes(*b"LFTW");
/// Internal, never persisted: the recorder and this decoder change together. The binding reports it at load.
pub const WIRE: u32 = 1;
/// Quota checked before any span is read: 16.7M nodes, 30x Titan's recorded count.
pub const MAX_NODES: u64 = 1 << 24;
/// Wire code of `Prim::ALL[0]`.
pub const PRIM0: u8 = 16;
/// Node flag of a bound `Bind` (a loop's or graph function's binder); 0 is a free variable.
pub const BOUND: u16 = 1;

const SPANS: [(Span, usize); 11] = [
    (Span::Nodes, 16),
    (Span::Args, 4),
    (Span::Lits, 8),
    (Span::Rows, 4),
    (Span::Blobs, 1),
    (Span::Syms, 1),
    (Span::SymAt, 4),
    (Span::Sites, 12),
    (Span::NodeSites, 4),
    (Span::Decls, 4),
    (Span::Roots, 4),
];

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

/// A `len`-element range at `off` inside `total` elements.
fn fits(off: u32, len: u32, total: usize) -> bool {
    off.checked_add(len).is_some_and(|end| end as usize <= total)
}

/// What an Fn ordinal names: the wire numbers graph and raw functions together, `Graph` keeps them apart.
#[derive(Clone, Copy)]
enum FnSlot {
    Graph(u32),
    Raw(u32),
}

/// The header and the eleven spans, each checked for length, padding and whole elements.
fn spans(buf: &[u8]) -> Result<(Program, [&[u8]; 11]), Error> {
    let word = |at: usize| buf.get(at..at + 4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]));
    let h = |i: usize| word(4 * i).ok_or_else(|| bad(Span::Header, i, Fault::Truncated));
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
    let mut out: [&[u8]; 11] = [&[]; 11];
    let mut at = 16usize;
    for (i, (span, size)) in SPANS.iter().enumerate() {
        let oob = || bad(*span, 0, Fault::Bounds);
        let (len, pad) = (word(at).ok_or_else(oob)? as usize, word(at + 4).ok_or_else(oob)?);
        // The node quota, before any span is read: a length is all it takes.
        if i == 0 && (len / 16) as u64 > MAX_NODES {
            return Err(Error::Budget { meter: Meter::Nodes, limit: MAX_NODES, spent: (len / 16) as u64 });
        }
        let start = at + 8;
        let end = start.checked_add(len).filter(|e| *e <= buf.len()).ok_or_else(oob)?;
        let padded = end.next_multiple_of(8);
        let clean = pad == 0 && len % size == 0 && buf.get(end..padded).is_some_and(|z| z.iter().all(|b| *b == 0));
        clean.then_some(()).ok_or_else(oob)?;
        out[i] = &buf[start..end];
        at = padded;
    }
    (at == buf.len()).then_some(()).ok_or_else(|| bad(Span::Header, 3, Fault::Length))?;
    Ok((program, out))
}

/// The decls of a job, classified into `Graph`'s per-kind arrays, with each Fn ordinal's kind and each decl's word
/// offset (for the layout pass, which needs the nodes).
struct Decls {
    fns: Vec<GraphFn>,
    raws: Vec<RawFn>,
    loops: Vec<Loop>,
    kernels: Vec<Kernel>,
    params: Vec<Param>,
    inputs: Vec<Input>,
    fn_slots: Vec<FnSlot>,
    fn_at: Vec<usize>,
    loop_at: Vec<usize>,
    program_roots: u32,
}

/// The counts the decl words are checked against.
struct Sizes {
    nodes: usize,
    lits: usize,
    rows: usize,
    syms: usize,
}

fn decls(d: &[u32], roots: &[u32], z: &Sizes) -> Result<Decls, Error> {
    let r0 = *d.first().ok_or_else(|| bad(Span::Decls, 0, Fault::Length))?;
    (r0 as usize <= roots.len()).then_some(()).ok_or_else(|| bad(Span::Decls, 0, Fault::Ref { found: r0 }))?;
    let mut out = Decls {
        fns: Vec::new(),
        raws: Vec::new(),
        loops: Vec::new(),
        kernels: Vec::new(),
        params: Vec::new(),
        inputs: Vec::new(),
        fn_slots: Vec::new(),
        fn_at: Vec::new(),
        loop_at: Vec::new(),
        program_roots: r0,
    };
    let (mut at, mut r) = (1usize, r0 as usize);
    while at < d.len() {
        let head = d.get(at..at + 3).ok_or_else(|| bad(Span::Decls, at, Fault::Length))?;
        let (tag, nr, nw) = (head[0], head[1] as usize, head[2] as usize);
        // Saturating: on a 32-bit target (the wasm viewer) a hostile count must not wrap.
        let w = d.get(at + 3..(at + 3).saturating_add(nw)).ok_or_else(|| bad(Span::Decls, at, Fault::Length))?;
        let rs = roots.get(r..r.saturating_add(nr)).ok_or_else(|| bad(Span::Decls, at, Fault::Length))?;
        let layout = |ok: bool| ok.then_some(()).ok_or_else(|| bad(Span::Decls, at, Fault::Layout));
        let sym =
            |s: u32| ((s as usize) < z.syms).then_some(s).ok_or_else(|| bad(Span::Decls, at, Fault::Sym { found: s }));
        let node =
            |x: u32| ((x as usize) < z.nodes).then_some(x).ok_or_else(|| bad(Span::Decls, at, Fault::Ref { found: x }));
        let lit =
            |k: u32, n: u8| fits(k, u32::from(n), z.lits).then_some(k).ok_or_else(|| bad(Span::Decls, at, Fault::Lit));
        let ty = |c: u32| {
            Ty::ALL.get(c as usize).copied().ok_or_else(|| bad(Span::Decls, at, Fault::Ty { code: c.min(255) as u8 }))
        };
        match (tag, w) {
            (0, [0, name, ret, n, binders @ ..]) => {
                layout(binders.len() == *n as usize && rs.len() == 1)?;
                let binders = binders.iter().map(|b| node(*b)).collect::<Result<_, _>>()?;
                out.fn_slots.push(FnSlot::Graph(out.fns.len() as u32));
                out.fn_at.push(at);
                out.fns.push(GraphFn { name: sym(*name)?, ret: ty(*ret)?, binders, body: node(rs[0])? });
            }
            (0, [1, name, ret, text, n, deps @ ..]) => {
                layout(deps.len() == *n as usize && rs.is_empty())?;
                // A dep is a raw function declared earlier, so the raw functions are acyclic.
                let raw = |k: &u32| match out.fn_slots.get(*k as usize) {
                    Some(FnSlot::Raw(i)) => Ok(*i),
                    Some(FnSlot::Graph(_)) => Err(bad(Span::Decls, at, Fault::Tag { found: *k })),
                    None => Err(bad(Span::Decls, at, Fault::Ref { found: *k })),
                };
                let deps = deps.iter().map(raw).collect::<Result<_, _>>()?;
                out.fn_slots.push(FnSlot::Raw(out.raws.len() as u32));
                out.raws.push(RawFn { name: sym(*name)?, ret: ty(*ret)?, text: sym(*text)?, deps });
            }
            (1, [n, stop, binders @ ..]) => {
                let n = *n as usize;
                let roots_n = n.saturating_mul(2).saturating_add(1 + *stop as usize);
                layout(*stop <= 1 && binders.len() == n.saturating_add(1) && rs.len() == roots_n)?;
                let ids = |x: &[u32]| x.iter().map(|b| node(*b)).collect::<Result<Box<[u32]>, Error>>();
                out.loop_at.push(at);
                out.loops.push(Loop {
                    count: node(rs[0])?,
                    init: ids(&rs[1..1 + n])?,
                    state: ids(&binders[..n])?,
                    index: node(binders[n])?,
                    next: ids(&rs[1 + n..1 + 2 * n])?,
                    stop: rs.get(1 + 2 * n).map(|s| node(*s)).transpose()?,
                });
            }
            (2, [0, name, lanes, entries, role]) => {
                layout(rs.is_empty())?;
                let role = *Role::ALL.get(*role as usize).ok_or_else(|| bad(Span::Decls, at, Fault::Layout))?;
                out.kernels.push(Kernel::Table { name: sym(*name)?, lanes: *lanes, entries: *entries, role });
            }
            (2, [1, label, off, len]) => {
                layout(rs.is_empty())?;
                fits(*off, *len, z.rows)
                    .then_some(())
                    .ok_or_else(|| bad(Span::Decls, at, Fault::Ref { found: *off }))?;
                out.kernels.push(Kernel::Rows { label: sym(*label)?, at: *off, len: *len });
            }
            (8, [name, def, min, max]) => {
                layout(rs.is_empty())?;
                out.params.push(Param {
                    name: sym(*name)?,
                    def: lit(*def, 1)?,
                    min: lit(*min, 1)?,
                    max: lit(*max, 1)?,
                });
            }
            (9, [name, t, def]) => {
                layout(rs.is_empty())?;
                let t = ty(*t)?;
                layout(t.width().is_some())?;
                out.inputs.push(Input { name: sym(*name)?, ty: t, def: lit(*def, t.lits())? });
            }
            (0..=2 | 8 | 9, _) => layout(false)?,
            (_, _) => return Err(bad(Span::Decls, at, Fault::Tag { found: tag })),
        }
        at += 3 + nw;
        r += nr;
    }
    (r == roots.len()).then_some(out).ok_or_else(|| bad(Span::Roots, r, Fault::Length))
}

/// Decodes and validates a job: every bound, reference, layout, quota and type rule, before any pass sees it.
pub fn decode(buf: &[u8]) -> Result<Job, Error> {
    let (program, spans) = spans(buf)?;
    let [nodes_b, args_b, lits_b, rows_b, blobs_b, syms_b, at_b, sites_b, nsites_b, decls_b, roots_b] = spans;

    let n_nodes = nodes_b.len() / 16;
    let args = words(args_b);
    let lits: Box<[u64]> = lits_b.as_chunks::<8>().0.iter().map(|c| u64::from_le_bytes(*c)).collect();
    if let Some(i) = lits.iter().position(|b| !f64::from_bits(*b).is_finite()) {
        return Err(bad(Span::Lits, i, Fault::Lit));
    }
    let rows = words(rows_b);
    let syms = std::str::from_utf8(syms_b).map_err(|e| bad(Span::Syms, e.valid_up_to(), Fault::Utf8))?;
    let sym_at = words(at_b);
    let starts_at_0 = sym_at.first() == Some(&0) && sym_at.last().is_some_and(|e| *e as usize == syms.len());
    starts_at_0.then_some(()).ok_or_else(|| bad(Span::SymAt, 0, Fault::Length))?;
    if let Some(i) = sym_at.windows(2).position(|p| p[1] < p[0] || !syms.is_char_boundary(p[1] as usize)) {
        return Err(bad(Span::SymAt, i + 1, Fault::Length));
    }
    let n_syms = sym_at.len() - 1;
    let sites = words(sites_b)
        .as_chunks::<3>()
        .0
        .iter()
        .enumerate()
        .map(|(i, [f, line, col])| {
            let f = *f as usize;
            (f < n_syms)
                .then(|| Site { file: syms[sym_at[f] as usize..sym_at[f + 1] as usize].into(), line: *line, col: *col })
                .ok_or_else(|| bad(Span::Sites, i, Fault::Sym { found: f as u32 }))
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
    let sizes = Sizes { nodes: n_nodes, lits: lits.len(), rows: rows.len(), syms: n_syms };
    let d = decls(&words(decls_b), &roots, &sizes)?;

    let nodes = nodes(&words(nodes_b), &args, &d, &sizes)?;
    let graph = Graph {
        nodes,
        args,
        lits,
        rows,
        blobs: blobs_b.into(),
        syms: syms.into(),
        sym_at,
        sites,
        node_sites,
        fns: d.fns.into(),
        raws: d.raws.into(),
        loops: d.loops.into(),
        kernels: d.kernels.into(),
        params: d.params.into(),
        inputs: d.inputs.into(),
        roots: roots[..d.program_roots as usize].into(),
    };
    for (f, at) in graph.fns.iter().zip(&d.fn_at) {
        fn_layout(&graph, f).map_err(|()| bad(Span::Decls, *at, Fault::Layout))?;
    }
    for (l, at) in graph.loops.iter().zip(&d.loop_at) {
        loop_layout(&graph, l).map_err(|()| bad(Span::Decls, *at, Fault::Layout))?;
    }
    for i in 0..graph.nodes.len() {
        typed(&graph, i).map_err(|(want, got)| Error::Scene {
            site: graph.site(i as u32).cloned(),
            fault: SceneFault::Type { want, got },
        })?;
    }
    Ok(Job { program, graph })
}

/// The node words, each classified into its `Op` with its references, counts and flags checked.
fn nodes(w: &[u32], args: &[u32], d: &Decls, z: &Sizes) -> Result<Box<[Node]>, Error> {
    let mut out: Vec<Node> = Vec::with_capacity(w.len() / 4);
    for (i, &[head, a, n, imm]) in w.as_chunks::<4>().0.iter().enumerate() {
        let fault = |what: Fault| bad(Span::Nodes, i, what);
        // flags is 16 bits: a table's lane reaches 632 in the corpus (the_strike), past the 8 bits §3.2 first drew.
        let (code, ty_code, flags) = ((head & 0xff) as u8, ((head >> 8) & 0xff) as u8, (head >> 16) as u16);
        let ty = *Ty::ALL.get(usize::from(ty_code)).ok_or_else(|| fault(Fault::Ty { code: ty_code }))?;
        // A Lit's count is its literal entries, not args.
        let n_args = if code == 0 { 0 } else { n };
        fits(a, n_args, args.len()).then_some(()).ok_or_else(|| fault(Fault::Ref { found: a }))?;
        (n_args > 0 || a == 0).then_some(()).ok_or_else(|| fault(Fault::Ref { found: a }))?;
        let operands = &args[a as usize..(a + n_args) as usize];
        if let Some(k) = operands.iter().position(|x| *x as usize >= i) {
            return Err(bad(Span::Args, a as usize + k, Fault::Ref { found: operands[k] }));
        }
        let arity = |ok: bool| ok.then_some(()).ok_or_else(|| fault(Fault::Arity { found: n }));
        let no_flags = || (flags == 0).then_some(()).ok_or_else(|| fault(Fault::Flags { found: flags }));
        let no_imm = || (imm == 0).then_some(()).ok_or_else(|| fault(Fault::Ref { found: imm }));
        let ordinal =
            |len: usize| ((imm as usize) < len).then_some(imm).ok_or_else(|| fault(Fault::Ref { found: imm }));
        // Every root and binder of a called or looped decl precedes the node: a Fn that calls itself, or a Loop whose
        // body reads its own result, which inlining or unrolling would chase forever, cannot be written.
        let before = |late: Option<u32>| match late {
            Some(root) => Err(fault(Fault::Order { root })),
            None => Ok(()),
        };
        let op = match code {
            0 => {
                no_flags()?;
                (n == u32::from(ty.lits()) && fits(imm, n, z.lits)).then_some(()).ok_or_else(|| fault(Fault::Lit))?;
                Op::Lit(imm)
            }
            1 => no_flags().and(no_imm()).and(arity(n == 0)).map(|()| Op::Time)?,
            2 => no_flags().and(arity(n == 0)).and(ordinal(d.params.len())).map(Op::Param)?,
            3 => no_flags().and(arity(n == 0)).and(ordinal(d.inputs.len())).map(Op::Input)?,
            4 => {
                arity(n == 0)?;
                ((imm as usize) < z.syms).then_some(()).ok_or_else(|| fault(Fault::Sym { found: imm }))?;
                match flags {
                    BOUND => Op::Bind(imm),
                    0 => Op::Var(imm),
                    f => return Err(fault(Fault::Flags { found: f })),
                }
            }
            5 => {
                no_flags()?;
                let f = match d.fn_slots.get(imm as usize) {
                    Some(FnSlot::Graph(f)) => *f,
                    Some(FnSlot::Raw(_)) => return Err(fault(Fault::Tag { found: imm })),
                    None => return Err(fault(Fault::Ref { found: imm })),
                };
                let g = &d.fns[f as usize];
                arity(n as usize == g.binders.len())?;
                before(late(g.binders.iter().copied().chain([g.body]), i))?;
                Op::Call(f)
            }
            6 => {
                arity(n == 1)?;
                let kernel = ordinal(d.kernels.len())?;
                let lanes = match d.kernels[kernel as usize] {
                    Kernel::Table { lanes, .. } => lanes,
                    Kernel::Rows { .. } => 1,
                };
                (u32::from(flags) < lanes).then_some(()).ok_or_else(|| fault(Fault::Flags { found: flags }))?;
                Op::Fetch { kernel, lane: flags }
            }
            7 => {
                arity(n == 0)?;
                let decl = ordinal(d.loops.len())?;
                let l = &d.loops[decl as usize];
                (usize::from(flags) < l.init.len())
                    .then_some(())
                    .ok_or_else(|| fault(Fault::Flags { found: flags }))?;
                let all = [l.count, l.index].into_iter().chain(l.init.iter().copied());
                before(late(all.chain(l.state.iter().copied()).chain(l.next.iter().copied()).chain(l.stop), i))?;
                Op::Loop { decl, k: flags }
            }
            8 => no_flags().and(no_imm()).and(arity(n == 2)).map(|()| Op::Grad)?,
            9 => {
                no_flags()?;
                match d.fn_slots.get(imm as usize) {
                    Some(FnSlot::Raw(r)) => Op::Raw(*r),
                    Some(FnSlot::Graph(_)) => return Err(fault(Fault::Tag { found: imm })),
                    None => return Err(fault(Fault::Ref { found: imm })),
                }
            }
            10 => {
                no_flags().and(arity(n == 1))?;
                let len = imm >> 8;
                let ok = (1..=4).contains(&len) && (imm & 0xff) >> (2 * len) == 0;
                ok.then_some(()).ok_or_else(|| fault(Fault::Ref { found: imm }))?;
                Op::Swizzle(Sel { len: len as u8, comps: (imm & 0xff) as u8 })
            }
            11 => no_flags().and(no_imm()).and(arity((1..=4).contains(&n))).map(|()| Op::Vec)?,
            12..PRIM0 => return Err(fault(Fault::Op { code })),
            PRIM0.. => {
                let p = *Prim::ALL.get(usize::from(code - PRIM0)).ok_or_else(|| fault(Fault::Op { code }))?;
                no_flags().and(no_imm()).and(arity(n as usize == p.arity())).map(|()| Op::Prim(p))?
            }
        };
        out.push(Node { op, ty, args: a, n: n_args });
    }
    Ok(out.into())
}

/// The first of `xs` at or after node `i`.
fn late(mut xs: impl Iterator<Item = u32>, i: usize) -> Option<u32> {
    xs.find(|x| *x as usize >= i)
}

fn binder(g: &Graph, x: u32) -> Result<Ty, ()> {
    let node = &g.nodes[x as usize];
    match node.op {
        Op::Bind(_) => Ok(node.ty),
        Op::Lit(_)
        | Op::Time
        | Op::Param(_)
        | Op::Input(_)
        | Op::Var(_)
        | Op::Call(_)
        | Op::Fetch { .. }
        | Op::Loop { .. }
        | Op::Grad
        | Op::Raw(_)
        | Op::Swizzle(_)
        | Op::Vec
        | Op::Prim(_) => Err(()),
    }
}

/// A graph function's binders are binder nodes, and its body has its return type.
fn fn_layout(g: &Graph, f: &GraphFn) -> Result<(), ()> {
    f.binders.iter().try_for_each(|b| binder(g, *b).map(|_| ()))?;
    (g.nodes[f.body as usize].ty == f.ret).then_some(()).ok_or(())
}

/// A loop's count and index are f32, each state component keeps one type from init through binder to next, and its
/// stop is a bool.
fn loop_layout(g: &Graph, l: &Loop) -> Result<(), ()> {
    let ty = |x: u32| g.nodes[x as usize].ty;
    (ty(l.count) == Ty::F32 && binder(g, l.index)? == Ty::F32).then_some(()).ok_or(())?;
    for ((init, state), next) in l.init.iter().zip(&l.state).zip(&l.next) {
        (binder(g, *state)? == ty(*init) && ty(*next) == ty(*init)).then_some(()).ok_or(())?;
    }
    l.stop.map_or(Ok(()), |s| (ty(s) == Ty::Bool).then_some(()).ok_or(()))
}

/// A vector of `parts`: their widths sum to the recorded vector's, or one f32 splats to it.
fn vec_result(parts: &[Ty], rec: Ty) -> Result<Ty, (Ty, Ty)> {
    let want = rec.width().filter(|w| *w > 1).ok_or((Ty::V4, rec))?;
    let mut sum = 0u8;
    for p in parts {
        sum = sum.saturating_add(p.width().ok_or((Ty::F32, *p))?);
    }
    match parts {
        [Ty::F32] => Ok(rec),
        _ if sum == want => Ok(rec),
        _ => Err((rec, Ty::of_width(sum).unwrap_or(Ty::V4))),
    }
}

/// Node `i`'s recorded type follows its operation's rule. Err is (wanted, given).
fn typed(g: &Graph, i: usize) -> Result<(), (Ty, Ty)> {
    let node = &g.nodes[i];
    let arg = |k: usize| g.nodes[g.args_of(node)[k] as usize].ty;
    let rec = node.ty;
    let want = match node.op {
        Op::Lit(_) | Op::Bind(_) | Op::Var(_) => rec,
        Op::Time | Op::Param(_) => Ty::F32,
        Op::Input(d) => g.inputs[d as usize].ty,
        Op::Prim(p) => {
            // At most 3 operands: no allocation per node on a 1M-node job.
            let mut few = [Ty::F32; 3];
            let slot = &mut few[..node.n as usize];
            slot.iter_mut().enumerate().for_each(|(k, t)| *t = arg(k));
            p.result(slot)?
        }
        Op::Vec => {
            let mut few = [Ty::F32; 4];
            let slot = &mut few[..node.n as usize];
            slot.iter_mut().enumerate().for_each(|(k, t)| *t = arg(k));
            vec_result(slot, rec)?
        }
        Op::Swizzle(s) => {
            let w = arg(0).width().filter(|w| *w > 1).ok_or((Ty::V4, arg(0)))?;
            let fits = (0..s.len).all(|k| (s.comps >> (2 * k)) & 3 < w);
            if !fits {
                return Err((Ty::V4, arg(0)));
            }
            Ty::of_width(s.len).ok_or((rec, rec))?
        }
        Op::Call(f) => {
            let f = &g.fns[f as usize];
            for (k, b) in f.binders.iter().enumerate() {
                let b = g.nodes[*b as usize].ty;
                (b == arg(k)).then_some(()).ok_or((b, arg(k)))?;
            }
            f.ret
        }
        Op::Fetch { .. } => (arg(0) == Ty::F32).then_some(Ty::V4).ok_or((Ty::F32, arg(0)))?,
        Op::Loop { decl, k } => g.nodes[g.loops[decl as usize].init[usize::from(k)] as usize].ty,
        Op::Grad => match (arg(0), arg(1).width()) {
            (Ty::F32, Some(_)) => arg(1),
            (Ty::F32, None) => return Err((Ty::F32, arg(1))),
            (f, _) => return Err((Ty::F32, f)),
        },
        Op::Raw(r) => g.raws[r as usize].ret,
    };
    if want == rec { Ok(()) } else { Err((want, rec)) }
}
