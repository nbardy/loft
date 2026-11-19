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
//!             raw   [1, name, ret ty, text sym, n deps, dep Fn ordinal × n deps, n reads, read × n reads]  roots []
//!                   (each dep a raw Fn before it); read 0 table [kernel] | 1 output [kernel, output] | 2 pass [pass]
//!   1 Loop    [n, has stop, state binder × n, index binder] roots [count, init × n, next × n, stop?]
//!   2 Kernel  by fill; a table's head is [name, lanes, entries, role]:
//!             map        [0, head, n, (count, index binder) × n]             roots [lanes × n runs]
//!             rows       [1, label, rows offset, length]                     roots []
//!             scan       [2, head, first, steps, n, state binder × n, index binder]  roots [init × n, next × n, lanes]
//!             data       [3, head, rows offset] (entries × lanes × 4 f32)    roots []
//!             recurrence [4, head, 0 kept, cells, rate lit, from lit, until lit, first, steps, every, prev kernel,
//!                         index binder, clock binder]  roots [start × lanes, step × lanes]; cells 0 list [n] | 1 grid
//!                        [w, h] | [4, head, 1 prev / 2 even / 3 odd, kept kernel]  roots []
//!             compute    [5, name, when, entry sym, dispatch x y z, text sym, n libs, raw Fn ordinal × n, n writes,
//!                         (key sym, symbol sym, shape) × n]  roots [values]; no head (not a table, never fetched);
//!                         when 0 build | 1 load | 2 frame | 3 steps [start sym, count, rate lit, keep];
//!                         shape 0 buffer [entries, lanes] | 1 texture [dims, x, y, z, format sym]
//!   8 Param   [name, def lit, min lit, max lit]      9 Input [name, ty, first def lit]       roots []
//!   3 Draw    [name, layer, cull (none, back, front), depth write, blend, paint Fn, cover...]     roots []
//!             cover  0 grid [u, v, place Fn, normal Fn, cells Fn, count]   1 mesh [kernel, count]   2 quads [place Fn, count]
//!                    3 ray box [find, f, box lits (min, max)]   4 ray instances [find, f, kernel, local lits]
//!                    5 screen []      find 0 hit (f a Fn), 1 medium (f a Medium ordinal)
//!   4 Pass    [camera, first draw, end draw, clear..., ground..., outline...]                    roots []
//!             clear 0 load | 1 colour [rgba lits]; ground 0 none | 1 traced [hit Fn, paint Fn];
//!             outline 0 none | 1 ink [outline Fn]; then target 0 view | 1 texture [name sym, size, keep, scene]:
//!             size 0 view [scale lit] | 1 fixed [w, h]; keep 0 colour | 1 depth | 2 light; scene 0 | 1
//!   5 Medium  [0 march, density Fn, within lits, steps, shadow steps] | [1 gaussians, kernel]      roots []
//!   6 Check   [label, 0 prove boxes | 1 sample n, formula...]  formula, pre-order:  roots in that order
//!             0 atom (roots margin, test) | 1 always / 2 eventually [n, leaf node × n] (roots lo × n, hi × n),
//!             then the body | 3 and / 4 or [n] then n parts | 5 not, then one part
//!   7 Solve   [label, numbers lits (t0, t1, delta, jerk, acceleration, near weight), n,
//!              (name, controls, jet binder × 4) × n, m, unknown × m, h, hold × h]   roots [from × n, to × m, margin × h]
//!             hold [0 equal, within lit | 1 bound, times..., said sym, site sym]; times 0 at [t lit] | 1 during
//!             [lits] | 2 span
//!   10 Camera [name, 0 look] roots [eye, target, up, fovy] | [name, 1 matrix] roots [4 columns]
//!             | [name, 2 table, kernel] roots []
//! ```
//! Fn, kernel, medium and camera references are checked after every decl is read, so a decl may name a later one.
//! A Call or Loop node comes after every root and binder of its decl, so the job is acyclic through decls too.
use crate::error::{Error, Fault, Internal, Meter, SceneFault, Span, WireFault};
use crate::graph::{
    Blend, Camera, Cells, Check, Clear, Compute, Cover, Cull, Draw, Fill, Find, Formula, Graph, GraphFn, Ground, Hold,
    HoldKind, Input, Keep, Kept, Kernel, Layer, Lens, Loop, Medium, Near, Node, Op, OutShape, Outline, Output, Param,
    Part, Pass, Prim, Program, Proxy, Quant, Raster, RawFn, Read, Recur, Role, Run, Scan, Sel, Site, Size, Solve,
    Strategy, Target, Times, Ty, Unknown, When,
};

pub const MAGIC: u32 = u32::from_le_bytes(*b"LFTW");
/// Internal, never persisted: the recorder and this decoder change together. The binding reports it at load.
pub const WIRE: u32 = 5;
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
    raw_at: Vec<usize>,
    loop_at: Vec<usize>,
    kernel_at: Vec<usize>,
    draws: Vec<Draw>,
    passes: Vec<Pass>,
    media: Vec<Medium>,
    cameras: Vec<Camera>,
    row_steps: Vec<(crate::bundle::RowStep, crate::bundle::InputKind)>,
    checks: Vec<Check>,
    solves: Vec<Solve>,
    /// Each scene-level decl's word offset, for refusing a reference checked after all decls are read.
    top_at: Vec<(Top, usize)>,
    program_roots: u32,
}

