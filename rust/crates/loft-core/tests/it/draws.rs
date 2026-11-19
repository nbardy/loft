//! Lane 3.1 checks (lean Rust plan §8.2) that need no GPU: every cover, both media and the composer lower to modules
//! Naga accepts under both `lf` aliases; a draw writing no depth is veiled by its pass's media and one writing depth is
//! not; ground and outline logic exist in exactly one place each (the grep gate).
use loft_core::draw::{Cover, Draw, Find, Medium, Pass, Prim as P, check, lower};
use loft_core::emit::{Row, Slots};
use loft_ir::bundle::Access;
use loft_ir::bundle::{self as b, Blend, Clear, Cull, F32, Layer, Proxy, Raster, Target};
use loft_ir::graph::{Graph, GraphFn, Node, Op, Prim, Ty};

#[derive(Default)]
struct B {
    nodes: Vec<Node>,
    args: Vec<u32>,
    lits: Vec<u64>,
    fns: Vec<GraphFn>,
    syms: String,
    /// Symbol starts; `graph` appends the end.
    starts: Vec<u32>,
    /// Layout's place per kernel id (one read pack); the draws read no params or inputs.
    at_kernels: Vec<Row>,
    bind: u32,
}

impl B {
    fn node(&mut self, op: Op, ty: Ty, args: &[u32]) -> u32 {
        self.nodes.push(Node { op, ty, args: self.args.len() as u32, n: args.len() as u32 });
        self.args.extend_from_slice(args);
        (self.nodes.len() - 1) as u32
    }
    fn p(&mut self, p: Prim, ty: Ty, args: &[u32]) -> u32 {
        self.node(Op::Prim(p), ty, args)
    }
    fn lit(&mut self, v: &[f64], ty: Ty) -> u32 {
        let i = self.lits.len() as u32;
        self.lits.extend(v.iter().map(|x| x.to_bits()));
        self.node(Op::Lit(i), ty, &[])
    }
    fn arg(&mut self, ty: Ty) -> u32 {
        self.bind += 1;
        self.node(Op::Bind(self.bind), ty, &[])
    }
    /// A kernel id whose resource layout places at `at`.
    fn kernel(&mut self, [res, lanes]: [u32; 2]) -> u32 {
        let offset = self.at_kernels.iter().map(|r| u64::from(r.rows * r.lanes)).sum();
        self.at_kernels.push(Row { res, pack: 0, offset, rows: 16, lanes });
        (self.at_kernels.len() - 1) as u32
    }
    fn func(&mut self, binders: &[u32], body: u32) -> u32 {
        let ret = self.nodes[body as usize].ty;
        self.fns.push(GraphFn { name: 0, ret, binders: binders.into(), body });
        (self.fns.len() - 1) as u32
    }
    fn sym(&mut self, s: &str) -> u32 {
        self.starts.push(self.syms.len() as u32);
        self.syms.push_str(s);
        (self.starts.len() - 1) as u32
    }
    fn slots(&self) -> Slots {
        Slots {
            params: Box::new([]),
            inputs: Box::new([]),
            kernels: self.at_kernels.clone().into(),
            packs: Box::new([Access::Read]),
        }
    }
    fn graph(&self) -> Graph {
        let n = self.nodes.len();
        let mut sym_at = self.starts.clone();
        sym_at.push(self.syms.len() as u32);
        #[rustfmt::skip]
        let g = Graph { nodes: self.nodes.clone().into(), args: self.args.clone().into(), lits: self.lits.clone().into(),
            rows: Box::new([]), blobs: Box::new([]), syms: self.syms.clone().into(), sym_at: sym_at.into(),
            sites: Box::new([]), node_sites: vec![0; n].into(), fns: self.fns.clone().into(), raws: Box::new([]),
            loops: Box::new([]), kernels: Box::new([]), params: Box::new([]), inputs: Box::new([]), roots: Box::new([]), ..Graph::default() };
        g
    }
}

