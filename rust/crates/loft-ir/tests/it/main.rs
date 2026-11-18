//! Lane 1.1's checks (plan §8.2): the wire refuses fuzzed malformed jobs with typed errors and no panic; a bundle
//! writes, reads and writes to the same bytes; every Identity field moves the hash.
use loft_ir::bundle::*;
use loft_ir::error::{Error, Fault, Internal, SceneFault, Span};
use loft_ir::graph::{Leaf, Op, Prim, Program, Site, Ty};
use loft_ir::identity::{self, Backend, Device, FrameKey, Identity, Precision};
use loft_ir::wire::{self, MAGIC, WIRE};

/// A job writer in the wire's layout (the recorder's job in TS; here only to make test jobs).
#[derive(Default, Clone)]
struct Job {
    nodes: Vec<[u32; 4]>,
    args: Vec<u32>,
    lits: Vec<f64>,
    syms: Vec<String>,
    sites: Vec<[u32; 3]>,
    node_sites: Vec<u32>,
    decls: Vec<[u32; 4]>,
    roots: Vec<u32>,
}

const fn ty(t: u32) -> u32 {
    t << 8
}
const F32: u32 = ty(0);
const V3: u32 = ty(2);
const BOOL: u32 = ty(4);
fn prim(p: Prim) -> u32 {
    16 + Prim::ALL.iter().position(|q| *q == p).unwrap() as u32
}

impl Job {
    fn node(&mut self, head: u32, args: &[u32], imm: u32) -> u32 {
        self.nodes.push([head, self.args.len() as u32, args.len() as u32, imm]);
        self.args.extend_from_slice(args);
        self.node_sites.push(0);
        self.nodes.len() as u32 - 1
    }
    fn lit(&mut self, head: u32, v: &[f64]) -> u32 {
        let at = self.lits.len() as u32;
        self.lits.extend_from_slice(v);
        self.node(head, &[], at)
    }
    fn sym(&mut self, s: &str) -> u32 {
        self.syms.push(s.into());
        self.syms.len() as u32 - 1
    }
    fn decl(&mut self, tag: u32, arity: u32, roots: &[u32]) -> u32 {
        self.decls.push([tag | arity << 8, u32::MAX, self.roots.len() as u32, roots.len() as u32]);
        self.roots.extend_from_slice(roots);
        self.decls.len() as u32 - 1
    }
    fn bytes(&self, program: u32) -> Vec<u8> {
        let flat = |v: &[[u32; 4]]| v.iter().flatten().flat_map(|w| w.to_le_bytes()).collect::<Vec<u8>>();
        let le = |v: &[u32]| v.iter().flat_map(|w| w.to_le_bytes()).collect::<Vec<u8>>();
        let mut ends = Vec::new();
        let mut text = String::new();
        for s in &self.syms {
            text.push_str(s);
            ends.push(text.len() as u32);
        }
        let spans: [Vec<u8>; 11] = [
            flat(&self.nodes),
            le(&self.args),
            self.lits.iter().flat_map(|x| x.to_bits().to_le_bytes()).collect(),
            le(&[1.5f32.to_bits(), 2.0f32.to_bits()]),
            vec![7, 8, 9],
            text.into_bytes(),
            le(&ends),
            le(&self.sites.concat()),
            le(&self.node_sites),
            flat(&self.decls),
            le(&self.roots),
        ];
        let mut out = [MAGIC, WIRE, program, 11].iter().flat_map(|w| w.to_le_bytes()).collect::<Vec<u8>>();
        out.resize(16 + 88, 0);
        for (i, s) in spans.iter().enumerate() {
            out.resize(out.len().next_multiple_of(8), 0);
            let (off, len) = (out.len() as u32, s.len() as u32);
            out[16 + 8 * i..20 + 8 * i].copy_from_slice(&off.to_le_bytes());
            out[20 + 8 * i..24 + 8 * i].copy_from_slice(&len.to_le_bytes());
            out.extend_from_slice(s);
        }
        out
    }
}

/// One job using every op and every checked decl layout.
fn sample() -> Job {
    let mut j = Job::default();
    let file = j.sym("examples/owl.ts");
    j.sites.push([file, 12, 5]);
    let one = j.lit(F32, &[1.0]);
    let two = j.lit(F32, &[2.0]);
    let up = j.lit(V3, &[0.0, 1.0, 0.0]);
    let time = j.node(1 | F32, &[], 0);
    let p = j.decl(8, 0, &[one, one, two]);
    let param = j.node(1 | F32 | 1 << 16, &[], p);
    let i = j.decl(9, 0, &[up]);
    let input = j.node(1 | V3 | 2 << 16, &[], i);
    let x = j.node(1 | F32 | 3 << 16, &[], 0);
    let sq = j.node(prim(Prim::Mul) | F32, &[x, x], 0);
    let f = j.decl(0, 1, &[x, sq]);
    let call = j.node(2 | F32, &[time], f);
    let s = j.node(1 | F32 | 3 << 16, &[], 1);
    let k = j.node(1 | F32 | 3 << 16, &[], 2);
    let next = j.node(prim(Prim::Add) | F32, &[s, k], 0);
    let stop = j.node(prim(Prim::Gt) | BOOL, &[next, two], 0);
    let l = j.decl(1, 1, &[two, one, s, k, next, stop]);
    let looped = j.node(4 | F32, &[], l);
    let kernel = j.decl(2, 0, &[]);
    let fetch = j.node(3 | ty(3) | 2 << 16, &[call], kernel);
    let sw = j.node(7 | ty(1), &[fetch], 2 | (2 << 3)); // z then x
    let v = j.node(prim(Prim::Vec) | V3, &[sw, looped], 0);
    let scaled = j.node(prim(Prim::Mul) | V3, &[v, param], 0);
    let grad = j.node(5 | V3, &[call, input], 0);
    let raw = j.sym("fn wobble(x: f32) -> f32 { return x; }");
    let r = j.node(6 | F32, &[time], raw);
    let pick = j.node(prim(Prim::Pick) | V3, &[r, scaled, grad, up], 0);
    *j.node_sites.last_mut().unwrap() = 1;
    let _draw = j.decl(3, 0, &[pick]);
    j
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
    let job = wire::decode(&sample().bytes(1)).expect("the sample is well formed");
    assert_eq!(job.program, Program::Solve);
    let g = &job.graph;
    assert_eq!(g.nodes.last().unwrap().ty, Ty::V3);
    assert_eq!(g.site(g.nodes.len() as u32 - 1).map(|s| s.to_string()), Some("examples/owl.ts:12:5".into()));
    assert!(matches!(g.nodes[3].op, Op::Leaf(Leaf::Time)));
    assert_eq!(g.sym(1), "fn wobble(x: f32) -> f32 { return x; }");
}