/// The scene-level decl kinds, for `top_at`.
#[derive(Clone, Copy)]
enum Top {
    Draw(usize),
    Pass(usize),
    Medium(usize),
    Camera(usize),
    Check(usize),
    Solve(usize),
}

/// Reads a scene-level decl's words and roots in order; any shortfall or excess is the decl's `Layout` fault.
struct Cur<'a> {
    w: &'a [u32],
    r: &'a [u32],
    wi: usize,
    ri: usize,
    z: &'a Sizes,
}
impl Cur<'_> {
    fn word(&mut self) -> Result<u32, ()> {
        let x = *self.w.get(self.wi).ok_or(())?;
        self.wi += 1;
        Ok(x)
    }
    fn root(&mut self) -> Result<u32, ()> {
        let x = *self.r.get(self.ri).ok_or(())?;
        self.ri += 1;
        ((x as usize) < self.z.nodes).then_some(x).ok_or(())
    }
    fn roots(&mut self, n: usize) -> Result<Box<[u32]>, ()> {
        (0..n).map(|_| self.root()).collect()
    }
    fn node(&mut self) -> Result<u32, ()> {
        self.word().and_then(|x| ((x as usize) < self.z.nodes).then_some(x).ok_or(()))
    }
    fn sym(&mut self) -> Result<u32, ()> {
        self.word().and_then(|x| ((x as usize) < self.z.syms).then_some(x).ok_or(()))
    }
    fn lits(&mut self, n: u32) -> Result<u32, ()> {
        self.word().and_then(|x| fits(x, n, self.z.lits).then_some(x).ok_or(()))
    }
    fn flag(&mut self) -> Result<bool, ()> {
        match self.word()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(()),
        }
    }
    fn done(&self) -> Result<(), ()> {
        (self.wi == self.w.len() && self.ri == self.r.len()).then_some(()).ok_or(())
    }
}

fn draw(c: &mut Cur) -> Result<Draw, ()> {
    let name = c.sym()?;
    let layer = [Layer::Subject, Layer::Surroundings].get(c.word()? as usize).copied().ok_or(())?;
    let cull = [Cull::None, Cull::Back, Cull::Front].get(c.word()? as usize).copied().ok_or(())?;
    let depth_write = c.flag()?;
    let blend = [Blend::Opaque, Blend::Alpha, Blend::Add].get(c.word()? as usize).copied().ok_or(())?;
    let paint = c.word()?;
    let find = |c: &mut Cur| match c.word()? {
        0 => Ok(Find::Hit(c.word()?)),
        1 => Ok(Find::Medium(c.word()?)),
        _ => Err(()),
    };
    let cover = match c.word()? {
        0 => Cover::Grid {
            u: c.word()?,
            v: c.word()?,
            place: c.word()?,
            normal: c.word()?,
            cells: c.word()?,
            count: c.word()?,
        },
        1 => Cover::Mesh { kernel: c.word()?, count: c.word()? },
        2 => Cover::Quads { place: c.word()?, count: c.word()? },
        3 => {
            let find = find(c)?;
            Cover::Ray { proxy: Proxy::Box { lits: c.lits(6)? }, find }
        }
        4 => {
            let find = find(c)?;
            Cover::Ray { proxy: Proxy::Instances { kernel: c.word()?, local: c.lits(6)? }, find }
        }
        5 => Cover::Screen,
        _ => return Err(()),
    };
    c.done()?;
    Ok(Draw { name, layer, cover, paint, raster: Raster { cull, depth_write, blend } })
}

fn pass(c: &mut Cur) -> Result<Pass, ()> {
    let camera = c.word()?;
    let draws = [c.word()?, c.word()?];
    let clear = match c.word()? {
        0 => Clear::Load,
        1 => Clear::Colour(c.lits(4)?),
        _ => return Err(()),
    };
    let ground = match c.word()? {
        0 => Ground::None,
        1 => Ground::Traced { hit: c.word()?, paint: c.word()? },
        _ => return Err(()),
    };
    let outline = match c.word()? {
        0 => Outline::None,
        1 => Outline::Ink(c.word()?),
        _ => return Err(()),
    };
    let target = match c.word()? {
        0 => Target::View,
        1 => {
            let name = c.sym()?;
            let size = match c.word()? {
                0 => Size::View(c.lits(1)?),
                1 => Size::Fixed(c.word()?, c.word()?),
                _ => return Err(()),
            };
            let keep = [Keep::Colour, Keep::Depth, Keep::Light].get(c.word()? as usize).copied().ok_or(())?;
            Target::Texture { name, size, keep, scene: c.flag()? }
        }
        _ => return Err(()),
    };
    c.done()?;
    (draws[0] <= draws[1]).then_some(Pass { camera, draws, clear, ground, outline, target }).ok_or(())
}