/// The library functions a scene would record, as small graphs: a lit paint, a sphere hit, a wavy grid, a ring of
/// specks, a ball of density, and an ink rule.
struct Fns {
    paint: u32,
    hit: u32,
    grid_normal: u32,
    grid_cells: u32,
    grid: u32,
    quads: u32,
    density: u32,
    outline: u32,
    ground_hit: u32,
}

fn library(b: &mut B) -> Fns {
    let (p, n, _uv, id) = (b.arg(Ty::V3), b.arg(Ty::V3), b.arg(Ty::V2), b.arg(Ty::F32));
    let sun = b.lit(&[0.3, 0.8, 0.5], Ty::V3);
    let lam = b.p(Prim::Dot, Ty::F32, &[n, sun]);
    let lam = b.p(Prim::Saturate, Ty::F32, &[lam]);
    let k = b.lit(&[0.1], Ty::F32);
    let tint = b.p(Prim::Mul, Ty::F32, &[id, k]);
    let base = b.p(Prim::Fract, Ty::V3, &[p]);
    let c = b.p(Prim::Mul, Ty::V3, &[base, lam]);
    let c = b.p(Prim::Add, Ty::V3, &[c, tint]);
    let mark = b.lit(&[0.49], Ty::F32);
    let rgba = b.node(Op::Vec, Ty::V4, &[c, mark]);
    let paint = b.func(&[p, n, _uv, id], rgba);

    // A sphere of radius 1 at (0, 1, 0): (normal, t), t = -1 on a miss.
    let (o, d, span, w, _copy) = (b.arg(Ty::V3), b.arg(Ty::V3), b.arg(Ty::V2), b.arg(Ty::V2), b.arg(Ty::F32));
    let centre = b.lit(&[0.0, 1.0, 0.0], Ty::V3);
    let oc = b.p(Prim::Sub, Ty::V3, &[o, centre]);
    let bb = b.p(Prim::Dot, Ty::F32, &[oc, d]);
    let cc = b.p(Prim::Dot, Ty::F32, &[oc, oc]);
    let one = b.lit(&[1.0], Ty::F32);
    let cc = b.p(Prim::Sub, Ty::F32, &[cc, one]);
    let b2 = b.p(Prim::Mul, Ty::F32, &[bb, bb]);
    let h = b.p(Prim::Sub, Ty::F32, &[b2, cc]);
    let zero = b.lit(&[0.0], Ty::F32);
    let hs = b.p(Prim::Max, Ty::F32, &[h, zero]);
    let s = b.p(Prim::Sqrt, Ty::F32, &[hs]);
    let nb = b.p(Prim::Neg, Ty::F32, &[bb]);
    let t = b.p(Prim::Sub, Ty::F32, &[nb, s]);
    let far = b.node(Op::Swizzle(loft_ir::graph::Sel { len: 1, comps: 1 }), Ty::F32, &[span]);
    let miss = b.p(Prim::Lt, Ty::Bool, &[h, zero]);
    let beyond = b.p(Prim::Gt, Ty::Bool, &[t, far]);
    let miss = b.p(Prim::Or, Ty::Bool, &[miss, beyond]);
    let neg = b.lit(&[-1.0], Ty::F32);
    let t = b.p(Prim::Select, Ty::F32, &[miss, neg, t]);
    let at = b.p(Prim::Mul, Ty::V3, &[d, t]);
    let at = b.p(Prim::Add, Ty::V3, &[o, at]);
    let nrm = b.p(Prim::Sub, Ty::V3, &[at, centre]);
    let nrm = b.p(Prim::Normalize, Ty::V3, &[nrm]);
    let wx = b.node(Op::Swizzle(loft_ir::graph::Sel { len: 1, comps: 0 }), Ty::F32, &[w]);
    let nrm = b.p(Prim::Add, Ty::V3, &[nrm, wx]);
    let out = b.node(Op::Vec, Ty::V4, &[nrm, t]);
    let hit = b.func(&[o, d, span, w, _copy], out);

    // The plane y = 0 as the ground's hit.
    let (o, d, span, w, copy) = (b.arg(Ty::V3), b.arg(Ty::V3), b.arg(Ty::V2), b.arg(Ty::V2), b.arg(Ty::F32));
    let oy = b.node(Op::Swizzle(loft_ir::graph::Sel { len: 1, comps: 1 }), Ty::F32, &[o]);
    let dy = b.node(Op::Swizzle(loft_ir::graph::Sel { len: 1, comps: 1 }), Ty::F32, &[d]);
    let noy = b.p(Prim::Neg, Ty::F32, &[oy]);
    let t = b.p(Prim::Div, Ty::F32, &[noy, dy]);
    let up = b.lit(&[0.0, 1.0, 0.0], Ty::V3);
    let wy = b.node(Op::Swizzle(loft_ir::graph::Sel { len: 1, comps: 1 }), Ty::F32, &[w]);
    let sy = b.node(Op::Swizzle(loft_ir::graph::Sel { len: 1, comps: 0 }), Ty::F32, &[span]);
    let t = b.p(Prim::Max, Ty::F32, &[t, sy]);
    let t = b.p(Prim::Add, Ty::F32, &[t, wy]);
    let out = b.node(Op::Vec, Ty::V4, &[up, t]);
    let ground_hit = b.func(&[o, d, span, w, copy], out);

    let (uv, id) = (b.arg(Ty::V2), b.arg(Ty::F32));
    let u = b.node(Op::Swizzle(loft_ir::graph::Sel { len: 1, comps: 0 }), Ty::F32, &[uv]);
    let v = b.node(Op::Swizzle(loft_ir::graph::Sel { len: 1, comps: 1 }), Ty::F32, &[uv]);
    let time = b.node(Op::Time, Ty::F32, &[]);
    let wave = b.p(Prim::Add, Ty::F32, &[u, time]);
    let wave = b.p(Prim::Sin, Ty::F32, &[wave]);
    let y = b.p(Prim::Add, Ty::F32, &[wave, id]);
    let pos = b.node(Op::Vec, Ty::V3, &[u, y, v]);
    let grid = b.func(&[uv, id], pos);
    let (uv2, id2) = (b.arg(Ty::V2), b.arg(Ty::F32));
    let up = b.lit(&[0.0, 1.0, 0.0], Ty::V3);
    let up = b.p(Prim::Add, Ty::V3, &[up, id2]);
    let u2 = b.node(Op::Swizzle(loft_ir::graph::Sel { len: 1, comps: 0 }), Ty::F32, &[uv2]);
    let up = b.p(Prim::Add, Ty::V3, &[up, u2]);
    let grid_normal = b.func(&[uv2, id2], up);
    let id3 = b.arg(Ty::F32);
    let cells = b.lit(&[32.0, 16.0], Ty::V2);
    let cells = b.p(Prim::Add, Ty::V2, &[cells, id3]);
    let grid_cells = b.func(&[id3], cells);

    let id = b.arg(Ty::F32);
    let a = b.p(Prim::Mul, Ty::F32, &[id, k]);
    let (x, z) = (b.p(Prim::Cos, Ty::F32, &[a]), b.p(Prim::Sin, Ty::F32, &[a]));
    let r = b.lit(&[0.05], Ty::F32);
    let q = b.node(Op::Vec, Ty::V4, &[x, one, z, r]);
    let quads = b.func(&[id], q);

    let p = b.arg(Ty::V3);
    let len = b.p(Prim::Distance, Ty::F32, &[p, centre]);
    let dens = b.p(Prim::Sub, Ty::F32, &[one, len]);
    let density = b.func(&[p], dens);

    let (c, q) = (b.arg(Ty::V3), b.arg(Ty::V3));
    let half = b.lit(&[0.5], Ty::F32);
    let ink = b.p(Prim::Mul, Ty::V3, &[c, half]);
    let qy = b.node(Op::Swizzle(loft_ir::graph::Sel { len: 1, comps: 1 }), Ty::F32, &[q]);
    let wd = b.p(Prim::Mul, Ty::F32, &[qy, k]);
    let wd = b.p(Prim::Abs, Ty::F32, &[wd]);
    let o4 = b.node(Op::Vec, Ty::V4, &[ink, wd]);
    let outline = b.func(&[c, q], o4);
    Fns { paint, hit, grid, grid_normal, grid_cells, quads, density, outline, ground_hit }
}

