//! Layer 2 of the meta engine (plan 2026-11-20 §1): every pass from source to `Program`, one module per pass.
//!
//!   parse -> index -> check -> lower the schedule -> stage 0 (rows) -> derive (bounds, L) -> emit -> naga validate
//!
//! `compile` runs them all; `Compiled` keeps what the gates need afterwards (the CPU twin of every query).

mod cache;
mod authored_compute;
mod frame_pass;
mod motion;
mod pipeline;
mod provenance;
mod shade_schedule;
mod shader;
pub use loft_check::{Prog, bsdf, check, kind_space, pose_field, source, world, TWIN_LIGHT};
pub use loft_eval::{ad, eval, lip};
pub use loft_gltf as gltf;
use loft_check::{FINITE_DIFF_STEP, infer, unit};
use loft_eval::{as_f32, pack};

pub mod emit {
    pub use loft_emit::emit::*;
    pub use crate::motion::motion_module;
}
pub use loft_parse as parse;

use eval::{Eval, RowVal, Tables, Val};
use loft_parse::ast::*;
use crate::program::{Draw, Program};
use loft_ir::{Code, Diag, Res, Span};
use source::Graph;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

/// Parse every file, then generate the derivative fns `diff` asked for (one more module at the end).
pub fn parse_all(src: &Graph) -> Res<Vec<Module>> {
    let (mut modules, mut next) = src.parsed()?;
    // Certify the normalized function and the actual resolved call closure. A
    // comment or a different function in the same file has no bearing on it.
    let certified = {
        let p = Prog::index(&modules)?;
        let mut certified = HashSet::new();
        for (name, hash) in [
            ("box_sdf", "103768bf7d29ffc6232109015b9f5dc3419951aeba89bac263e4b8bbf2c22035"),
            ("capsule", "d5969ede3cb6b897e19919fe25668130e063e5649a1287d6e2480e5e0113e261"),
            ("round_box", "f34d6300722e78c429d9810a41bb4b0f5c4ded1b2e05f5fbfab8acc926fa05c9"),
            ("smin", "90337285c51e676a04494e72cd5c0e7249e39f241ac12d77d4efa91c433117ca"),
            ("sphere", "f8de200152a8ad78550c80e959ea5d93e4bd9e0e4921063373e07fbaf5368add"),
            ("ssub", "0c3786ccda1d02f6cd30c6fca3cfb2645161b4365f2208a09f953edf00650697"),
        ] {
            if let Some(f) = p.fns.get(name) && cache::certificate(&p, f)? == hash {
                certified.insert(f.name.span);
            }
        }
        certified
    };
    for item in modules.iter_mut().flat_map(|m| &mut m.items) {
        let Item::Fn(f) = item else { continue };
        let sp = f.name.span;
        if !certified.contains(&sp) { continue; }
        let make = |name: &str, next: &mut u32| {
            let id = *next;
            *next += 1;
            Expr { id, span: sp, kind: ExprKind::Name(name.into()) }
        };
        let ax = match f.name.name.as_str() {
            "sphere" | "round_box" | "box_sdf" => make("v", &mut next),
            "capsule" => make("p", &mut next),
            "smin" | "ssub" => {
                let args = ["a", "b"].map(|n| Arg { name: None, value: make(n, &mut next) }).to_vec();
                let id = next;
                next += 1;
                Expr { id, span: sp, kind: ExprKind::Call { callee: Ident { name: "max".into(), span: sp }, args } }
            }
            _ => unreachable!("known certified function"),
        };
        f.attrs.push(Attr { name: Ident { name: "lipschitz".into(), span: sp }, args: vec![ax] });
    }

    infer::run(&mut modules)?;
    ad::run(&mut modules, &mut next)?;
    Ok(modules)
}

/// The GPU passes in dependency order; `find_shadow_map` is optional, between the vis bundle and classification.
pub const GPU_ORDER: [&str; 6] = ["cull_indirect", "render_bundle", "find_shadow_map", "tile_classify", "shade_tiles", "tonemap_aces"];