fn medium(c: &mut Cur) -> Result<Medium, ()> {
    let m = match c.word()? {
        0 => {
            let density = c.word()?;
            let within = c.lits(6)?;
            let (steps, shadow_steps) = (c.word()?, c.word()?);
            let (steps, shadow_steps) =
                (u16::try_from(steps).map_err(|_| ())?, u16::try_from(shadow_steps).map_err(|_| ())?);
            Medium::March { density, within, steps, shadow_steps }
        }
        1 => Medium::Gaussians { kernel: c.word()? },
        _ => return Err(()),
    };
    c.done()?;
    Ok(m)
}

fn camera(c: &mut Cur) -> Result<Camera, ()> {
    let name = c.sym()?;
    let lens = match c.word()? {
        0 => Lens::Look { eye: c.root()?, target: c.root()?, up: c.root()?, fovy: c.root()? },
        1 => Lens::Matrix { cols: [c.root()?, c.root()?, c.root()?, c.root()?] },
        2 => Lens::Table { kernel: c.word()? },
        _ => return Err(()),
    };
    c.done()?;
    Ok(Camera { name, lens })
}

fn formula(c: &mut Cur, depth: u32) -> Result<Formula, ()> {
    // A hostile job cannot nest past the stack: 64 levels is more than any authored check.
    (depth < 64).then_some(()).ok_or(())?;
    Ok(match c.word()? {
        0 => Formula::Atom { margin: c.root()?, test: c.root()? },
        k @ (1 | 2) => {
            let n = c.word()? as usize;
            (1..=16).contains(&n).then_some(()).ok_or(())?;
            let leaves = (0..n).map(|_| c.node()).collect::<Result<Box<[u32]>, ()>>()?;
            let (lo, hi) = (c.roots(n)?, c.roots(n)?);
            let q = Quant { leaves, lo, hi, body: Box::new(formula(c, depth + 1)?) };
            if k == 1 { Formula::Always(q) } else { Formula::Eventually(q) }
        }
        k @ (3 | 4) => {
            let n = c.word()? as usize;
            (2..=64).contains(&n).then_some(()).ok_or(())?;
            let parts = (0..n).map(|_| formula(c, depth + 1)).collect::<Result<Box<[Formula]>, ()>>()?;
            if k == 3 { Formula::And(parts) } else { Formula::Or(parts) }
        }
        5 => Formula::Not(Box::new(formula(c, depth + 1)?)),
        _ => return Err(()),
    })
}

fn check(c: &mut Cur) -> Result<Check, ()> {
    let label = c.sym()?;
    let how = match c.word()? {
        0 => Strategy::Prove { boxes: c.word()? },
        1 => Strategy::Sample { n: c.word()? },
        _ => return Err(()),
    };
    let body = formula(c, 0)?;
    c.done()?;
    Ok(Check { label, how, body })
}

fn solve(c: &mut Cur) -> Result<Solve, ()> {
    let label = c.sym()?;
    let numbers = c.lits(6)?;
    let n = c.word()? as usize;
    (n <= c.w.len()).then_some(()).ok_or(())?;
    let mut unknowns = Vec::with_capacity(n);
    for _ in 0..n {
        let (name, numbers) = (c.sym()?, c.word()?);
        let jets = [c.node()?, c.node()?, c.node()?, c.node()?];
        unknowns.push(Unknown { name, numbers, from: 0, jets });
    }
    let m = c.word()? as usize;
    (m <= c.w.len()).then_some(()).ok_or(())?;
    let near_of = (0..m).map(|_| c.word()).collect::<Result<Vec<u32>, ()>>()?;
    let h = c.word()? as usize;
    (h <= c.w.len()).then_some(()).ok_or(())?;
    let mut holds = Vec::with_capacity(h);
    for _ in 0..h {
        let kind = match c.word()? {
            0 => HoldKind::Equal { margin: 0, within: c.lits(1)? },
            1 => HoldKind::Bound { margin: 0 },
            _ => return Err(()),
        };
        let times = match c.word()? {
            0 => Times::At { t: c.lits(1)? },
            1 => Times::During { lits: c.lits(2)? },
            2 => Times::Span,
            _ => return Err(()),
        };
        holds.push(Hold { kind, times, said: c.sym()?, site: c.sym()? });
    }
    // Roots after the words: each unknown's start, each near curve, each hold's margin.
    for u in &mut unknowns {
        u.from = c.root()?;
    }
    let near =
        near_of.into_iter().map(|unknown| Ok(Near { unknown, to: c.root()? })).collect::<Result<Box<[Near]>, ()>>()?;
    for hold in &mut holds {
        hold.kind = match hold.kind {
            HoldKind::Equal { within, .. } => HoldKind::Equal { margin: c.root()?, within },
            HoldKind::Bound { .. } => HoldKind::Bound { margin: c.root()? },
        };
    }
    c.done()?;
    Ok(Solve { label, numbers, unknowns: unknowns.into(), near, holds: holds.into() })
}

/// The counts the decl words are checked against.
struct Sizes {
    nodes: usize,
    lits: usize,
    rows: usize,
    syms: usize,
}

