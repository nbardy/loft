//! Draws, media and passes (lean Rust plan §1.5, §1.6, lane 3.1): each draw becomes one WGSL module, its graph
//! functions (emit, lane 2.2) followed by fixed templates (`crates/loft-core/wgsl/`). Rust knows one `Ray` template,
//! one vertex stage per raster primitive, `Screen`, and a medium's two integrations; what differs between the old
//! tracers, particle drawables and media is the library's graph functions (content), never a branch here.
//!
//! Graph function signatures, by role (emit's export wrappers take and return `lf`):
//! - `paint (p: vec3, n: vec3, id: f32) -> vec4`: rgba, alpha below 0.5 asking the composer for ink. For a medium, p
//!   is a sample, n the sun direction and the result the light scattered there per unit extinction. For `Screen`, p is
//!   (uv, alpha under) and n the colour under.
//! - `hit (o: vec3, d: vec3, span: vec2, width: vec2) -> vec4`: (normal, t), t < 0 a miss; the pixel's width at t is
//!   `width.x + width.y * t` (TS `Ray.width`).
//! - grid `place (uv: vec2, id: f32) -> vec3`; quads `place (id: f32) -> vec4` (centre, radius).
//! - medium `density (p: vec3) -> f32`; ground `hit` and `paint` as a draw's; `outline (colour: vec3, q: vec3) -> vec4`
//!   (ink rgb, width in world units).
//!
//! Composition, decided once here: a draw that writes no depth (glow, specks, rain) is invisible to a medium's
//! scene-depth clip, so its colour is multiplied by every medium of its passes (`veil`); one that writes depth is
//! veiled by the medium drawn over it. Ground and ink exist only in `compose`.
use crate::emit::{self, Row, Slots, Stage};
use loft_ir::bundle::{self as b, Clear, F32, Layer, Proxy, Raster, Target};
use loft_ir::error::SceneFault;
use loft_ir::{Error, graph::Graph};

const VIEW: &str = include_str!("../../wgsl/view.wgsl");
const RAY: &str = include_str!("../../wgsl/ray.wgsl");
const HIT: &str = include_str!("../../wgsl/hit.wgsl");
const VOLUME: &str = include_str!("../../wgsl/volume.wgsl");
const GRID: &str = include_str!("../../wgsl/grid.wgsl");
const MESH: &str = include_str!("../../wgsl/mesh.wgsl");
const QUADS: &str = include_str!("../../wgsl/quads.wgsl");
const LIT: &str = include_str!("../../wgsl/lit.wgsl");
const SCREEN: &str = include_str!("../../wgsl/screen.wgsl");
const MARCH: &str = include_str!("../../wgsl/march.wgsl");
const GAUSS: &str = include_str!("../../wgsl/gauss.wgsl");
const OUTLINE: &str = include_str!("../../wgsl/outline.wgsl");

/// A draw as the compiler holds it: bundle `Draw` with its graph functions (`Graph::fns` ids) still attached. Resource
/// ids (`Mesh.res`, `Proxy::Instances.res`, `Medium::Gaussians.res`) are `Graph::kernels` ids, mapped through
/// `Slots::kernels` to a pack and an offset (layout).
pub struct Draw {
    pub name: u32,
    pub layer: Layer,
    pub cover: Cover,
    pub paint: u32,
    pub raster: Raster,
}
pub enum Cover {
    Raster { prim: Prim, count: u32 },
    Ray { proxy: Proxy, find: Find },
    Screen,
}
pub enum Prim {
    Grid { u: u32, v: u32, place: u32 },
    Mesh { res: u32 },
    Quads { place: u32 },
}
pub enum Find {
    Hit(u32),
    Medium(u32),
}
/// One medium, two ways to integrate along a ray (plan §1.6). Albedo, phase and light are its draw's paint.
pub enum Medium {
    March { density: u32, within: [[F32; 3]; 2], steps: u16, shadow_steps: u16 },
    Gaussians { res: u32 },
}
pub struct Pass {
    pub target: Target,
    pub camera: u32,
    pub draws: [u32; 2],
    pub clear: Clear,
    /// The ground's (hit, paint), traced over the whole view.
    pub ground: Option<[u32; 2]>,
    pub outline: Option<u32>,
}

/// The bundle's draw-side lists. Module and name ids index `modules` and `names` (WGSL text with the `lf` alias).
#[derive(Default)]
pub struct Lowered {
    pub modules: Vec<String>,
    pub names: Vec<Box<str>>,
    pub draws: Vec<b::Draw>,
    pub media: Vec<b::Medium>,
    pub passes: Vec<b::Pass>,
}

