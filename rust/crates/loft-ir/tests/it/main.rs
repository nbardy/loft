//! Lane 1.1's checks (plan §8.2), on lane 1.2's wire: the wire refuses fuzzed malformed jobs with typed errors and no
//! panic; the canonical print equals the TS print over the corpus; a bundle writes, reads and writes to the same
//! bytes; every Identity field moves the hash.
use loft_ir::bundle::*;
use loft_ir::error::{Error, Fault, Internal, SceneFault, Span};
use loft_ir::graph::{self, Fill, Op, Prim, Program, Site, Ty};
use loft_ir::identity::{self, Backend, Device, FrameKey, Identity, Precision};
use loft_ir::wire::{self, BOUND, MAGIC, PRIM0, WIRE};

/// A job writer in the wire's layout (src/record/wire.ts `pack`; the recorder's job in TS, here only for test jobs).
#[derive(Default, Clone)]
struct Job {
    nodes: Vec<[u32; 4]>,
    args: Vec<u32>,
    lits: Vec<f64>,
    rows: Vec<f32>,
    syms: Vec<String>,
    sites: Vec<[u32; 3]>,
    node_sites: Vec<u32>,
    /// (tag, words, roots) per decl, in decl order.
    decls: Vec<(u32, Vec<u32>, Vec<u32>)>,
    roots: Vec<u32>,
}

const fn ty(t: u32) -> u32 {
    t << 8
}
const F32: u32 = ty(0);
const V2: u32 = ty(1);
const V3: u32 = ty(2);
const V4: u32 = ty(3);
const BOOL: u32 = ty(4);
const BIND: u32 = 4 | (BOUND as u32) << 16;
fn prim(p: Prim) -> u32 {
    u32::from(PRIM0) + Prim::ALL.iter().position(|q| *q == p).unwrap() as u32
}

impl Job {
    fn node(&mut self, head: u32, args: &[u32], imm: u32) -> u32 {
        let at = if args.is_empty() { 0 } else { self.args.len() as u32 };
        self.nodes.push([head, at, args.len() as u32, imm]);
        self.args.extend_from_slice(args);
        self.node_sites.push(0);
        self.nodes.len() as u32 - 1
    }
    fn lit_at(&mut self, v: &[f64]) -> u32 {
        self.lits.extend_from_slice(v);
        (self.lits.len() - v.len()) as u32
    }
    fn lit(&mut self, head: u32, v: &[f64]) -> u32 {
        let at = self.lit_at(v);
        let k = self.node(head, &[], at);
        self.nodes[k as usize][2] = v.len() as u32;
        k
    }
    fn sym(&mut self, s: &str) -> u32 {
        self.syms.push(s.into());
        self.syms.len() as u32 - 1
    }
    fn decl(&mut self, tag: u32, words: &[u32], roots: &[u32]) -> usize {
        self.decls.push((tag, words.into(), roots.into()));
        self.decls.len() - 1
    }
    fn bytes(&self, program: u32) -> Vec<u8> {
        let le = |v: &[u32]| v.iter().flat_map(|w| w.to_le_bytes()).collect::<Vec<u8>>();
        let mut at = vec![0u32];
        let mut text = String::new();
        for s in &self.syms {
            text.push_str(s);
            at.push(text.len() as u32);
        }
        let mut decls = vec![self.roots.len() as u32];
        let mut roots = self.roots.clone();
        for (tag, w, r) in &self.decls {
            decls.extend([*tag, r.len() as u32, w.len() as u32]);
            decls.extend(w);
            roots.extend(r);
        }
        let spans: [Vec<u8>; 11] = [
            le(&self.nodes.concat()),
            le(&self.args),
            self.lits.iter().flat_map(|x| x.to_bits().to_le_bytes()).collect(),
            le(&self.rows.iter().map(|x| x.to_bits()).collect::<Vec<_>>()),
            vec![7, 8, 9],
            text.into_bytes(),
            le(&at),
            le(&self.sites.concat()),
            le(&self.node_sites),
            le(&decls),
            le(&roots),
        ];
        let mut out = le(&[MAGIC, WIRE, program, 11]);
        for s in &spans {
            out.extend(le(&[s.len() as u32, 0]));
            out.extend(s);
            out.resize(out.len().next_multiple_of(8), 0);
        }
        out
    }
}