/// Front end only: parse, index, check. For `loft check` and the error tests.
pub fn front(src: &Graph) -> Res<(Vec<Module>, Vec<Ty>)> {
    let mut modules = parse_all(src)?;
    let mut types = vec![];
    check_modules(&mut modules, &mut types)?;
    loft_eval::refine::check(&Prog::index(&modules)?, &types, &[])?;
    Ok((modules, types))
}

fn check_modules(modules: &mut [Module], types: &mut Vec<Ty>) -> Res<Option<Span>> {
    let mut effects = check::check(&Prog::index(modules)?, types)?;
    for item in modules.iter_mut().flat_map(|m| &mut m.items) {
        if let Item::Kernel(k) = item {
            k.writes = effects.kernels.remove(&k.f.name.name).expect("checked kernel");
        }
    }
    Ok(effects.pending_lowering)
}

/// Per drawn kind: rows, distinct shapes (one bound derivation each) and the derived L range.
pub struct KindReport {
    pub name: String,
    pub proof: GeometryProof,
    pub rows: usize,
    pub shapes: usize,
    pub l_min: f64,
    pub l_max: f64,
    pub evals: u64,
    pub cache_hits: usize,
    pub draw: Draw,
}

pub struct Report {
    pub kinds: Vec<KindReport>,
    /// Wall milliseconds per pass, in order.
    pub ms: Vec<(&'static str, f64)>,
}

/// What survives compilation for the gates: the source, types, scans and stage-0 tables, so any query runs on the CPU.
pub struct Compiled {
    pub modules: Vec<Module>,
    pub types: Vec<Ty>,
    pub tables: Tables,
    pub luts: Vec<eval::Lut>,
    /// Drawn kinds by kind id: (stage-0 table index, name).
    pub drawn: Vec<(usize, String)>,
    pub meshes: Vec<gltf::Mesh>,
    pub compute: AuthoredCompute,
}

/// The standalone pass-derived representation of source-authored compute.
#[derive(Default)]
pub struct AuthoredCompute {
    standalone: bool,
    pub bundle: loft_ir::Bundle,
    pub tables: BTreeMap<String, loft_ir::TableId>,
    pub determinism: BTreeMap<loft_ir::TableId, Determinism>,
    pub truth: BTreeMap<String, loft_ir::Verdict>,
    pub tiers: BTreeMap<String, Determinism>,
    producers: BTreeMap<loft_ir::TableId, Span>,
}

/// One kind's queries on the CPU in f32 (the twin). Traced and analytic kinds take a world point and direction;
/// surface kinds take (u, v) in `p[0..2]`. `material` is the Bsdf seen along its normal, lit 60 degrees off it (in
/// the plane of the normal and `TWIN_LIGHT`), plus its emission.
pub struct CpuQuery {
    pub trace: f32,
    pub visible: f32,
    pub material: [f32; 3],
    pub surf_p: [f32; 3],
    pub surf_n: [f32; 3],
}

/// Rotate a vector by the rotational component of a rigid transform.
pub fn rot(r: &eval::Rigid, v: [f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|i| r.r[i][0] * v[0] + r.r[i][1] * v[1] + r.r[i][2] * v[2])
}
fn v3<S: eval::Scalar>(x: [f32; 3]) -> Val<S> {
    Val::V(x.iter().map(|c| S::c(f64::from(*c))).collect(), 0.0)
}
fn of_v3(v: Val<f32>) -> [f32; 3] {
    match v {
        Val::V(c, _) => [c[0], c[1], c[2]],
        other @ Val::Void | other @ Val::S(_) | other @ Val::B(_) | other @ Val::U(_) | other @ Val::I(_) | other @ Val::X(_) | other @ Val::Row(..) | other @ Val::MapRef(..) | other @ Val::Bsdf(_) => unreachable!("a vec3, got {other:?}"),
    }
}

fn reflect_params(prog: &Prog<'_>, types: &[Ty], packed: &[f32]) -> Res<Vec<loft_ir::ParamMeta>> {
    use loft_ir::{IrTy, ParamId, ParamMeta};
    let mut eval: Eval<f32> = Eval::new(prog, types, vec![], &[]);
    prog.continuous_params().enumerate().map(|(i, (name, decl))| {
        let range = decl.range.as_ref().map(|r| -> Res<_> {
            let lo = as_f32(eval::concrete(&eval.module_expr(&r.lo)?, r.lo.span)?, r.lo.span)?;
            let lo = if r.strict { lo.next_up() } else { lo };
            let hi = match &r.hi {
                Some(e) => as_f32(eval::concrete(&eval.module_expr(e)?, e.span)?, e.span)?,
                None => f32::MAX,
            };
            Ok(([lo, 0., 0., 0.], [hi, 0., 0., 0.]))
        }).transpose()?;
        let offset = (3 + i) * 4;
        Ok(ParamMeta {
            id: ParamId(i as u32), name: name.to_owned(), ty: IrTy::F32,
            offset_bytes: (offset * 4) as u32,
            default: [packed[offset], 0., 0., 0.],
            range,
        })
    }).collect()
}

impl Compiled {
    pub fn prog(&self) -> Prog<'_> {
        Prog::index(&self.modules).expect("indexed once already")
    }