fn f(x: f32) -> F32 {
    F32(x.to_bits())
}
fn bx(lo: f32, hi: f32) -> [[F32; 3]; 2] {
    [[f(lo); 3], [f(hi); 3]]
}

const OPAQUE: Raster = Raster { cull: Cull::Back, depth_write: true, blend: Blend::Opaque };
const GLOW: Raster = Raster { cull: Cull::None, depth_write: false, blend: Blend::Add };

/// One scene with every cover, both media, a shadow pass and a view pass with ground and outline.
fn scene() -> (B, Vec<Draw>, Vec<Medium>, Vec<Pass>) {
    let mut b = B::default();
    let fns = library(&mut b);
    let mesh = b.kernel([3, 2]);
    let inst = b.kernel([4, 1]);
    let blobs = b.kernel([5, 2]);
    let name = b.sym("draw");
    let draw = |cover, raster| Draw { name, layer: Layer::Subject, cover, paint: fns.paint, raster };
    let ray = |proxy, find| Cover::Ray { proxy, find };
    let ball = Proxy::Box { min: bx(-1.0, 0.0)[0], max: bx(0.0, 1.0)[1] };
    let draws = vec![
        draw(ray(ball.clone(), Find::Hit(fns.hit)), OPAQUE),
        draw(ray(Proxy::Instances { res: inst, local: bx(-0.5, 0.5) }, Find::Hit(fns.hit)), OPAQUE),
        draw(
            Cover::Raster {
                prim: P::Grid { u: 32, v: 16, place: fns.grid, normal: fns.grid_normal, cells: fns.grid_cells },
                count: 2,
            },
            OPAQUE,
        ),
        draw(Cover::Raster { prim: P::Mesh { res: mesh }, count: 1 }, OPAQUE),
        draw(ray(Proxy::Box { min: bx(-2.0, 2.0)[0], max: bx(-2.0, 2.0)[1] }, Find::Medium(0)), GLOW),
        draw(ray(Proxy::Box { min: bx(-3.0, 3.0)[0], max: bx(-3.0, 3.0)[1] }, Find::Medium(1)), GLOW),
        draw(Cover::Raster { prim: P::Quads { place: fns.quads }, count: 500 }, GLOW),
        draw(Cover::Screen, GLOW),
    ];
    let media = vec![
        Medium::March { density: fns.density, within: bx(-2.0, 2.0), steps: 48, shadow_steps: 8 },
        Medium::Gaussians { res: blobs },
    ];
    #[rustfmt::skip]
    let passes = vec![
        Pass { target: Target::Shadow { light: 0, map: 9 }, camera: 1, draws: [0, 4], clear: Clear::Load, ground: None, outline: None },
        Pass { target: Target::View, camera: 0, draws: [0, 7], clear: Clear::Color([f(0.0); 4]), ground: Some([fns.ground_hit, fns.paint]), outline: Some(fns.outline) },
        Pass { target: Target::View, camera: 0, draws: [7, 8], clear: Clear::Load, ground: None, outline: None },
    ];
    (b, draws, media, passes)
}