/// Named nodes and decls of the sample, for the fault cases.
struct Ids {
    sq: usize,
    call: usize,
    looped: usize,
    fetch: usize,
    sw: usize,
    x: usize,
    last: usize,
    one: usize,
    fn_decl: usize,
    loop_decl: usize,
    raw_decl: usize,
    map_decl: usize,
    scan_decl: usize,
}

/// One job using every op and every pinned decl layout.
fn sample() -> (Job, Ids) {
    let mut j = Job::default();
    let file = j.sym("examples/owl.ts");
    j.sites.push([file, 12, 5]);
    let one = j.lit(F32, &[1.0]);
    let two = j.lit(F32, &[2.0]);
    let up = j.lit(V3, &[0.0, 1.0, 0.0]);
    let time = j.node(1 | F32, &[], 0);
    let (gain, def) = (j.sym("gain"), j.lit_at(&[0.5, 0.0, 1.0]));
    j.decl(8, &[gain, def, def + 1, def + 2], &[]);
    let param = j.node(2 | F32, &[], 0);
    let (wind, def) = (j.sym("wind"), j.lit_at(&[0.0, 0.0, 1.0]));
    j.decl(9, &[wind, 2, def], &[]);
    let input = j.node(3 | V3, &[], 0);
    // Fn ordinal 0: a graph function.
    let a0 = j.sym("F0a0");
    let x = j.node(BIND | F32, &[], a0);
    let sq = j.node(prim(Prim::Mul) | F32, &[x, x], 0);
    let square = j.sym("square");
    let fn_decl = j.decl(0, &[0, square, 0, 1, x], &[sq]);
    let call = j.node(5 | F32, &[time], 0);
    let (s0, si) = (j.sym("L0s0"), j.sym("L0i"));
    let s = j.node(BIND | F32, &[], s0);
    let k = j.node(BIND | F32, &[], si);
    let next = j.node(prim(Prim::Add) | F32, &[s, k], 0);
    let stop = j.node(prim(Prim::Gt) | BOOL, &[next, two], 0);
    let loop_decl = j.decl(1, &[1, 1, s, k], &[two, one, next, stop]);
    let looped = j.node(7 | F32, &[], 0);
    // Kernel 0: a map table, one run of 64 rows, each lane the row index splatted.
    let (tab, ki) = (j.sym("tab"), j.sym("K0r0i"));
    let ri = j.node(BIND | F32, &[], ki);
    let lane = j.node(11 | V4, &[ri], 0);
    let map_decl = j.decl(2, &[0, tab, 3, 64, 1, 1, 64, ri], &[lane, lane, lane]);
    let fetch = j.node(6 | V4 | 2 << 16, &[call], 0);
    let curve = j.sym("curve");
    j.rows.extend([1.5, 2.0]);
    j.decl(2, &[1, curve, 0, 2], &[]);
    // Kernels 2..=4: a scan (a counter from 1), supplied numbers and a recurrence's table.
    let (scan, ss, si) = (j.sym("scan"), j.sym("K2s0"), j.sym("K2i"));
    let (sb, sk) = (j.node(BIND | F32, &[], ss), j.node(BIND | F32, &[], si));
    let (sn, sl) = (j.node(prim(Prim::Add) | F32, &[sb, sk], 0), j.node(11 | V4, &[sb], 0));
    let scan_decl = j.decl(2, &[2, scan, 1, 8, 2, 0, 8, 1, sb, sk], &[one, sn, sl]);
    let data = j.sym("data");
    j.rows.extend([0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]);
    j.decl(2, &[3, data, 1, 2, 2, 2], &[]);
    // Kernels 4 and 5: a recurrence over a list of 2, one step kept whole (2 states of 3 rows), and its previous state.
    let (kept, prev, ki, kt) = (j.sym("kept"), j.sym("kept_prev"), j.sym("K4i"), j.sym("K4t"));
    let (rate, from, until) = (j.lit_at(&[60.0]), j.lit_at(&[0.0]), j.lit_at(&[1.0 / 60.0]));
    let (bi, bt) = (j.node(BIND | F32, &[], ki), j.node(BIND | F32, &[], kt));
    let start = j.node(11 | V4, &[bi], 0);
    let before = j.node(6 | V4, &[bi], 5);
    let stepped = j.node(prim(Prim::Add) | V4, &[before, bt], 0);
    j.decl(2, &[4, kept, 1, 6, 2, 0, 0, 2, rate, from, until, 0, 1, 1, 5, bi, bt], &[start, stepped]);
    j.decl(2, &[4, prev, 1, 3, 1, 1, 4], &[]);
    let row = j.node(6 | V4, &[looped], 1);
    let sw = j.node(10 | V2, &[fetch], 2 | 2 << 8); // z then x
    let v = j.node(11 | V3, &[sw, looped], 0);
    let scaled = j.node(prim(Prim::Mul) | V3, &[v, param], 0);
    let grad = j.node(8 | V3, &[call, input], 0);
    // Fn ordinals 1 and 2: raw functions, the second calling the first.
    let (base, base_text) = (j.sym("base"), j.sym("fn base(x: f32) -> f32 { return x; }"));
    j.decl(0, &[1, base, 0, base_text, 0, 0], &[]);
    let (wobble, text) = (j.sym("wobble"), j.sym("fn wobble(x: f32) -> f32 { return base(x); }"));
    let raw_decl = j.decl(0, &[1, wobble, 0, text, 1, 1, 0], &[]);
    let r = j.node(9 | F32, &[time], 2);
    let u = j.sym("u");
    let var = j.node(4 | F32, &[], u);
    let cond = j.node(prim(Prim::Lt) | BOOL, &[r, var], 0);
    let out = j.node(prim(Prim::Select) | V3, &[cond, scaled, grad], 0);
    let rx = j.node(10 | F32, &[row], 1 << 8);
    let last = j.node(prim(Prim::Mix) | V3, &[out, up, rx], 0);
    *j.node_sites.last_mut().unwrap() = 1;
    j.roots.push(last);
    let ids = Ids {
        sq: sq as usize,
        call: call as usize,
        looped: looped as usize,
        fetch: fetch as usize,
        sw: sw as usize,
        x: x as usize,
        last: last as usize,
        one: one as usize,
        fn_decl,
        loop_decl,
        raw_decl,
        map_decl,
        scan_decl,
    };
    (j, ids)
}