    /// Reflected slots follow the same ordering and offsets used by WGSL lowering.
    pub fn param_meta(&self, program: &Program) -> Res<Vec<loft_ir::ParamMeta>> {
        reflect_params(&self.prog(), &self.types, &program.params)
    }

    pub fn mesh_triangles(&self) -> impl Iterator<Item = &[[[f32; 3]; 3]]> {
        self.meshes.iter().map(|mesh| mesh.triangles.as_slice())
    }

    /// The row's placement in World (identity for a kind asked in World).
    pub fn pose(&self, kind_id: usize, row: usize) -> eval::Rigid {
        let prog = self.prog();
        let (table, name) = &self.drawn[kind_id];
        match pose_field(prog.kinds[name.as_str()]) {
            Some(i) => match &self.tables[*table][row][i] {
                RowVal::X(x) => *x,
                other @ RowVal::F(_) | other @ RowVal::V(_) | other @ RowVal::U(_) | other @ RowVal::I(_) | other @ RowVal::B(_) => unreachable!("checked pose, got {other:?}"),
            },
            None => eval::Rigid::yaw([0.0; 3], 0.0),
        }
    }

    pub fn cpu_query(&self, kind_id: usize, row: usize, p: [f32; 3], rd: [f32; 3]) -> Res<CpuQuery> {
        let prog = self.prog();
        let (table, name) = &self.drawn[kind_id];
        let k = prog.kinds[name.as_str()];
        let mut ev: Eval<f32> = Eval::new(&prog, &self.types, self.tables.clone(), &[]);
        ev.luts = &self.luts;
        let pose = self.pose(kind_id, row);
        let to_local = pose.inv();
        let me = Val::Row(*table, row);
        let sp = Span::default();
        let f32_of = |v: Val<f32>| match v {
            Val::S(s) => s,
            other @ Val::Void | other @ Val::V(..) | other @ Val::B(_) | other @ Val::U(_) | other @ Val::I(_) | other @ Val::X(_) | other @ Val::Row(..) | other @ Val::MapRef(..) | other @ Val::Bsdf(_) => unreachable!("query returns f32, got {other:?}"),
        };
        let mut out = CpuQuery { trace: 0.0, visible: 0.0, material: [0.0; 3], surf_p: [0.0; 3], surf_n: [0.0; 3] };
        if name == "Mesh" && k.queries.is_empty() {
            let RowVal::U(asset) = self.tables[*table][row][1] else { unreachable!("checked asset") };
            let mesh = &self.meshes[asset as usize];
            let r = rot(&to_local, p);
            let origin = std::array::from_fn(|i| r[i] + to_local.t[i]);
            let n = unit(rd);
            out.visible = mesh.hit(origin, unit(rot(&to_local, rd)));
            let a = unit(TWIN_LIGHT);
            let t = unit([n[1] * a[2] - n[2] * a[1], n[2] * a[0] - n[0] * a[2], n[0] * a[1] - n[1] * a[0]]);
            let lt = std::array::from_fn(|i| n[i] * 0.5 + t[i] * 0.866_025_4);
            let b = Val::Bsdf(vec![eval::Layer::Lobe(prog.linked(k.name.span, "ggx")?.into(), vec![v3(mesh.color), Val::S(mesh.roughness), Val::S(mesh.metal)])]);
            out.material = of_v3(ev.eval_bsdf(&b, &v3(n), &v3(n), &v3(lt), sp)?);
            return Ok(out);
        }
        let (pl, nl, nw, uv) = match k.query(QueryKind::Surface) {
            Some(surf) => {
                let (u, v) = (Val::S(p[0]), Val::S(p[1]));
                let at = of_v3(ev.call_fn(surf, vec![me.clone(), u.clone(), v.clone()], sp)?);
                let (du, dv) = match k.proof {
                    GeometryProof::Required => {
                        let jvp = k.fns.iter().find(|f| f.name.name == "surface__jvp").expect("generated");
                        let du = of_v3(ev.call_fn(jvp, vec![me.clone(), u.clone(), v.clone(), Val::S(1.0), Val::S(0.0)], sp)?);
                        let dv = of_v3(ev.call_fn(jvp, vec![me.clone(), u, v, Val::S(0.0), Val::S(1.0)], sp)?);
                        (du, dv)
                    }
                    GeometryProof::Unproved(_) => {
                        let mut tap = |dx: f32, dy: f32| ev.call_fn(surf, vec![me.clone(),
                            Val::S(p[0] + dx * FINITE_DIFF_STEP), Val::S(p[1] + dy * FINITE_DIFF_STEP)], sp).map(of_v3);
                        let [a, b, c, d] = [tap(-1., -1.)?, tap(1., -1.)?, tap(-1., 1.)?, tap(1., 1.)?];
                        (std::array::from_fn(|i| (b[i] - a[i]) + (d[i] - c[i])),
                         std::array::from_fn(|i| (c[i] - a[i]) + (d[i] - b[i])))
                    }
                };
                let n = unit([du[1] * dv[2] - du[2] * dv[1], du[2] * dv[0] - du[0] * dv[2], du[0] * dv[1] - du[1] * dv[0]]);
                let pw = rot(&pose, at);
                out.surf_p = [0, 1, 2].map(|i| pw[i] + pose.t[i]);
                out.surf_n = rot(&pose, n);
                (at, n, out.surf_n, Some([p[0], p[1]]))
            }
            None => {
                let pl = { let r = rot(&to_local, p); [0, 1, 2].map(|i| r[i] + to_local.t[i]) };
                let dl = unit(rot(&to_local, rd));
                out.trace = f32_of(ev.call_fn(k.query(QueryKind::Trace).expect("checked"), vec![me.clone(), v3(pl)], sp)?);
                if let Some(f) = k.query(QueryKind::Visible) {
                    out.visible = f32_of(ev.call_fn(f, vec![me.clone(), v3(pl), v3(dl)], sp)?);
                }
                (pl, dl, unit(rd), None)
            }
        };
        let mut args = vec![me, v3(pl), v3(nl)];
        if let Some(uv) = uv {
            args.push(Val::V(vec![uv[0], uv[1]], 0.0));
        }
        let b = ev.call_fn(k.query(QueryKind::Material).expect("checked"), args, sp)?;
        let (a, k) = (unit(TWIN_LIGHT), nw);
        let t = unit([k[1] * a[2] - k[2] * a[1], k[2] * a[0] - k[0] * a[2], k[0] * a[1] - k[1] * a[0]]);
        let lt = [0, 1, 2].map(|i| k[i] * 0.5 + t[i] * 0.866_025_4);
        let lit = of_v3(ev.eval_bsdf(&b, &v3(nw), &v3(nw), &v3(lt), sp)?);
        let glow = of_v3(ev.eval_bsdf(&b, &v3(nw), &v3(nw), &v3([0.0; 3]), sp)?);
        out.material = [0, 1, 2].map(|i| lit[i] + glow[i]);
        Ok(out)
    }
}