#[test]
fn faults_are_named() {
    let base = sample();
    type Edit = Box<dyn Fn(&mut Job)>;
    let cases: Vec<(Edit, Span, Fault)> = vec![
        (Box::new(|j| j.nodes[4][1] = 99), Span::Nodes, Fault::Ref { found: 99 }),
        (Box::new(|j| j.args[0] = 50), Span::Args, Fault::Ref { found: 50 }),
        (Box::new(|j| j.nodes[0][0] = 9), Span::Nodes, Fault::Op { code: 9 }),
        (Box::new(|j| j.nodes[0][0] = 5 << 8), Span::Nodes, Fault::Ty { code: 5 }),
        (Box::new(|j| j.nodes[0][3] = 1000), Span::Nodes, Fault::Lit),
        (Box::new(|j| j.lits[0] = f64::NAN), Span::Lits, Fault::Lit),
        (Box::new(|j| j.decls[0][0] = 11), Span::Decls, Fault::Tag { found: 11 }),
        (Box::new(|j| j.decls[0][3] = 2), Span::Decls, Fault::Layout),
        (Box::new(|j| j.node_sites.push(0)), Span::NodeSites, Fault::Length),
        (Box::new(|j| j.sites[0][0] = 7), Span::Sites, Fault::Sym { found: 7 }),
    ];
    for (k, (edit, span, fault)) in cases.into_iter().enumerate() {
        let mut j = base.clone();
        edit(&mut j);
        let e = wire::decode(&j.bytes(0)).expect_err("a malformed job is refused");
        assert_eq!(wire_fault(&e), Some((span, fault)), "case {k}: {e:?}");
    }
    let mut bytes = base.bytes(0);
    bytes[8] = 9;
    assert_eq!(wire_fault(&wire::decode(&bytes).unwrap_err()), Some((Span::Header, Fault::Program { found: 9 })));
}

#[test]
fn a_type_error_is_the_authors_at_their_line() {
    let mut j = sample();
    let last = j.nodes.len() - 1;
    j.nodes[last][0] = prim(Prim::Pick) | ty(1);
    match wire::decode(&j.bytes(0)) {
        Err(Error::Scene { site: Some(site), fault: SceneFault::Type { want, got } }) => {
            assert_eq!((site, want, got), (Site { file: "examples/owl.ts".into(), line: 12, col: 5 }, Ty::V3, Ty::V2));
        }
        other => panic!("expected a scene type error, got {other:?}"),
    }
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
    let base = sample().bytes(0);
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
            Ok(_) => accepted += 1,
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
    let mut b = sample().bytes(0);
    let (n, off) = ((wire::MAX_NODES as u32 + 1) * 16, b.len().next_multiple_of(8));
    b.resize(off + n as usize, 0);
    b[16..20].copy_from_slice(&(off as u32).to_le_bytes());
    b[20..24].copy_from_slice(&n.to_le_bytes());
    assert!(matches!(wire::decode(&b), Err(Error::Budget { spent, .. }) if spent == wire::MAX_NODES + 1));
}

fn bundle() -> Bundle {
    let f = |x: f32| loft_ir::bundle::F32(x.to_bits());
    Bundle {
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
                raster: Raster { cull: true, depth_write: true, blend: Blend::Opaque },
            },
            Draw {
                name: 1,
                layer: Layer::Surroundings,
                cover: Cover::Raster { prim: RasterPrim::Grid { u: 64, v: 32 }, count: 1 },
                shade: (0, 1),
                raster: Raster { cull: false, depth_write: false, blend: Blend::Alpha },
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
        cameras: [(0, 0)].into(),
        params: [ParamDecl { name: 1, def: f(0.5), min: f(0.0), max: f(1.0) }].into(),
        inputs: [InputDecl { name: 0, ty: Ty::V3, def: [f(0.0), f(1.0), f(-0.0)].into() }].into(),
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
}

#[test]
fn every_identity_field_moves_the_hash() {
    let base = Identity {
        build: identity::build(&bundle().write().unwrap()),
        frame: FrameKey {
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