fn wire_fault(e: &Error) -> Option<(Span, Fault)> {
    match e {
        Error::Internal(Internal::Wire(w)) => Some((w.span, w.what)),
        Error::Usage(_)
        | Error::Scene { .. }
        | Error::Budget { .. }
        | Error::Load(_)
        | Error::Environment(_)
        | Error::Internal(_) => None,
    }
}

#[test]
fn sample_decodes() {
    let (j, id) = sample();
    let job = wire::decode(&j.bytes(1)).expect("the sample is well formed");
    assert_eq!(job.program, Program::Solve);
    let g = &job.graph;
    assert_eq!(g.nodes[id.last].ty, Ty::V3);
    assert_eq!(g.site(id.last as u32).map(|s| s.to_string()), Some("examples/owl.ts:12:5".into()));
    assert!(matches!(g.nodes[id.call].op, Op::Call(0)));
    assert!(matches!(g.nodes[id.fetch].op, Op::Fetch { kernel: 0, lane: 2 }));
    assert!(matches!(g.nodes[id.x].op, Op::Bind(_)));
    // Fn ordinal 2 is the second raw function: raws[1], whose dep is raws[0].
    assert!(matches!(g.nodes[id.last - 5].op, Op::Raw(1)));
    assert_eq!((g.fns.len(), g.raws.len(), &*g.raws[1].deps), (1, 2, &[0][..]));
    assert_eq!(g.sym(g.raws[0].name), "base");
    assert_eq!(&*g.roots, &[id.last as u32]);
    // Each fill layout decodes into its variant.
    let fills: Vec<&Fill> = g
        .kernels
        .iter()
        .filter_map(|k| match k {
            graph::Kernel::Table { fill, .. } => Some(fill),
            graph::Kernel::Rows { .. } | graph::Kernel::Compute(_) => None,
        })
        .collect();
    assert!(matches!(fills[0], Fill::Map(runs) if runs.len() == 1 && runs[0].count == 64 && runs[0].lanes.len() == 3));
    assert!(matches!(fills[1], Fill::Scan(s) if s.first == 0 && s.steps == 8 && s.state.len() == 1));
    assert!(matches!(fills[2], Fill::Data { at: 2 }));
    assert!(matches!(fills[3], Fill::Recurrence(graph::Recur::Kept(k)) if k.prev == 5 && k.step.len() == 1));
    assert!(matches!(fills[4], Fill::Recurrence(graph::Recur::Part { part: graph::Part::Prev, kept: 4 })));
}