/// Lowers every draw, medium and pass. `at` is layout's answer (`emit::Slots`, as emit takes it). Every module is checked by
/// Naga (f32 alias) before it is returned.
pub fn lower(g: &Graph, at: &Slots, draws: &[Draw], media: &[Medium], passes: &[Pass]) -> Result<Lowered, Error> {
    let mut l = Lowered::default();
    let mut tr = vec![None; media.len()];
    for (i, d) in draws.iter().enumerate() {
        let veils: Vec<u32> = match d.raster.depth_write {
            true => vec![],
            false => in_view(draws, passes, i as u32),
        };
        let (glue, cover, raster) = cover(at, d, media, &veils)?;
        let module = l.module(g, at, &glue)?;
        if let Cover::Ray { find: Find::Medium(k), .. } = d.cover {
            tr[k as usize].get_or_insert((module, l.name(&format!("loft_tr{k}"))));
        }
        let (name, shade) = (l.name(g.sym(d.name)), (module, l.name("fs")));
        l.draws.push(b::Draw { name, layer: d.layer, cover, shade, raster });
    }
    for (k, (m, t)) in media.iter().zip(tr).enumerate() {
        let transmittance =
            t.ok_or_else(|| unsupported(&format!("medium {k} is found by no draw: draw it or drop it")))?;
        let integrate = match *m {
            Medium::March { steps, shadow_steps, .. } => b::Integrate::March { steps, shadow_steps },
            Medium::Gaussians { res } => b::Integrate::Gaussians { res: at.kernels[res as usize].res },
        };
        l.media.push(b::Medium { transmittance, integrate });
    }
    for p in passes {
        let compose = l.compose(g, at, p)?;
        #[rustfmt::skip]
        l.passes.push(b::Pass { target: p.target.clone(), camera: p.camera, draws: p.draws, clear: p.clear.clone(), compose });
    }
    Ok(l)
}

/// The media found by draws of the colour passes that draw `i`: what a draw writing no depth is veiled by.
fn in_view(draws: &[Draw], passes: &[Pass], i: u32) -> Vec<u32> {
    let mut out = vec![];
    for p in passes.iter().filter(|p| (p.draws[0]..p.draws[1]).contains(&i)) {
        let colour = match p.target {
            Target::View | Target::Texture(_) => true,
            Target::Shadow { .. } => false,
        };
        for d in &draws[p.draws[0] as usize..p.draws[1] as usize] {
            if let (true, Cover::Ray { find: Find::Medium(k), .. }) = (colour, &d.cover) {
                out.extend((!out.contains(k)).then_some(*k));
            }
        }
    }
    out
}

/// What one module is made of: the graph functions it exports, its glue and its templates.
#[derive(Default)]
struct Glue {
    exports: Vec<(String, u32)>,
    /// Row readers already written (by bundle resource).
    readers: Vec<u32>,
    text: String,
    templates: Vec<&'static str>,
}

