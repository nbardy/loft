//! Shared world lowering for the authored path schedules.
use super::{Emitter, KindModule, LUTS, LutRef, fn_symbol, reachable, render_template};
use loft_check::{Prog, program::{Draw, KindTable, abi}};
pub use loft_check::paths::{Integrator, PathTrace, WorldResources, cpu};
use loft_ir::{Code, Diag, Res, Span};
use loft_parse::ast::{FnDecl, QueryKind, Ty};
use std::{collections::BTreeSet, fmt::Write};

pub(super) enum WorldStorage {
    Screen,
    Tiled,
}

pub(super) fn declarations(text: &str) -> BTreeSet<&str> {
    text.lines().filter_map(|line| {
        let word = ["struct ", "fn ", "const "].iter().find_map(|prefix| line.strip_prefix(prefix))?;
        word.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).next()
    }).collect()
}

/// Each generated symbol is private to one kind's material layout and query set.
fn namespace(text: &str, kind: usize) -> String {
    let names = declarations(text);
    let mut out = String::with_capacity(text.len());
    for token in text.split_inclusive(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
        let name = token.trim_end_matches(|c: char| !(c.is_ascii_alphanumeric() || c == '_'));
        if names.contains(name) {
            let _ = write!(out, "kind{kind}_");
        }
        out.push_str(token);
    }
    out
}