/// A raw function's reads (`Read`), to the end of its words; references are checked in `late_refs`.
fn raw_reads(c: &mut Cur) -> Result<Box<[Read]>, ()> {
    let n = c.word()?;
    let reads = (0..n)
        .map(|_| match c.word()? {
            0 => Ok(Read::Table(c.word()?)),
            1 => Ok(Read::Output { kernel: c.word()?, output: c.word()? }),
            2 => Ok(Read::Pass(c.word()?)),
            _ => Err(()),
        })
        .collect::<Result<_, ()>>()?;
    c.done()?;
    Ok(reads)
}

/// A recurrence table's fill after its head: the kept states (`part` 0), or a part naming them.
fn recur(c: &mut Cur, part: u32, lanes: usize) -> Result<Recur, ()> {
    let r = match part {
        0 => {
            let cells = match c.word()? {
                0 => Cells::List(c.word()?),
                1 => Cells::Grid(c.word()?, c.word()?),
                _ => return Err(()),
            };
            let (rate, from, until) = (c.lits(1)?, c.lits(1)?, c.lits(1)?);
            let (first, steps, every, prev) = (c.word()?, c.word()?, c.word()?, c.word()?);
            let (index, clock) = (c.node()?, c.node()?);
            (every >= 1 && cells.count() >= 1).then_some(()).ok_or(())?;
            Recur::Kept(Box::new(Kept {
                cells,
                rate,
                from,
                until,
                first,
                steps,
                every,
                prev,
                index,
                clock,
                start: c.roots(lanes)?,
                step: c.roots(lanes)?,
            }))
        }
        1..=3 => Recur::Part { part: [Part::Prev, Part::Even, Part::Odd][part as usize - 1], kept: c.word()? },
        _ => return Err(()),
    };
    c.done()?;
    Ok(r)
}

/// A compute kernel after its fill word. Lib references are Fn ordinals, mapped to raws in `late_refs`.
fn compute(c: &mut Cur) -> Result<Compute, ()> {
    let name = c.sym()?;
    let when = match c.word()? {
        0 => When::Build,
        1 => When::Load,
        2 => When::Frame,
        3 => When::Steps { start: c.sym()?, count: c.word()?, rate: c.lits(1)?, keep: c.word()? },
        _ => return Err(()),
    };
    let entry = c.sym()?;
    let dispatch = [c.word()?, c.word()?, c.word()?];
    let text = c.sym()?;
    let n = c.word()?;
    let libs = (0..n).map(|_| c.word()).collect::<Result<_, ()>>()?;
    let n = c.word()?;
    let writes = (0..n)
        .map(|_| {
            let (key, symbol) = (c.sym()?, c.sym()?);
            let shape = match c.word()? {
                0 => OutShape::Buffer { entries: c.word()?, lanes: c.word()? },
                1 => {
                    let dims = c.word()?;
                    let size = [c.word()?, c.word()?, c.word()?];
                    ((dims == 2 && size[2] == 1) || dims == 3).then_some(()).ok_or(())?;
                    OutShape::Texture { dims, size, format: c.sym()? }
                }
                _ => return Err(()),
            };
            Ok(Output { key, symbol, shape })
        })
        .collect::<Result<Box<[Output]>, ()>>()?;
    (!writes.is_empty() && dispatch.iter().all(|d| *d >= 1)).then_some(()).ok_or(())?;
    let values = c.roots(c.r.len() - c.ri)?;
    c.done()?;
    Ok(Compute { name, when, entry, dispatch, text, libs, writes, values })
}