impl Glue {
    fn export(&mut self, name: &str, decl: u32) {
        self.exports.push((name.into(), decl));
    }
    /// The draw's paint and its f32 face (`paint(p, n, id: u32)`), which every colour template calls.
    fn paint(&mut self, decl: u32) {
        self.export("loft_paint", decl);
        self.line("fn paint(p: vec3<f32>, n: vec3<f32>, id: u32) -> vec4<f32> { return vec4<f32>(loft_paint(vec3<lf>(p), vec3<lf>(n), lf(f32(id)))); }");
    }
    /// A row resource's reader, `loft_rows{res}(k) -> vec4<f32>` over its `loft_rows{res}_n` vec4s, clamped (plan §4.3),
    /// from its pack; written once per module. Returns the reader's name.
    fn rows(&mut self, r: Row) -> String {
        let name = format!("loft_rows{}", r.res);
        if !self.readers.contains(&r.res) {
            self.readers.push(r.res);
            let n = u64::from(r.rows) * u64::from(r.lanes);
            self.line(format!(
                "const {name}_n = {n}u;\nfn {name}(k: u32) -> vec4<f32> {{ return loft_pack{}[{}u + min(k, {}u)]; }}",
                r.pack,
                r.offset,
                n.max(1) - 1
            ));
        }
        name
    }
    fn line(&mut self, s: impl AsRef<str>) {
        self.text.push_str(s.as_ref());
        self.text.push('\n');
    }
    /// The bounds of instance i of a Ray cover.
    fn proxy(&mut self, at: &Slots, p: &Proxy) {
        match p {
            Proxy::Box { min, max } => self.line(format!(
                "fn proxy(i: u32) -> array<vec3<f32>, 2> {{ return array({}, {}); }}",
                v3(min),
                v3(max)
            )),
            Proxy::Instances { res, local } => {
                let g = self.rows(at.kernels[*res as usize]);
                self.line(format!(
                    "fn proxy(i: u32) -> array<vec3<f32>, 2> {{ let r = {g}(i); return array(r.xyz + {} * r.w, r.xyz + {} * r.w); }}",
                    v3(&local[0]),
                    v3(&local[1])
                ));
            }
        }
    }
    /// Medium k's functions (`loft_tr{k}`, `loft_scatter{k}`).
    fn medium(&mut self, at: &Slots, k: u32, m: &Medium) {
        let text = match m {
            Medium::March { density, within, steps, shadow_steps } => {
                self.export(&format!("loft_d{k}"), *density);
                self.line(format!(
                    "const MEDIUM{k}_LO = {};\nconst MEDIUM{k}_HI = {};",
                    v3(&within[0]),
                    v3(&within[1])
                ));
                self.line(format!("const MEDIUM{k}_STEPS = {steps}u;\nconst MEDIUM{k}_SHADOW = {shadow_steps}u;"));
                MARCH.replace("{K}", &k.to_string())
            }
            Medium::Gaussians { res } => {
                let rows = self.rows(at.kernels[*res as usize]);
                GAUSS.replace("{K}", &k.to_string()).replace("{COUNT}", &format!("{rows}_n")).replace("{ROWS}", &rows)
            }
        };
        self.line(text);
    }
}

fn v3(v: &[F32; 3]) -> String {
    let f = |x: F32| format!("{:?}", f32::from_bits(x.0));
    format!("vec3<f32>({}, {}, {})", f(v[0]), f(v[1]), f(v[2]))
}

fn unsupported(what: &str) -> Error {
    Error::Scene { site: None, fault: SceneFault::Unsupported { what: what.into() } }
}

/// The one dispatcher over covers: a draw's glue and templates, its bundle cover and its raster state.
fn cover(at: &Slots, d: &Draw, media: &[Medium], veils: &[u32]) -> Result<(Glue, b::Cover, Raster), Error> {
    let mut gl = Glue::default();
    gl.paint(d.paint);
    let mut need = veils.to_vec();
    let (out, raster) = match &d.cover {
        Cover::Raster { prim, count } => {
            let prim = match *prim {
                Prim::Grid { u, v, place } => {
                    gl.export("loft_place", place);
                    gl.line(format!("const GRID_U = {u}u;\nconst GRID_V = {v}u;"));
                    gl.templates.push(GRID);
                    b::RasterPrim::Grid { u, v }
                }
                Prim::Mesh { res } => {
                    let r = gl.rows(at.kernels[res as usize]);
                    gl.line(format!("fn mesh_row(k: u32) -> vec4<f32> {{ return {r}(k); }}"));
                    gl.templates.push(MESH);
                    b::RasterPrim::Mesh { res: at.kernels[res as usize].res }
                }
                Prim::Quads { place } => {
                    gl.export("loft_place", place);
                    gl.templates.push(QUADS);
                    b::RasterPrim::Quads
                }
            };
            gl.templates.push(LIT);
            (b::Cover::Raster { prim, count: *count }, d.raster)
        }
        Cover::Ray { proxy, find } => {
            gl.proxy(at, proxy);
            gl.templates.push(RAY);
            let find = match *find {
                Find::Hit(f) => {
                    gl.export("loft_hit", f);
                    gl.templates.push(HIT);
                    b::Find::Hit
                }
                Find::Medium(k) => {
                    need.extend((!need.contains(&k)).then_some(k));
                    gl.line(format!(
                        "fn scatter(r: Ray, s: vec2<f32>) -> vec4<f32> {{ return loft_scatter{k}(r, s); }}"
                    ));
                    gl.templates.push(VOLUME);
                    b::Find::Medium(k)
                }
            };
            // The proxy box is wound inward, so only culling back faces runs each pixel once (outside or inside the
            // box). A draw that asked for no culling (a medium drawn like glow) would otherwise composite twice, once per face.
            (b::Cover::Ray { proxy: bundle_proxy(at, proxy), find }, Raster { cull: true, ..d.raster })
        }
        Cover::Screen => {
            gl.templates.push(SCREEN);
            (b::Cover::Screen, d.raster)
        }
    };
    for &k in &need {
        let m = media.get(k as usize).ok_or_else(|| unsupported(&format!("medium {k} is not declared")))?;
        gl.medium(at, k, m);
    }
    let reach = veils.iter().map(|k| format!(" * loft_tr{k}(p, back, reach, MEDIUM_VEIL)")).collect::<String>();
    gl.line(format!(
        "const MEDIUM_VEIL = 32u;\nfn veil(p: vec3<f32>, back: vec3<f32>, reach: f32) -> f32 {{ return 1.0{reach}; }}"
    ));
    Ok((gl, out, raster))
}