fn arg<'e>(pd: &'e PassDecl, name: &str) -> Res<&'e Expr> {
    pd.args.iter().find(|a| a.name.as_ref().is_some_and(|n| n.name == name)).map(|a| &a.value).ok_or_else(|| {
        Diag::new(Code::E520, pd.verb.span, format!("`{}` needs `{name}: ...`", pd.verb.name), format!("add `{name}: ...`"))
    })
}

/// The kind names among a pass's positional arguments.
fn kind_args<'p>(p: &Prog<'p>, pd: &PassDecl) -> Res<Vec<&'p str>> {
    let mut kinds = vec![];
    for a in pd.args.iter().filter(|a| a.name.is_none()) {
        let ExprKind::Name(n) = &a.value.kind else {
            return Err(Diag::new(Code::E011, a.value.span, "expected kind name", "name a declared drawable kind"));
        };
        if p.schedule.is_some_and(|s| s.implicit) && matches!(n.as_str(), "Drawables" | "Casters") {
            kinds.extend(p.kind_order.iter().copied().filter(|k| {
                let kind = p.kinds[k];
                (!kind.queries.is_empty() || *k == "Mesh") && (n == "Drawables" || pose_field(kind).is_some())
            }));
        } else if let Some((&name, _)) = p.kinds.get_key_value(n.as_str()) {
            kinds.push(name);
        } else {
            return Err(Diag::new(Code::E011, a.value.span, format!("unknown kind {n}"), "name a declared drawable kind"));
        }
    }
    Ok(kinds)
}