#[test]
fn faults_are_named() {
    let (base, id) = sample();
    type Edit = Box<dyn Fn(&mut Job)>;
    let (sq, call, looped, fetch, sw, one) = (id.sq, id.call, id.looped, id.fetch, id.sw, id.one);
    let (fd, ld, rd, md, sd) = (id.fn_decl, id.loop_decl, id.raw_decl, id.map_decl, id.scan_decl);
    let cases: Vec<(Edit, Span, Fault)> = vec![
        (Box::new(move |j| j.nodes[sq][1] = 99), Span::Nodes, Fault::Ref { found: 99 }),
        (Box::new(|j| j.args[0] = 50), Span::Args, Fault::Ref { found: 50 }),
        (Box::new(move |j| j.nodes[one + 3][0] = 12), Span::Nodes, Fault::Op { code: 12 }),
        (Box::new(move |j| j.nodes[one][0] = 5 << 8), Span::Nodes, Fault::Ty { code: 5 }),
        (Box::new(move |j| j.nodes[one][3] = 1000), Span::Nodes, Fault::Lit),
        (Box::new(move |j| j.nodes[one][2] = 3), Span::Nodes, Fault::Lit),
        (Box::new(|j| j.lits[0] = f64::NAN), Span::Lits, Fault::Lit),
        (Box::new(move |j| j.decls[fd].0 = 11), Span::Decls, Fault::Tag { found: 11 }),
        (Box::new(move |j| j.decls[ld].1[1] = 2), Span::Decls, Fault::Layout),
        (Box::new(move |j| j.decls[ld].1[2] = one as u32), Span::Decls, Fault::Layout),
        (Box::new(move |j| j.decls[rd].1[5] = 2), Span::Decls, Fault::Ref { found: 2 }),
        (Box::new(move |j| j.decls[rd].1[5] = 0), Span::Decls, Fault::Tag { found: 0 }),
        // A map run past the table's entries, a scan lane that is not a vec4, a fill kind that does not exist.
        (Box::new(move |j| j.decls[md].1[6] = 65), Span::Decls, Fault::Layout),
        (Box::new(move |j| j.decls[sd].2[2] = one as u32), Span::Decls, Fault::Layout),
        (Box::new(move |j| j.decls[sd].1[0] = 6), Span::Decls, Fault::Layout),
        (Box::new(|j| j.node_sites.push(0)), Span::NodeSites, Fault::Length),
        (Box::new(|j| j.sites[0][0] = 70), Span::Sites, Fault::Sym { found: 70 }),
        (Box::new(move |j| j.nodes[call][3] = 1), Span::Nodes, Fault::Tag { found: 1 }),
        (Box::new(move |j| j.nodes[id.x][0] = BIND + (1 << 16)), Span::Nodes, Fault::Flags { found: 2 }),
        (Box::new(move |j| j.nodes[fetch][0] = 6 | V4 | 3 << 16), Span::Nodes, Fault::Flags { found: 3 }),
        (
            Box::new(move |j| j.nodes[sw][3] = 2 | 1 << 2 | 1 << 8),
            Span::Nodes,
            Fault::Ref { found: 2 | 1 << 2 | 1 << 8 },
        ),
        // A Fn whose body is its own call, and a Loop whose next value is its own result: cycles through a decl.
        (Box::new(move |j| j.decls[fd].2[0] = call as u32), Span::Nodes, Fault::Order { root: call as u32 }),
        (Box::new(move |j| j.decls[ld].2[2] = looped as u32), Span::Nodes, Fault::Order { root: looped as u32 }),
    ];
    for (k, (edit, span, fault)) in cases.into_iter().enumerate() {
        let mut j = base.clone();
        edit(&mut j);
        let e = wire::decode(&j.bytes(0)).expect_err("a malformed job is refused");
        assert_eq!(wire_fault(&e), Some((span, fault)), "case {k}: {e:?}");
    }
    let bytes = base.bytes(0);
    let refused = |edit: &dyn Fn(&mut Vec<u8>)| {
        let mut b = bytes.clone();
        edit(&mut b);
        wire_fault(&wire::decode(&b).unwrap_err())
    };
    assert_eq!(refused(&|b| b[8] = 9), Some((Span::Header, Fault::Program { found: 9 })));
    assert_eq!(refused(&|b| b[20] = 1), Some((Span::Nodes, Fault::Bounds)), "a nonzero pad word");
    assert_eq!(refused(&|b| b.extend([0; 8])), Some((Span::Header, Fault::Length)), "bytes after the last span");
}