#[test]
fn every_cover_medium_and_composer_validates_both_ways() {
    let (b, draws, media, passes) = scene();
    let g = b.graph();
    let l = lower(&g, &b.slots(), &draws, &media, &passes).unwrap_or_else(|e| panic!("{e:?}"));
    for (k, m) in l.modules.iter().enumerate() {
        check(m, true).unwrap_or_else(|e| panic!("module {k} refused as f16: {e:?}\n{m}"));
    }
    // 8 draws, a ground and an outline; the medium entries live in their draws' modules.
    assert_eq!((l.modules.len(), l.draws.len(), l.media.len(), l.passes.len()), (10, 8, 2, 3));
    assert!(matches!(l.media[1].integrate, b::Integrate::Gaussians { res: 5 }));
    assert!(matches!(l.draws[1].cover, b::Cover::Ray { proxy: Proxy::Instances { res: 4, .. }, find: b::Find::Hit }));
    // A Ray cover culls back faces whatever its draw asked (the inward proxy runs each pixel once); others keep theirs.
    assert!(
        l.draws[4].raster.cull == Cull::Back
            && l.draws[5].raster.cull == Cull::Back
            && l.draws[6].raster.cull == Cull::None
    );
    let veil = |d: usize| l.modules[l.draws[d].shade.0 as usize].lines().find(|t| t.starts_with("fn veil")).unwrap();
    // The specks (no depth write) are veiled by both media of their pass; the opaque sphere and the screen pass are not.
    assert!(veil(6).contains("loft_tr0(") && veil(6).contains("loft_tr1("), "{}", veil(6));
    assert!(!veil(0).contains("loft_tr"), "{}", veil(0));
    assert!(!veil(7).contains("loft_tr"), "{}", veil(7));
    let bytes: usize = l.modules.iter().map(String::len).sum();
    println!("{} modules, {bytes} bytes of WGSL; specks veil: {}", l.modules.len(), veil(6));
}