/// References that may name a later decl, checked once every decl is read: raw reads (a table, a compute output, an
/// offscreen pass), a recurrence part's kept states and the kept states' `prev`, a compute's libs (Fn ordinals, made
/// `Graph::raws` indices here) and a table camera's kernel.
fn late_refs(out: &mut Decls) -> Result<(), Error> {
    let kernels = &out.kernels;
    let table = |k: u32| matches!(kernels.get(k as usize), Some(Kernel::Table { .. }));
    let recur = |k: u32| match kernels.get(k as usize) {
        Some(Kernel::Table { fill: Fill::Recurrence(r), .. }) => Some(r),
        _ => None,
    };
    for (r, at) in out.raws.iter().zip(&out.raw_at) {
        let ok = r.reads.iter().all(|x| match *x {
            Read::Table(k) => table(k),
            Read::Output { kernel, output } => {
                matches!(kernels.get(kernel as usize), Some(Kernel::Compute(c)) if (output as usize) < c.writes.len())
            }
            Read::Pass(p) => matches!(out.passes.get(p as usize), Some(Pass { target: Target::Texture { .. }, .. })),
        });
        ok.then_some(()).ok_or_else(|| bad(Span::Decls, *at, Fault::Layout))?;
    }
    for (i, (k, at)) in out.kernels.iter().zip(&out.kernel_at).enumerate() {
        let ok = match k {
            Kernel::Table { fill: Fill::Recurrence(Recur::Kept(x)), .. } => {
                matches!(recur(x.prev), Some(Recur::Part { part: Part::Prev, kept }) if *kept as usize == i)
            }
            Kernel::Table { fill: Fill::Recurrence(Recur::Part { kept, .. }), .. } => {
                matches!(recur(*kept), Some(Recur::Kept(_)))
            }
            Kernel::Table { .. } | Kernel::Rows { .. } | Kernel::Compute(_) => true,
        };
        ok.then_some(()).ok_or_else(|| bad(Span::Decls, *at, Fault::Layout))?;
    }
    for (k, c) in out.cameras.iter().enumerate() {
        let Lens::Table { kernel } = c.lens else { continue };
        let at = out.top_at.iter().find(|(t, _)| t.same(Top::Camera(k))).map_or(0, |(_, a)| *a);
        table(kernel).then_some(()).ok_or_else(|| bad(Span::Decls, at, Fault::Layout))?;
    }
    let slots = &out.fn_slots;
    for (k, at) in out.kernels.iter_mut().zip(&out.kernel_at) {
        let Kernel::Compute(c) = k else { continue };
        let libs = c.libs.iter().map(|f| match slots.get(*f as usize) {
            Some(FnSlot::Raw(r)) => Ok(*r),
            Some(FnSlot::Graph(_)) => Err(bad(Span::Decls, *at, Fault::Tag { found: *f })),
            None => Err(bad(Span::Decls, *at, Fault::Ref { found: *f })),
        });
        c.libs = libs.collect::<Result<_, _>>()?;
    }
    Ok(())
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
        raw_at: Vec::new(),
        loop_at: Vec::new(),
        kernel_at: Vec::new(),
        draws: Vec::new(),
        passes: Vec::new(),
        media: Vec::new(),
        cameras: Vec::new(),
        row_steps: Vec::new(),
        checks: Vec::new(),
        solves: Vec::new(),
        top_at: Vec::new(),
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
            (0, [1, name, ret, text, n, rest @ ..]) => {
                let n = *n as usize;
                layout(rest.len() > n && rs.is_empty())?;
                // A dep is a raw function declared earlier, so the raw functions are acyclic.
                let raw = |k: &u32| match out.fn_slots.get(*k as usize) {
                    Some(FnSlot::Raw(i)) => Ok(*i),
                    Some(FnSlot::Graph(_)) => Err(bad(Span::Decls, at, Fault::Tag { found: *k })),
                    None => Err(bad(Span::Decls, at, Fault::Ref { found: *k })),
                };
                let deps = rest[..n].iter().map(raw).collect::<Result<_, _>>()?;
                // Reads name kernels and passes, which may come later: their kinds are checked in `late_refs`.
                let mut c = Cur { w: &rest[n..], r: &[], wi: 0, ri: 0, z };
                let reads = raw_reads(&mut c).map_err(|()| bad(Span::Decls, at, Fault::Layout))?;
                out.raw_at.push(at);
                out.fn_slots.push(FnSlot::Raw(out.raws.len() as u32));
                out.raws.push(RawFn { name: sym(*name)?, ret: ty(*ret)?, text: sym(*text)?, deps, reads });
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
            (2, [1, label, off, len]) => {
                layout(rs.is_empty())?;
                fits(*off, *len, z.rows)
                    .then_some(())
                    .ok_or_else(|| bad(Span::Decls, at, Fault::Ref { found: *off }))?;
                out.kernel_at.push(at);
                out.kernels.push(Kernel::Rows { label: sym(*label)?, at: *off, len: *len });
            }
            (2, [5, rest @ ..]) => {
                let mut c = Cur { w: rest, r: rs, wi: 0, ri: 0, z };
                let k = compute(&mut c).map_err(|()| bad(Span::Decls, at, Fault::Layout))?;
                out.kernel_at.push(at);
                out.kernels.push(Kernel::Compute(Box::new(k)));
            }
            (2, [fill @ (0 | 2 | 3 | 4), name, lanes, entries, role, rest @ ..]) => {
                let role = *Role::ALL.get(*role as usize).ok_or_else(|| bad(Span::Decls, at, Fault::Layout))?;
                layout(*lanes >= 1)?;
                let n_lanes = *lanes as usize;
                let ids = |x: &[u32]| x.iter().map(|b| node(*b)).collect::<Result<Box<[u32]>, Error>>();
                let fill = match (fill, rest) {
                    (0, [n, runs @ ..]) => {
                        let n = *n as usize;
                        layout(runs.len() == n.saturating_mul(2) && rs.len() == n.saturating_mul(n_lanes))?;
                        let total = runs.chunks(2).try_fold(0u32, |t, r| t.checked_add(r[0]));
                        layout(total.is_some_and(|t| t <= *entries))?;
                        let runs = runs
                            .chunks(2)
                            .zip(rs.chunks(n_lanes))
                            .map(|(r, l)| Ok(Run { count: r[0], index: node(r[1])?, lanes: ids(l)? }))
                            .collect::<Result<Box<[Run]>, Error>>()?;
                        Fill::Map(runs)
                    }
                    (2, [first, steps, n, binders @ ..]) => {
                        let n = *n as usize;
                        let roots_n = n.saturating_mul(2).saturating_add(n_lanes);
                        layout(binders.len() == n.saturating_add(1) && rs.len() == roots_n)?;
                        layout(first.checked_add(*steps).is_some_and(|e| e <= *entries))?;
                        Fill::Scan(Scan {
                            first: *first,
                            steps: *steps,
                            init: ids(&rs[..n])?,
                            state: ids(&binders[..n])?,
                            index: node(binders[n])?,
                            next: ids(&rs[n..2 * n])?,
                            lanes: ids(&rs[2 * n..])?,
                        })
                    }
                    (3, [off]) => {
                        layout(rs.is_empty())?;
                        let len = u64::from(*entries) * u64::from(*lanes) * 4;
                        let fit = u32::try_from(len).ok().is_some_and(|len| fits(*off, len, z.rows));
                        fit.then_some(()).ok_or_else(|| bad(Span::Decls, at, Fault::Ref { found: *off }))?;
                        Fill::Data { at: *off }
                    }
                    (4, [part, rest @ ..]) => {
                        let mut c = Cur { w: rest, r: rs, wi: 0, ri: 0, z };
                        Fill::Recurrence(
                            recur(&mut c, *part, n_lanes).map_err(|()| bad(Span::Decls, at, Fault::Layout))?,
                        )
                    }
                    _ => return Err(bad(Span::Decls, at, Fault::Layout)),
                };
                out.kernel_at.push(at);
                out.kernels.push(Kernel::Table { name: sym(*name)?, lanes: *lanes, entries: *entries, role, fill });
            }
            (11, [text, plan, seed, output, input, hz, ticks, how, arg]) => {
                use crate::bundle::{InputKind, RowStep};
                let table =
                    |r: u32| matches!(out.kernels.get(r as usize), Some(Kernel::Table { fill: Fill::Data { .. }, .. }));
                layout(
                    rs.is_empty()
                        && table(*plan)
                        && table(*seed)
                        && table(*output)
                        && (*input as usize) < out.inputs.len()
                        && *hz > 0
                        && *ticks > 0,
                )?;
                let kind = match (*how, *arg) {
                    (0, 0) => InputKind::Level,
                    (1, r) if table(r) => InputKind::Script { samples: r },
                    (2, slots) if slots > 0 => InputKind::Events { slots },
                    _ => return Err(bad(Span::Decls, at, Fault::Layout)),
                };
                out.row_steps.push((
                    RowStep {
                        module: sym(*text)?,
                        plan: *plan,
                        seed: *seed,
                        output: *output,
                        input: *input,
                        hz: *hz,
                        ticks: *ticks,
                    },
                    kind,
                ));
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
            (3..=7 | 10, _) => {
                let mut c = Cur { w, r: rs, wi: 0, ri: 0, z };
                let failed = || bad(Span::Decls, at, Fault::Layout);
                match tag {
                    3 => out.top_at.push((Top::Draw(out.draws.len()), at)),
                    4 => out.top_at.push((Top::Pass(out.passes.len()), at)),
                    5 => out.top_at.push((Top::Medium(out.media.len()), at)),
                    6 => out.top_at.push((Top::Check(out.checks.len()), at)),
                    7 => out.top_at.push((Top::Solve(out.solves.len()), at)),
                    _ => out.top_at.push((Top::Camera(out.cameras.len()), at)),
                }
                match tag {
                    3 => out.draws.push(draw(&mut c).map_err(|()| failed())?),
                    4 => out.passes.push(pass(&mut c).map_err(|()| failed())?),
                    5 => out.media.push(medium(&mut c).map_err(|()| failed())?),
                    6 => out.checks.push(check(&mut c).map_err(|()| failed())?),
                    7 => out.solves.push(solve(&mut c).map_err(|()| failed())?),
                    _ => out.cameras.push(camera(&mut c).map_err(|()| failed())?),
                }
            }
            (0..=2 | 8 | 9, _) => layout(false)?,
            (_, _) => return Err(bad(Span::Decls, at, Fault::Tag { found: tag })),
        }
        at += 3 + nw;
        r += nr;
    }
    (r == roots.len()).then_some(()).ok_or_else(|| bad(Span::Roots, r, Fault::Length))?;
    graph_fns(&mut out)?;
    late_refs(&mut out)?;
    Ok(out)
}