#[test]
fn a_type_error_is_the_authors_at_their_line() {
    let (mut j, id) = sample();
    j.nodes[id.last][0] = prim(Prim::Mix) | V2;
    match wire::decode(&j.bytes(0)) {
        Err(Error::Scene { site: Some(site), fault: SceneFault::Type { want, got } }) => {
            assert_eq!((site, want, got), (Site { file: "examples/owl.ts".into(), line: 12, col: 5 }, Ty::V3, Ty::V2));
        }
        other => panic!("expected a scene type error, got {other:?}"),
    }
}

/// The print of a small job, by hand from print.ts's grammar: a call inlined, a loop with its binders by level.
#[test]
fn print_known_answer() {
    let mut j = Job::default();
    let one = j.lit(F32, &[1.0]);
    let time = j.node(1 | F32, &[], 0);
    let a0 = j.sym("F0a0");
    let x = j.node(BIND | F32, &[], a0);
    let sq = j.node(prim(Prim::Mul) | F32, &[x, x], 0);
    let name = j.sym("square");
    j.decl(0, &[0, name, 0, 1, x], &[sq]);
    let (s0, si) = (j.sym("L0s0"), j.sym("L0i"));
    let s = j.node(BIND | F32, &[], s0);
    let k = j.node(BIND | F32, &[], si);
    let next = j.node(prim(Prim::Add) | F32, &[s, k], 0);
    j.decl(1, &[1, 0, s, k], &[one, time, next]);
    let looped = j.node(7 | F32, &[], 0);
    let call = j.node(5 | F32, &[looped], 0);
    j.roots.extend([call, time]);
    let g = wire::decode(&j.bytes(0)).unwrap().graph;
    let want = "lit f32 3ff0000000000000\ntime\nbind f32 0 s0\nbind f32 0 i\nadd f32 #2 #3\n\
                loop #0 [#1] -> [#4] until -\niter f32 0 #5\nmul f32 #6 #6\nroots #7 #1";
    assert_eq!(loft_ir::print::print(&g).unwrap(), want);
}