#[test]
fn a_medium_no_draw_finds_is_refused() {
    let (b, mut draws, media, passes) = scene();
    draws.retain(|d| !matches!(d.cover, Cover::Ray { find: Find::Medium(1), .. }));
    let passes: Vec<Pass> = passes.into_iter().map(|p| Pass { draws: [p.draws[0], p.draws[1].min(7)], ..p }).collect();
    let e = lower(&b.graph(), &b.slots(), &draws, &media, &passes).err().expect("refused");
    assert!(format!("{e:?}").contains("medium 1 is found by no draw"), "{e:?}");
}

/// Ground and ink are composed once (plan §8.2 lane 3.1): outside the composer no template or Rust file names them.
#[test]
fn ground_and_outline_logic_appear_once() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/");
    let files = [
        "src/draw/mod.rs",
        "wgsl/view.wgsl",
        "wgsl/ray.wgsl",
        "wgsl/hit.wgsl",
        "wgsl/volume.wgsl",
        "wgsl/grid.wgsl",
        "wgsl/mesh.wgsl",
        "wgsl/quads.wgsl",
        "wgsl/lit.wgsl",
        "wgsl/screen.wgsl",
        "wgsl/march.wgsl",
        "wgsl/gauss.wgsl",
        "wgsl/outline.wgsl",
    ];
    let mut found: Vec<(&str, &str)> = vec![];
    for f in files {
        let text = std::fs::read_to_string(format!("{root}{f}")).unwrap();
        for word in ["ground", "outline", "ink"] {
            // Code only: comments may say what the composer does.
            let code = text.lines().map(|l| l.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n");
            if code.contains(word) {
                found.push((f, word));
            }
        }
    }
    found.sort();
    found.dedup();
    let mut want = vec![
        ("src/draw/mod.rs", "ground"),
        ("src/draw/mod.rs", "outline"),
        ("wgsl/outline.wgsl", "ink"),
        ("wgsl/outline.wgsl", "outline"),
    ];
    want.sort();
    assert_eq!(found, want);
}