/// A scene-level decl names graph functions by Fn ordinal (graph and raw numbered together); `Graph` keeps graph
/// functions apart, so each becomes its `Graph::fns` index here. A raw function or a missing one is the decl's fault.
fn graph_fns(out: &mut Decls) -> Result<(), Error> {
    let slots = &out.fn_slots;
    let at = |top: Top| out.top_at.iter().find(|(t, _)| t.same(top)).map_or(0, |(_, a)| *a);
    let map = |f: &mut u32, top: Top| match slots.get(*f as usize) {
        Some(FnSlot::Graph(g)) => {
            *f = *g;
            Ok(())
        }
        Some(FnSlot::Raw(_)) | None => Err(bad(Span::Decls, at(top), Fault::Layout)),
    };
    for (k, d) in out.draws.iter_mut().enumerate() {
        let top = Top::Draw(k);
        map(&mut d.paint, top)?;
        match &mut d.cover {
            Cover::Grid { place, normal, cells, .. } => {
                map(place, top)?;
                map(normal, top)?;
                map(cells, top)?;
            }
            Cover::Quads { place, .. } => map(place, top)?,
            Cover::Ray { find: Find::Hit(f), .. } => map(f, top)?,
            Cover::Mesh { .. } | Cover::Ray { find: Find::Medium(_), .. } | Cover::Screen => {}
        }
    }
    for (k, p) in out.passes.iter_mut().enumerate() {
        let top = Top::Pass(k);
        match &mut p.ground {
            Ground::Traced { hit, paint } => {
                map(hit, top)?;
                map(paint, top)?;
            }
            Ground::None => {}
        }
        match &mut p.outline {
            Outline::Ink(f) => map(f, top)?,
            Outline::None => {}
        }
    }
    for (k, m) in out.media.iter_mut().enumerate() {
        match m {
            Medium::March { density, .. } => map(density, Top::Medium(k))?,
            Medium::Gaussians { .. } => {}
        }
    }
    Ok(())
}