/// The kernel lines of the print: a map run and a scan, walked after the roots over `fill` binders.
#[test]
fn print_fills_known_answer() {
    let mut j = Job::default();
    let one = j.lit(F32, &[1.0]);
    let (tab, ki) = (j.sym("tab"), j.sym("K0r0i"));
    let ri = j.node(BIND | F32, &[], ki);
    let lane = j.node(11 | V4, &[ri], 0);
    j.decl(2, &[0, tab, 1, 4, 1, 1, 4, ri], &[lane]);
    let (scan, ss, si) = (j.sym("scan"), j.sym("K1s0"), j.sym("K1i"));
    let (sb, sk) = (j.node(BIND | F32, &[], ss), j.node(BIND | F32, &[], si));
    let (sn, sl) = (j.node(prim(Prim::Add) | F32, &[sb, sk], 0), j.node(11 | V4, &[sb], 0));
    j.decl(2, &[2, scan, 1, 2, 2, 0, 2, 1, sb, sk], &[one, sn, sl]);
    let f0 = j.node(6 | V4, &[one], 0);
    let f1 = j.node(6 | V4, &[one], 1);
    j.roots.extend([f0, f1]);
    let g = wire::decode(&j.bytes(0)).unwrap().graph;
    let want = "lit f32 3ff0000000000000\nfetch table:tab:1:4:data 0 #0\nfetch table:scan:1:2:static 0 #0\n\
                fill f32 i\nvec vec4 #3\nfill f32 s0\nadd f32 #5 #3\nvec vec4 #5\nroots #1 #2\n\
                kernel table:tab:1:4:data map 4 [#4]\nkernel table:scan:1:2:static scan 0 2 [#0] -> [#6] [#7]";
    assert_eq!(loft_ir::print::print(&g).unwrap(), want);
}

/// Lane 1.2's pass condition (plan §8.2): over the corpus, the Rust print of each job equals the TS graph's print.
/// `LOFT_WIRE_CORPUS` is a folder of `NAME.job` (score.ts `--dump`) and `NAME.txt` (`--print`) pairs.
#[test]
#[ignore = "needs LOFT_WIRE_CORPUS: a folder of score.ts --dump/--print pairs"]
fn corpus_prints_equal() {
    let dir = std::env::var("LOFT_WIRE_CORPUS").expect("LOFT_WIRE_CORPUS names the corpus folder");
    let mut jobs: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "job"))
        .collect();
    jobs.sort();
    let mut fails = Vec::new();
    for p in &jobs {
        let want = std::fs::read_to_string(p.with_extension("txt")).unwrap();
        let bytes = std::fs::read(p).unwrap();
        // The print recurses per graph level; Titan's deepest chains need more than a test thread's stack.
        let got = std::thread::Builder::new()
            .stack_size(1 << 30)
            .spawn(move || wire::decode(&bytes).and_then(|j| loft_ir::print::print(&j.graph)))
            .unwrap()
            .join()
            .unwrap();
        let name = p.file_stem().unwrap().to_string_lossy().into_owned();
        match got {
            Ok(text) if text == want => println!("equal {name}"),
            Ok(text) => {
                let (a, b) = (want.lines().collect::<Vec<_>>(), text.lines().collect::<Vec<_>>());
                let k = (0..a.len().max(b.len())).find(|k| a.get(*k) != b.get(*k)).unwrap_or(0);
                fails.push(format!("{name}: line {k}: ts «{:?}» rust «{:?}»", a.get(k), b.get(k)));
            }
            Err(e) => fails.push(format!("{name}: {e:?}")),
        }
    }
    println!("corpus print equality: {} of {} jobs", jobs.len() - fails.len(), jobs.len());
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}