/// The GPU half of "grains dim inside a draw.volume" (run by `veil_gpu.ts`): 60 white specks on a tilted ring inside a
/// marched ball of density, as one view pass with the medium's draw first. Writes the specks' module to `LR31_DUMP`.
#[test]
fn dump_veiled_specks() {
    let Ok(path) = std::env::var("LR31_DUMP") else { return };
    let mut b = B::default();
    let (p, n, id) = (b.arg(Ty::V3), b.arg(Ty::V3), b.arg(Ty::F32));
    let white = b.lit(&[1.0, 1.0, 1.0, 1.0], Ty::V4);
    let (zero, one) = (b.lit(&[0.0], Ty::F32), b.lit(&[1.0], Ty::F32));
    let z0 = b.p(Prim::Mul, Ty::F32, &[id, zero]);
    let nn = b.p(Prim::Dot, Ty::F32, &[p, n]);
    let nn = b.p(Prim::Mul, Ty::F32, &[nn, zero]);
    let white = b.p(Prim::Add, Ty::V4, &[white, z0]);
    let white = b.p(Prim::Add, Ty::V4, &[white, nn]);
    let paint = b.func(&[p, n, id], white);
    // centre (cos a, 1 + sin a, 0.5 sin 3a), a = 0.10472 i; radius 0.03.
    let id = b.arg(Ty::F32);
    let step = b.lit(&[0.10472], Ty::F32);
    let a = b.p(Prim::Mul, Ty::F32, &[id, step]);
    let (x, s) = (b.p(Prim::Cos, Ty::F32, &[a]), b.p(Prim::Sin, Ty::F32, &[a]));
    let y = b.p(Prim::Add, Ty::F32, &[one, s]);
    let three = b.lit(&[3.0], Ty::F32);
    let a3 = b.p(Prim::Mul, Ty::F32, &[a, three]);
    let s3 = b.p(Prim::Sin, Ty::F32, &[a3]);
    let half = b.lit(&[0.5], Ty::F32);
    let z = b.p(Prim::Mul, Ty::F32, &[s3, half]);
    let r = b.lit(&[0.03], Ty::F32);
    let q = b.node(Op::Vec, Ty::V4, &[x, y, z, r]);
    let place = b.func(&[id], q);
    // density 1.5 (1.2 - |p - (0, 1, 0)|), clamped at 0 by the template.
    let p = b.arg(Ty::V3);
    let c = b.lit(&[0.0, 1.0, 0.0], Ty::V3);
    let d = b.p(Prim::Distance, Ty::F32, &[p, c]);
    let k = b.lit(&[1.2], Ty::F32);
    let d = b.p(Prim::Sub, Ty::F32, &[k, d]);
    let g = b.lit(&[1.5], Ty::F32);
    let d = b.p(Prim::Mul, Ty::F32, &[d, g]);
    let density = b.func(&[p], d);
    let name = b.sym("specks");
    let box2 = Proxy::Box { min: bx(-2.0, 2.0)[0], max: bx(-2.0, 2.0)[1] };
    let draws = vec![
        Draw {
            name,
            layer: Layer::Surroundings,
            cover: Cover::Ray { proxy: box2, find: Find::Medium(0) },
            paint,
            raster: GLOW,
        },
        Draw {
            name,
            layer: Layer::Subject,
            cover: Cover::Raster { prim: P::Quads { place }, count: 60 },
            paint,
            raster: GLOW,
        },
    ];
    let media = vec![Medium::March { density, within: bx(-2.0, 2.0), steps: 48, shadow_steps: 0 }];
    let passes =
        vec![Pass { target: Target::View, camera: 0, draws: [0, 2], clear: Clear::Load, ground: None, outline: None }];
    let l = lower(&b.graph(), &b.slots(), &draws, &media, &passes).unwrap();
    std::fs::write(path, &l.modules[l.draws[1].shade.0 as usize]).unwrap();
}