pub fn fn_name_arg<'e>(p: &Prog, pd: &'e PassDecl, name: &str) -> Res<&'e str> {
    let e = arg(pd, name)?;
    match &e.kind {
        ExprKind::Name(n) if p.fns.contains_key(n.as_str()) => Ok(n),
        // Copying a stock schedule's `light: sun_sky` into a scene without importing it read as "names a library fn".
        ExprKind::Name(n) => Err(Diag::new(Code::E011, e.span, format!("`{n}` is not declared or imported in this file"),
            format!("import it from the catalog module that lists it, e.g. `use stdlib::integrators::direct::{{{n}}}`"))),
        ExprKind::Unproved { .. } | ExprKind::FiniteDiff { .. } | ExprKind::If { .. } | ExprKind::Num { .. } | ExprKind::Bool(_) | ExprKind::String(_) | ExprKind::Member(..) | ExprKind::Index(..) | ExprKind::Call { .. } | ExprKind::Ctor { .. } | ExprKind::Binary(..) | ExprKind::Unary(..) => Err(Diag::new(Code::E011, e.span, format!("`{name}:` names a library fn"), "write the fn's name, e.g. `sky: sky_color`")),
    }
}

fn require_schedule_fn(
    p: &Prog, types: &mut Vec<Ty>, name: &str, span: Span,
    params: &[Ty], output: Ty, help: &str,
) -> Res<()> {
    let f = p.fns[name];
    let checker = check::Ck::new(p, types);
    let actual = f.params.iter().map(|a| a.ty.as_ref().map(|t| checker.resolve(t, true)).transpose())
        .collect::<Res<Vec<_>>>()?;
    let ret = f.ret.as_ref().map(|t| checker.resolve(t, true)).transpose()?;
    if f.comptime || !actual.iter().map(Option::as_ref).eq(params.iter().map(Some)) || ret != Some(output) {
        return Err(Diag::new(Code::E003, span, format!("invalid schedule callback signature for {name}"), help));
    }
    Ok(())
}

fn as_v3(v: RowVal, span: Span) -> Res<[f32; 3]> {
    match v {
        RowVal::V(c) if c.len() == 3 => Ok([c[0], c[1], c[2]]),
        other @ RowVal::F(_) | other @ RowVal::V(_) | other @ RowVal::U(_) | other @ RowVal::I(_) | other @ RowVal::B(_) | other @ RowVal::X(_) => Err(Diag::new(Code::E003, span, format!("expected a vec3, found {other:?}"), "pass vec3(x, y, z) or Pt[S](...)")),
    }
}

/// Inputs shared by the scene and shader compilation stages.
#[derive(Clone, Copy)]
pub struct CompileOptions<'a> {
    pub overrides: &'a [(String, f64)],
    pub samples: &'a [program::MotionSample],
    pub cache: &'a Path,
    pub aov: emit::Aov,
    pub features: loft_ir::GpuFeatures,
}