/// xorshift64*: a fixed stream, so a failure replays.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// 10^5 mutated jobs: none panics; every refusal is typed. Mutations that land in payload bytes (rows, blobs, literal
/// values, text) can leave a well-formed job, which must then decode; the counts are printed for the lane note.
#[test]
fn fuzz_never_panics() {
    let base = sample().0.bytes(0);
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let (mut refused, mut accepted) = (0u32, 0u32);
    let mut kinds = std::collections::BTreeMap::<String, u32>::new();
    for _ in 0..100_000 {
        let mut b = base.clone();
        match rng.below(4) {
            0 => (0..1 + rng.below(8)).for_each(|_| {
                let i = rng.below(b.len());
                b[i] ^= 1 << rng.below(8);
            }),
            1 => {
                let i = rng.below(b.len() / 4) * 4;
                let w = [0, 1, u32::MAX, rng.next() as u32, rng.below(64) as u32][rng.below(5)];
                b[i..i + 4].copy_from_slice(&w.to_le_bytes());
            }
            2 => b.truncate(rng.below(b.len())),
            _ => b = (0..rng.below(400)).map(|_| rng.next() as u8).collect(),
        }
        match wire::decode(&b) {
            Ok(job) => {
                accepted += 1;
                // A job that decodes prints or refuses by name; it never panics either.
                let _ = loft_ir::print::print(&job.graph);
            }
            Err(e) => {
                refused += 1;
                let key = wire_fault(&e).map_or(e.kind().to_string(), |(s, f)| {
                    format!("{s:?}/{f:?}").split([' ', '{']).next().unwrap().into()
                });
                *kinds.entry(key).or_default() += 1;
            }
        }
    }
    println!("fuzz: {refused} refused, {accepted} still well formed, 0 panics; refusals by kind: {kinds:?}");
    assert!(refused > 90_000, "refused only {refused}");
}

#[test]
fn quota_is_a_budget() {
    let mut b = sample().0.bytes(0);
    b[16..20].copy_from_slice(&((wire::MAX_NODES as u32 + 1) * 16).to_le_bytes());
    assert!(matches!(wire::decode(&b), Err(Error::Budget { spent, .. }) if spent == wire::MAX_NODES + 1));
}

fn bundle() -> Bundle {
    let f = |x: f32| loft_ir::bundle::F32(x.to_bits());
    Bundle {
        row_steps: vec![],
        format: Format::V1,
        compiler: "c8502c2".into(),
        names: ["view".into(), "main".into()].into(),
        resources: [
            Resource { shape: Shape::Rows { rows: 64, lanes: 2 }, init: Init::Bytes(0) },
            Resource { shape: Shape::Texture { extent: [8, 8, 1], format: TexFormat::Rgba16Float }, init: Init::By(0) },
            Resource { shape: Shape::ViewSized { format: TexFormat::Depth32Float, scale: f(0.5) }, init: Init::Zero },
            Resource { shape: Shape::Frame, init: Init::Zero },
        ]
        .into(),
        packs: [Pack { access: Access::Write, vec4s: 64 * 2 * 91, holds: [(0, 0)].into() }].into(),
        kernels: [
            Kernel {
                writes: [1].into(),
                reads: [0].into(),
                body: Body::Map { module: 0, entry: 1, rows: 64 },
                when: When::Frame,
            },
            Kernel {
                writes: [0].into(),
                reads: [].into(),
                body: Body::Raw { module: 1, entry: 1, dispatch: [8, 1, 1] },
                when: When::Steps { dt: f(1.0 / 60.0), from: f(0.0), count: 360, keep: Keep::Every(4) },
            },
        ]
        .into(),
        modules: [
            Module { wgsl: 1, origin: Origin::Library, precision_alias: true },
            Module { wgsl: 2, origin: Origin::Author, precision_alias: false },
        ]
        .into(),
        draws: [
            Draw {
                name: 0,
                layer: Layer::Subject,
                cover: Cover::Ray { proxy: Proxy::Box { min: [f(-1.0); 3], max: [f(1.0); 3] }, find: Find::Hit },
                shade: (0, 1),
                raster: Raster { cull: Cull::Back, depth_write: true, blend: Blend::Opaque },
            },
            Draw {
                name: 1,
                layer: Layer::Surroundings,
                cover: Cover::Raster { prim: RasterPrim::Grid { u: 64, v: 32 }, count: 1 },
                shade: (0, 1),
                raster: Raster { cull: Cull::None, depth_write: false, blend: Blend::Alpha },
            },
        ]
        .into(),
        passes: [Pass {
            target: Target::View,
            camera: 0,
            draws: [0, 2],
            clear: Clear::Color([f(0.1), f(0.2), f(0.3), f(1.0)]),
            compose: Compose { outline: Some((0, 1)), ground: None },
        }]
        .into(),
        media: [Medium { transmittance: (0, 1), integrate: Integrate::March { steps: 64, shadow_steps: 16 } }].into(),
        cameras: [Camera { name: 0, lens: (0, 0) }].into(),
        params: [ParamDecl { name: 1, def: f(0.5), min: f(0.0), max: f(1.0) }].into(),
        inputs: [InputDecl { kind: InputKind::Level, name: 0, ty: Ty::V3, def: [f(0.0), f(1.0), f(-0.0)].into() }]
            .into(),
        sites: [Site { file: "examples/owl.ts".into(), line: 3, col: 1 }].into(),
        sections: [vec![1u8; 100].into(), b"fn main() {}".to_vec().into(), vec![].into()].into(),
    }
}