impl Top {
    fn same(self, o: Top) -> bool {
        match (self, o) {
            (Top::Draw(a), Top::Draw(b))
            | (Top::Pass(a), Top::Pass(b))
            | (Top::Medium(a), Top::Medium(b))
            | (Top::Camera(a), Top::Camera(b))
            | (Top::Check(a), Top::Check(b))
            | (Top::Solve(a), Top::Solve(b)) => a == b,
            (Top::Draw(_) | Top::Pass(_) | Top::Medium(_) | Top::Camera(_) | Top::Check(_) | Top::Solve(_), _) => false,
        }
    }
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
        draws: d.draws.into(),
        passes: d.passes.into(),
        media: d.media.into(),
        cameras: d.cameras.into(),
        row_steps: d.row_steps,
        checks: d.checks.into(),
        solves: d.solves.into(),
        roots: roots[..d.program_roots as usize].into(),
    };
    for (f, at) in graph.fns.iter().zip(&d.fn_at) {
        fn_layout(&graph, f).map_err(|()| bad(Span::Decls, *at, Fault::Layout))?;
    }
    for (l, at) in graph.loops.iter().zip(&d.loop_at) {
        loop_layout(&graph, l).map_err(|()| bad(Span::Decls, *at, Fault::Layout))?;
    }
    for (k, at) in graph.kernels.iter().zip(&d.kernel_at) {
        kernel_layout(&graph, k).map_err(|()| bad(Span::Decls, *at, Fault::Layout))?;
    }
    for (top, at) in &d.top_at {
        top_layout(&graph, *top).map_err(|()| bad(Span::Decls, *at, Fault::Layout))?;
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
                let lanes = match &d.kernels[kernel as usize] {
                    Kernel::Table { lanes, .. } => *lanes,
                    Kernel::Rows { .. } => 1,
                    // A compute pass writes outputs that raw text reads by symbol; no node fetches it.
                    Kernel::Compute(_) => return Err(fault(Fault::Tag { found: kernel })),
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

/// A fill's index binders are f32 binder nodes and its lanes vec4; a scan's state keeps one type from init through
/// binder to next.
fn kernel_layout(g: &Graph, k: &Kernel) -> Result<(), ()> {
    let ty = |x: u32| g.nodes[x as usize].ty;
    let vec4s = |xs: &[u32]| xs.iter().all(|x| ty(*x) == Ty::V4).then_some(()).ok_or(());
    match k {
        Kernel::Rows { .. } => Ok(()),
        // Values are read through `loft_value_<k>()`, of any number or bool type.
        Kernel::Compute(_) => Ok(()),
        Kernel::Table { fill: Fill::Recurrence(Recur::Kept(x)), lanes, entries, .. } => {
            let index = match x.cells {
                Cells::List(_) => Ty::F32,
                Cells::Grid(..) => Ty::V2,
            };
            (binder(g, x.index)? == index && binder(g, x.clock)? == Ty::F32).then_some(()).ok_or(())?;
            // Kept states: steps + 1, or one per checkpoint; each the cells, then the clock row.
            let states = if x.every == 1 { u64::from(x.steps) + 1 } else { u64::from(x.steps / x.every) + 1 };
            (states * (x.cells.count() + 1) == u64::from(*entries)).then_some(()).ok_or(())?;
            (x.start.len() == *lanes as usize).then_some(()).ok_or(())?;
            vec4s(&x.start).and(vec4s(&x.step))
        }
        Kernel::Table { fill, .. } => match fill {
            Fill::Map(runs) => runs.iter().try_for_each(|r| {
                (binder(g, r.index)? == Ty::F32).then_some(()).ok_or(())?;
                vec4s(&r.lanes)
            }),
            Fill::Scan(s) => {
                (binder(g, s.index)? == Ty::F32).then_some(()).ok_or(())?;
                for ((init, state), next) in s.init.iter().zip(&s.state).zip(&s.next) {
                    (binder(g, *state)? == ty(*init) && ty(*next) == ty(*init)).then_some(()).ok_or(())?;
                }
                vec4s(&s.lanes)
            }
            Fill::Data { .. } | Fill::Recurrence(_) => Ok(()),
        },
    }
}

/// A graph function `f` takes `params` and returns `ret` (a draw role's signature, loft-core `draw`).
fn sig(g: &Graph, f: u32, params: &[Ty], ret: Ty) -> Result<(), ()> {
    let f = g.fns.get(f as usize).ok_or(())?;
    let tys: Vec<Ty> = f.binders.iter().map(|b| g.nodes[*b as usize].ty).collect();
    (tys == params && f.ret == ret).then_some(()).ok_or(())
}

/// A scene-level decl's references (Fn, kernel, medium, camera and draw ranges) and its nodes' types.
fn top_layout(g: &Graph, top: Top) -> Result<(), ()> {
    use Ty::{F32, V2, V3, V4};
    let ty = |x: u32| g.nodes[x as usize].ty;
    let is = |x: u32, t: Ty| (ty(x) == t).then_some(()).ok_or(());
    let kernel = |k: u32| ((k as usize) < g.kernels.len()).then_some(()).ok_or(());
    let paint = |f: u32| sig(g, f, &[V3, V3, V2, F32], V4);
    let hit = |f: u32| sig(g, f, &[V3, V3, V2, V2, F32], V4);
    match top {
        Top::Draw(k) => {
            let d = &g.draws[k];
            paint(d.paint)?;
            match &d.cover {
                Cover::Grid { place, normal, cells, .. } => {
                    sig(g, *place, &[V2, F32], V3).and(sig(g, *normal, &[V2, F32], V3)).and(sig(g, *cells, &[F32], V2))
                }
                Cover::Mesh { kernel: r, .. } => kernel(*r),
                Cover::Quads { place, .. } => sig(g, *place, &[F32], V4),
                Cover::Ray { proxy, find } => {
                    match proxy {
                        Proxy::Box { .. } => {}
                        Proxy::Instances { kernel: r, .. } => kernel(*r)?,
                    }
                    match find {
                        Find::Hit(f) => hit(*f),
                        Find::Medium(m) => ((*m as usize) < g.media.len()).then_some(()).ok_or(()),
                    }
                }
                Cover::Screen => Ok(()),
            }
        }
        Top::Pass(k) => {
            let p = &g.passes[k];
            ((p.camera as usize) < g.cameras.len() && p.draws[1] as usize <= g.draws.len()).then_some(()).ok_or(())?;
            match p.ground {
                Ground::None => {}
                Ground::Traced { hit: h, paint: c } => hit(h).and(paint(c))?,
            }
            match p.outline {
                Outline::None => Ok(()),
                Outline::Ink(f) => sig(g, f, &[V3, V3], V4),
            }
        }
        Top::Medium(k) => match g.media[k] {
            Medium::March { density, .. } => sig(g, density, &[V3], F32),
            Medium::Gaussians { kernel: r } => kernel(r),
        },
        Top::Camera(k) => match g.cameras[k].lens {
            Lens::Look { eye, target, up, fovy } => is(eye, V3).and(is(target, V3)).and(is(up, V3)).and(is(fovy, F32)),
            Lens::Matrix { cols } => cols.iter().try_for_each(|c| is(*c, V4)),
            Lens::Table { .. } => Ok(()),
        },
        Top::Check(k) => {
            fn formula(f: &Formula, is: &dyn Fn(u32, Ty) -> Result<(), ()>, g: &Graph) -> Result<(), ()> {
                match f {
                    Formula::Atom { margin, test } => is(*margin, Ty::F32).and(is(*test, Ty::Bool)),
                    Formula::Always(q) | Formula::Eventually(q) => {
                        for ((leaf, lo), hi) in q.leaves.iter().zip(&q.lo).zip(&q.hi) {
                            let n = &g.nodes[*leaf as usize];
                            let leafy = matches!(n.op, Op::Time | Op::Param(_) | Op::Input(_) | Op::Var(_));
                            (leafy && n.ty == Ty::F32).then_some(()).ok_or(())?;
                            is(*lo, Ty::F32).and(is(*hi, Ty::F32))?;
                        }
                        formula(&q.body, is, g)
                    }
                    Formula::And(parts) | Formula::Or(parts) => parts.iter().try_for_each(|p| formula(p, is, g)),
                    Formula::Not(p) => formula(p, is, g),
                }
            }
            formula(&g.checks[k].body, &is, g)
        }
        Top::Solve(k) => {
            let s = &g.solves[k];
            for u in s.unknowns.iter() {
                is(u.from, F32)?;
                u.jets.iter().try_for_each(|j| (binder(g, *j)? == F32).then_some(()).ok_or(()))?;
            }
            for n in s.near.iter() {
                ((n.unknown as usize) < s.unknowns.len()).then_some(()).ok_or(())?;
                is(n.to, F32)?;
            }
            s.holds.iter().try_for_each(|h| match h.kind {
                HoldKind::Equal { margin, .. } => ty(margin).width().map(|_| ()).ok_or(()),
                HoldKind::Bound { margin } => is(margin, F32),
            })
        }
    }
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