pub(super) fn emit_world(
    em: &mut Emitter<'_, '_>, kinds: &[KindTable], queries: &[KindModule], spectral: bool,
    storage: WorldStorage, span: Span,
) -> Res<()> {
    let p = em.p;
    let (buffer, base) = match storage {
        WorldStorage::Screen => {
            em.out.push_str(crate::BVH_WGSL);
            em.out.push_str(include_str!("../../../../stdlib/render/world_storage.wgsl"));
            em.out.push_str(LUTS);
            ("world_words", "i")
        }
        WorldStorage::Tiled => {
            em.out.push_str(crate::BVH_TRAVERSAL);
            em.out.push_str(include_str!("../../../../stdlib/render/tiled_world.wgsl"));
            ("luts", "bitcast<u32>(luts[0]) + i")
        }
    };
    em.out.push_str(include_str!("../../../../stdlib/render/world_queries.wgsl"));
    em.out.push_str(loft_check::bsdf::VOLUME_STEP_WGSL);
    em.out.push_str(include_str!("../../../../stdlib/render/world_medium.wgsl"));
    // The path template uses the checked ABI payloads from lighting/world.
    let world_payload = |name| p.structs.get(name).ok_or_else(|| Diag::new(Code::E011, span,
        format!("missing World payload `{name}`"), "import stdlib::lighting::world"));
    for name in ["Ray", "Hit", "Sigma", "Vol"] {
        let mut fields = Vec::new();
        for field in world_payload(name)?.1 {
            fields.push(format!("  {}: {},", super::member_name(&field.name.name), super::wty(&em.resolve(&field.ty))));
        }
        let _ = writeln!(em.out, "struct loft_type_{name} {{\n{}\n}}", fields.join("\n"));
    }
    let volume = world_payload("Vol")?;
    let radiance = &volume.1.first().ok_or_else(|| Diag::new(Code::E003, volume.0.span,
        "Vol has no radiance field", "use the Vol payload from stdlib::lighting::world"))?.ty;
    if em.resolve(radiance) == Ty::VecN(3) {
        em.out.push_str(include_str!("../../../../stdlib/render/world_rgb_medium.wgsl"));
    }
    let mut local_cases = Vec::new();
    let mut field_cases = Vec::new();
    let mut eval_cases = Vec::new();
    let mut intersect_cases = Vec::new();
    let mut optics_cases = Vec::new();
    for (i, (kind, module)) in kinds.iter().zip(queries).enumerate() {
        let decl = p.kinds[kind.name.as_str()];
        if matches!(kind.draw, Draw::Surface { .. } | Draw::Mesh { .. }) {
            return Err(Diag::new(Code::E405, decl.name.span, "path schedules require trace geometry for every drawable",
                "supply a trace kind or use the raster schedule for surfaces and mesh assets"));
        }
        let mut query = em.kind_row(decl, buffer, base);
        query.push_str(&module.queries);
        em.out.push_str(&namespace(&query, i));
        em.sampled_bakes.extend(module.sampled_bakes.iter().cloned());
        local_cases.push(format!(r"
    case {i}u: {{
      let row = kind{i}_load_row(at);
      return loft_xf_pt(loft_xf_inv(kind{i}_loft_pose(row)), point);
    }}"));
        field_cases.push(format!(r"
    case {i}u: {{
      let row = kind{i}_load_row(at);
      let pose = kind{i}_loft_pose(row);
      return kind{i}_q_trace(row, loft_xf_pt(loft_xf_inv(pose), point));
    }}"));
        let intersection = match kind.draw {
            Draw::Analytic => format!(r"
                let row = kind{i}_load_row(at);
                let xf = loft_xf_inv(kind{i}_loft_pose(row));
                let t = kind{i}_q_visible(row, loft_xf_pt(xf, ray.o + ray.t0 * ray.d), loft_xf_vec(xf, ray.d));
                let hit = ray.t0 + t;
                return select(-1.0, hit, t >= 0.0 && hit >= span.x && hit <= span.y);"),
            Draw::Traced => "return loft_bvh_march(leaf, LoftRay(ray.o, ray.d, span.x, span.y), world_word(b + 6u));".into(),
            Draw::Surface { .. } | Draw::Mesh { .. } => unreachable!("raster-only geometry refused above"),
        };
        intersect_cases.push(format!("case {i}u: {{ {intersection} }}"));
        let mat = decl.query(QueryKind::Material).expect("draw material");
        let uv = if mat.params.len() == 4 { ", vec2<f32>(0.0)" } else { "" };
        eval_cases.push(format!(r"
    case {i}u: {{
      let row = kind{i}_load_row(h.at);
      let xf = loft_xf_inv(kind{i}_loft_pose(row));
      let b = kind{i}_q_material(row, loft_xf_pt(xf, h.p), loft_xf_vec(xf, select(-h.n, h.n, h.entering)){uv});
      return kind{i}_eval_bsdf(b, h.n, wo, wi);
    }}"));
        if spectral {
            let layout = super::material_layout(p, decl)?;
            let field = |j, zero| layout.first_field(|i, name| {
                has_dispersion_fields(em, p.fns[name]).then(|| format!("b.l{i}_{j}"))
            }, zero);
            if let Some(base) = field(0, "0.0") {
                let dispersion = field(1, "0.0").expect("spectral lobe");
                let absorb = field(2, "vec3<f32>(0.0)").expect("spectral lobe");
                optics_cases.push(format!(r"
    case {i}u: {{
      let row = kind{i}_load_row(h.at);
      let xf = loft_xf_inv(kind{i}_loft_pose(row));
      let b = kind{i}_q_material(row, loft_xf_pt(xf, h.p), loft_xf_vec(xf, select(-h.n, h.n, h.entering)){uv});
      return Optics({base}, {dispersion}, {absorb});
    }}"));
            }
        }
    }
    em.out += &render_template(include_str!("../../../../stdlib/render/world_dispatch.wgsl"), &[
        ("LOCAL", &local_cases.join("\n")), ("FIELD", &field_cases.join("\n")),
        ("INTERSECT", &intersect_cases.join("\n")), ("EVAL", &eval_cases.join("\n")),
    ]);
    if spectral {
        em.out += &render_template(include_str!("../../../../stdlib/render/world_optics.wgsl"), &[
            ("OPTICS", &optics_cases.join("\n")),
        ]);
    }
    Ok(())
}

// Optical payloads expose the three roles through their bound lobe parameters.
fn has_dispersion_fields(em: &Emitter<'_, '_>, lobe: &FnDecl) -> bool {
    let [ior, dispersion, absorption, _, _, _] = lobe.params.as_slice() else { return false };
    [ior, dispersion, absorption].into_iter()
        .zip([("ior", Ty::F32), ("dispersion", Ty::F32), ("absorption", Ty::VecN(3))])
        .all(|(param, (role, ty))| param.name.name == role && param.ty.as_ref().is_some_and(|t| em.resolve(t) == ty))
}

/// Resolve a template's function slots in the callback's module scope.
fn linked_template<'a>(
    p: &Prog<'a>, span: Span, template: &str, slots: &[(&str, &str)], helpers: &mut Vec<&'a str>,
) -> Res<String> {
    let mut values = Vec::with_capacity(slots.len());
    for &(slot, name) in slots {
        let key = p.linked(span, name)?;
        helpers.push(key);
        values.push((slot, fn_symbol(key)));
    }
    let values = values.iter().map(|(slot, symbol)| (*slot, symbol.as_str())).collect::<Vec<_>>();
    Ok(render_template(template, &values))
}

pub fn module(
    p: &Prog, types: &[Ty], kinds: &[KindTable], queries: &[KindModule], luts: &[LutRef],
    fixed: &std::collections::BTreeMap<(String, String), String>, path: &PathTrace, entry: &str,
) -> Res<String> {
    let f = p.fns.get(entry).ok_or_else(|| Diag::new(Code::E011, Span::NONE, format!("missing path function {entry}"), "load the selected engine recipe"))?;
    let span = f.name.span;
    let mut helper = vec![p.linked(span, "sky_dome")?];
    let mut em = Emitter::new(p, types);
    em.fixed = fixed;
    em.header(abi::NONCE_PLACEHOLDER);
    let spectral = matches!(path.integrator, Integrator::Spectral);
    emit_world(&mut em, kinds, queries, spectral, WorldStorage::Screen, span)?;
    em.out += &linked_template(p, span, include_str!("../../../../stdlib/render/world_path.wgsl"), &[
        ("RESERVOIR_ADD", "reservoir_add"),
        ("RESERVOIR_SCALE", "reservoir_scale"),
        ("RESERVOIR_REUSE", "reservoir_reuse"),
    ], &mut helper)?;
    let [radius, focus] = path.lens;
    let _ = writeln!(em.out, "const LENS_RADIUS: f32 = {}; const FOCUS: f32 = {};", super::f32_lit(f64::from(radius)), super::f32_lit(f64::from(focus)));
    let convert = if spectral {
        for name in ["spectral_path", "spectral_xyz"] {
            helper.push(p.linked(span, name)?);
        }
        em.out += &linked_template(p, span, include_str!("../../../../stdlib/render/spectral_path.wgsl"), &[
            ("SPECTRAL_TRANSITION", "spectral_transition"),
            ("SPECTRAL_IOR", "spectral_ior"),
            ("SPECTRAL_BAND", "spectral_band"),
        ], &mut helper)?;
        emit_deposit(&mut em, p.linked(span, "deposit_caustic")?)?;
        let display = p.linked(f.name.span, "agx_display")?;
        helper.push(display);
        format!("{}(sum / f32(samples))", fn_symbol(display))
    } else {
        "path_aces(sum / f32(samples))".to_owned()
    };

    let (fns, consts) = reachable(p, &[f], &helper);
    em.luts = luts;
    em.library(&fns, &consts);
    em.func(f, &fn_symbol(entry));
    em.sampled_textures();
    let bounces = path.bounces;
    em.out += &render_template(include_str!("../../../../stdlib/render/screen_pass.wgsl"), &[
        ("ENTRY", &fn_symbol(entry)), ("BOUNCES", &bounces.to_string()), ("CONVERT", &convert),
    ]);
    Ok(em.out)
}

fn emit_deposit(em: &mut Emitter<'_, '_>, key: &str) -> Res<()> {
    let kernel = em.p.kernels.get(key).ok_or_else(|| Diag::new(Code::E011, Span::NONE,
        "missing deterministic deposit kernel", "load the kernel used by the path recipe"))?;
    let Some(table) = kernel.f.params.first() else { return Err(Diag::new(Code::E003, kernel.f.name.span, "deposit needs a table", "use deposit_caustic(bins, bin, flux)")); };
    if kernel.writes.get(&table.name.name) != Some(&loft_parse::ast::Determinism::Det) || kernel.writes.len() != 1 || table.name.name != "bins" {
        return Err(Diag::new(Code::E504, kernel.f.name.span, "caustic deposit requires one deterministic table named bins", "use integer atomic accumulation"));
    }
    let mut f = kernel.f.clone();
    f.params.remove(0);
    em.func(&f, "deposit_caustic");
    Ok(())
}