impl Default for CompileOptions<'_> {
    fn default() -> Self {
        Self {
            overrides: &[],
            samples: &[],
            cache: Path::new("target/loft_cache"),
            aov: emit::Aov::Beauty,
            features: loft_ir::GpuFeatures::empty(),
        }
    }
}

impl<'a> CompileOptions<'a> {
    pub fn with_overrides(mut self, overrides: &'a [(String, f64)]) -> Self {
        self.overrides = overrides;
        self
    }

    pub fn with_samples(mut self, samples: &'a [program::MotionSample]) -> Self {
        self.samples = samples;
        self
    }

    pub fn with_cache(mut self, cache: &'a Path) -> Self {
        self.cache = cache;
        self
    }

    pub fn with_aov(mut self, aov: emit::Aov) -> Self {
        self.aov = aov;
        self
    }

    pub fn with_features(mut self, features: loft_ir::GpuFeatures) -> Self {
        self.features = features;
        self
    }
}

pub fn compile_with(src: &Graph, opts: &CompileOptions<'_>) -> Res<(Program, Report, Compiled)> {
    pipeline::compile(src, opts)
}

pub fn compile(src: &Graph, overrides: &[(String, f64)]) -> Res<(Program, Report, Compiled)> {
    compile_with(src, &CompileOptions::default().with_overrides(overrides))
}

pub fn compile_cached(src: &Graph, overrides: &[(String, f64)], cache: &std::path::Path) -> Res<(Program, Report, Compiled)> {
    compile_with(src, &CompileOptions::default().with_overrides(overrides).with_cache(cache))
}

/// Re-derive local bounds and pack the sampled pose before drawing a requested
/// still. The render path never culls an animated row against its time-zero box.
pub fn compile_snapshot(src: &Graph, overrides: &[(String, f64)], samples: &[program::MotionSample]) -> Res<(Program, Report, Compiled)> {
    compile_with(src, &CompileOptions::default().with_overrides(overrides).with_samples(samples))
}

pub fn compile_snapshot_aov(src: &Graph, overrides: &[(String, f64)], samples: &[crate::program::MotionSample], aov: emit::Aov) -> Res<(Program, Report, Compiled)> {
    compile_with(src, &CompileOptions::default().with_overrides(overrides).with_samples(samples).with_aov(aov))
}

pub fn compile_aov(src: &Graph, overrides: &[(String, f64)], aov: emit::Aov) -> Res<(Program, Report, Compiled)> {
    compile_with(src, &CompileOptions::default().with_overrides(overrides).with_aov(aov))
}

/// Link source-authored compute for the features enabled on the host's device.
pub fn compile_for_device(src: &Graph, overrides: &[(String, f64)], aov: emit::Aov, features: loft_ir::GpuFeatures) -> Res<(Program, Report, Compiled)> {
    compile_with(src, &CompileOptions::default().with_overrides(overrides).with_aov(aov).with_features(features))
}

pub mod program;
pub mod sexp;
pub mod telemetry;
pub mod fusion;
pub mod bvh;
pub mod quant;
pub mod bake_tex;
#[path = "bundle.rs"]
mod renderer_bundle;
pub mod bundle {
    use super::{Compiled, Program};
    use loft_ir::Bundle;

    pub use super::renderer_bundle::{Options, lower_program, lower_twin, parameters, replacement};

    pub fn lower(
        program: &Program, compiled: &Compiled, size: (u32, u32),
        frame: impl Fn(bool, Option<([f32; 3], [f32; 3], u32)>) -> Vec<u8>,
    ) -> Result<Bundle, String> {
        if compiled.compute.bundle.passes.is_empty() {
            return super::renderer_bundle::lower(program, compiled, size, frame);
        }
        if !compiled.compute.standalone || program.path.is_some() || !program.dispatches.is_empty() {
            return Err("the portable artifact needs a standalone authored compute schedule".into());
        }
        let mut bundle = compiled.compute.bundle.clone();
        bundle.params = parameters(program, compiled)?;
        super::renderer_bundle::compute_bakes(program, &compiled.compute.tables, &mut bundle)?;
        Ok(bundle)
    }
}
pub mod plan;