fn bundle_proxy(at: &Slots, p: &Proxy) -> Proxy {
    match p {
        Proxy::Box { .. } => p.clone(),
        Proxy::Instances { res, local } => Proxy::Instances { res: at.kernels[*res as usize].res, local: *local },
    }
}

impl Lowered {
    fn name(&mut self, s: &str) -> u32 {
        let at = self.names.iter().position(|n| &**n == s).unwrap_or_else(|| {
            self.names.push(s.into());
            self.names.len() - 1
        });
        at as u32
    }

    /// Emits the graph functions, appends the prelude, glue and templates, and checks the whole module with Naga.
    fn module(&mut self, g: &Graph, at: &Slots, gl: &Glue) -> Result<u32, Error> {
        let exports: Vec<(&str, u32)> = gl.exports.iter().map(|(n, d)| (n.as_str(), *d)).collect();
        // Naga at emit first, so a refusal in a graph function or raw source names the author's line; `check` below
        // then covers the glue and templates.
        let m = emit::module(g, &exports, at, Stage::Render)?;
        emit::check(g, &m)?;
        let mut text = m.text;
        for s in [VIEW].into_iter().chain(std::iter::once(gl.text.as_str())) {
            text.push('\n');
            text.push_str(s);
        }
        for t in &gl.templates {
            text.push('\n');
            text.push_str(t);
        }
        check(&text, false)?;
        self.modules.push(text);
        Ok((self.modules.len() - 1) as u32)
    }

    /// The pass composer: the ground, traced over the whole view, and the ink outline over the pass's colour and depth.
    /// The only place either exists.
    fn compose(&mut self, g: &Graph, at: &Slots, p: &Pass) -> Result<b::Compose, Error> {
        let entry = |l: &mut Lowered, gl: Option<Glue>| -> Result<Option<(u32, u32)>, Error> {
            let Some(gl) = gl else { return Ok(None) };
            let m = l.module(g, at, &gl)?;
            Ok(Some((m, l.name("fs"))))
        };
        let ground = p.ground.map(|[hit, paint]| {
            let mut gl = Glue::default();
            gl.paint(paint);
            gl.export("loft_hit", hit);
            gl.line("fn proxy(i: u32) -> array<vec3<f32>, 2> { return array(vec3<f32>(-3.4e38), vec3<f32>(3.4e38)); }");
            gl.line("fn veil(p: vec3<f32>, back: vec3<f32>, reach: f32) -> f32 { return 1.0; }");
            gl.line("@vertex fn vs(@builtin(vertex_index) k: u32) -> Out { return screen_out(k); }");
            gl.templates.push(HIT);
            gl
        });
        let outline = p.outline.map(|f| {
            let mut gl = Glue::default();
            gl.export("loft_outline", f);
            gl.templates.push(OUTLINE);
            gl
        });
        Ok(b::Compose { ground: entry(self, ground)?, outline: entry(self, outline)? })
    }
}

/// Naga's parse and validation of a whole draw module under one `lf` alias, with no GPU. The graph functions were
/// validated by emit; a refusal here is the glue's or a template's, or an author name colliding with a template's.
pub fn check(text: &str, f16: bool) -> Result<(), Error> {
    let prelude = if f16 { "enable f16;\nalias lf = f16;\n" } else { "alias lf = f32;\n" };
    let src = format!("{prelude}{text}");
    let refuse = |line: Option<u32>, msg: String| Error::Scene {
        site: None,
        fault: SceneFault::Wgsl {
            line: line.unwrap_or(0).saturating_sub(prelude.lines().count() as u32),
            msg: msg.into(),
        },
    };
    let m = naga::front::wgsl::parse_str(&src)
        .map_err(|e| refuse(e.location(&src).map(|l| l.line_number), e.emit_to_string(&src)))?;
    let caps = naga::valid::Capabilities::default() | naga::valid::Capabilities::SHADER_FLOAT16;
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), caps)
        .validate(&m)
        .map(|_| ())
        .map_err(|e| refuse(e.location(&src).map(|l| l.line_number), e.emit_to_string(&src)))
}