#[test]
fn bundle_round_trip_is_byte_identical() {
    let a = bundle().write().unwrap();
    let back = Bundle::read(&a).unwrap();
    assert_eq!(back, bundle());
    assert_eq!(back.write().unwrap(), a);
    let mut v2 = a.clone();
    v2[4] = 2;
    assert!(
        matches!(Bundle::read(&v2), Err(Error::Load(loft_ir::error::LoadFault::FormatMismatch { found: 2, compiler: Some(c), .. })) if &*c == "c8502c2")
    );
    for cut in [3, 15, 40, a.len() - 1] {
        assert!(matches!(Bundle::read(&a[..cut]), Err(Error::Load(_))), "cut at {cut}");
    }
    let mut long = a.clone();
    long.push(0);
    assert!(matches!(Bundle::read(&long), Err(Error::Load(loft_ir::error::LoadFault::Trailing { .. }))));
}

#[test]
fn every_identity_field_moves_the_hash() {
    let base = Identity {
        build: identity::build(&bundle().write().unwrap()),
        frame: FrameKey {
            steps: vec![],
            camera: 0,
            time: 0.5f32.to_bits(),
            size: [512, 512],
            params: [1, 2].into(),
            inputs: [3].into(),
            precision: Precision::F32,
        },
        device: Device { backend: Backend::Metal, adapter: "Apple M4".into(), driver: "".into() },
    };
    let edits: [fn(&mut Identity); 10] = [
        |i| i.build[31] ^= 1,
        |i| i.frame.camera = 1,
        |i| i.frame.time = 1.37f32.to_bits(),
        |i| i.frame.size = [512, 513],
        |i| i.frame.params = [1, 3].into(),
        |i| i.frame.inputs = [].into(),
        |i| i.frame.precision = Precision::Mixed,
        |i| i.device.backend = Backend::Vulkan,
        |i| i.device.adapter = "Apple M4 Pro".into(),
        |i| i.device.driver = "x".into(),
    ];
    let mut seen = std::collections::HashSet::from([base.hash()]);
    for (k, edit) in edits.iter().enumerate() {
        let mut i = base.clone();
        edit(&mut i);
        assert!(seen.insert(i.hash()), "edit {k} left the hash unchanged");
    }
}
