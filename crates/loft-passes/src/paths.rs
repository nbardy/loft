//! Shared world lowering for the authored path schedules.
use super::{Emitter, LutRef, fn_symbol, reachable};
use crate::{Prog, program::{Draw, KindTable, abi}};
use loft_ir::{Code, Diag, Res, Span};
use loft_parse::ast::{QueryKind, Ty};
#[path="path_twin.rs"]
pub mod cpu;
use std::{collections::BTreeSet, fmt::Write};

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub enum Integrator {
    Reference,
    Spectral,
}

#[derive(Clone, Copy)]
pub(crate) enum Entry<'a> {
    Legacy(&'a str),
    Primary { transport: Primary<'a>, jitter: &'a str },
}

#[derive(Clone, Copy)]
pub(crate) enum Primary<'a> {
    Physical(&'a str),
    Style(super::Style<'a>),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct WorldResources {
    pub kinds: Vec<u32>,
    pub media: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PathTrace {
    pub integrator: Integrator,
    pub bounces: u32,
    #[serde(default)]
    pub lens: [f32; 2],
    pub lamps: Vec<[[f32; 4]; 2]>,
    #[serde(default)]
    pub worlds: Vec<WorldResources>,
    #[serde(default)]
    pub media: Vec<[[f32; 4]; 5]>,
    #[serde(default)]
    pub area_kind: Option<u32>,
}

const WORLD: &str = include_str!("world_path.wgsl");

/// Keep the per-kind material's staged layout private, as in the tiled path.
fn namespace(text: &str, kind: usize) -> String {
    let names = text.lines().filter_map(|line| {
        let word = ["struct ", "fn ", "const "].iter().find_map(|prefix| line.strip_prefix(prefix))?;
        word.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).next()
    }).collect::<BTreeSet<_>>();
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

pub(super) const QUERIES_END: &str = "// --- generated entry points ---";

fn row_arena_source(text: &str) -> String {
    text.lines().filter(|line| !line.starts_with("@group(1)") && !line.starts_with("struct KindParams"))
        .collect::<Vec<_>>().join("\n")
        .replace("let o = i * STRIDE;", "let o = i;")
        .replace("rows[o +", "world_words[o +")
        .replace("luts[", "world_luts[")
}

pub(crate) fn module(p: &Prog, types: &[Ty], kinds: &[KindTable], queries: &[(String, String)], luts: &[LutRef], path: &PathTrace, entry: Entry<'_>) -> Res<String> {
    let mut em = Emitter::new(p, types);
    em.header(abi::NONCE_PLACEHOLDER);
    let prefix_len = em.out.len();
    em.out.push_str(crate::bvh::WGSL);
    let key = match entry {
        Entry::Legacy(key) | Entry::Primary { transport: Primary::Physical(key), .. } => key,
        Entry::Primary { transport: Primary::Style(style), .. } => style.estimate,
    };
    let f = p.fns[key];
    let mut world = WORLD.to_owned();
    let mut helper = vec![p.linked(f.name.span, "sky_dome")?];
    for name in ["reservoir_add", "reservoir_scale", "reservoir_reuse"] {
        let key = p.linked(f.name.span, name)?;
        helper.push(key);
        world = world.replace(&fn_symbol(name), &fn_symbol(key));
    }
    let area = path.area_kind.unwrap_or(u32::MAX);
    let _ = writeln!(em.out, "const LOFT_AREA_KIND: u32 = {area}u;");
    if let Some(kind) = path.area_kind {
        let decl = p.kinds[kinds[kind as usize].name.as_str()];
        let sample = p.linked(decl.name.span, "sample_sphere")?;
        helper.push(sample);
        world.push_str(&include_str!("world_area.wgsl").replace("loft_sample_sphere", &fn_symbol(sample)));
    } else {
        world.push_str("\nfn world_sphere_light(id: u32, point: vec3f, xi: vec2f) -> PathLight { return PathLight(vec3f(0.0), vec3f(0.0), 0.0); }\n");
    }
    em.out.push_str(&world);
    em.out.push_str(crate::bsdf::VOLUME_STEP_WGSL);
    // The authored medium arithmetic uses the arena accessors in WORLD.
    let (_, medium) = include_str!("world_medium.wgsl").split_once("struct WorldSigma")
        .expect("world medium math");
    em.out.push_str("struct WorldSigma");
    em.out.push_str(medium);
    // The path template uses the checked ABI payloads from lighting/world.
    let world_payload = |name| p.structs.get(name).ok_or_else(|| Diag::new(Code::E011, f.name.span,
        format!("missing path payload `{name}`"), "import stdlib::lighting::world"));
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
        em.out.push_str(r"
fn loft_medium(world: u32, p: vec3<f32>) -> loft_type_Sigma {
  let sigma = world_medium(world, p);
  return loft_type_Sigma(sigma.s, sigma.t, sigma.g, sigma.emission);
}
fn loft_authored_volume(world: u32, ray: loft_type_Ray, limit: f32, steps: u32, rng: array<u32, 4>) -> loft_type_Vol {
  let vol = world_volume(world, LoftRay(ray.o, ray.d, ray.t0, min(ray.t1, limit)), steps, true);
  return loft_type_Vol(vol.l, vol.t);
}
");
    }
    let [radius, focus] = path.lens;
    let _ = writeln!(em.out, "const LENS_RADIUS: f32 = {}; const FOCUS: f32 = {};", super::f32_lit(f64::from(radius)), super::f32_lit(f64::from(focus)));
    let spectral = matches!(path.integrator, Integrator::Spectral);
    let mut local_cases = Vec::new();
    let mut field_cases = Vec::new();
    let mut eval_cases = Vec::new();
    let mut intersect_cases = Vec::new();
    let mut optics_cases = Vec::new();
    for (i, (kind, (_, whole))) in kinds.iter().zip(queries).enumerate() {
        let decl = p.kinds[kind.name.as_str()];
        if matches!(kind.draw, Draw::Surface { .. } | Draw::Mesh { .. }) {
            return Err(Diag::new(Code::E405, decl.name.span, "path schedules require trace geometry for every drawable",
                "supply a trace kind or use the raster schedule for surfaces and mesh assets"));
        }
        let (head, _) = whole.split_once(QUERIES_END).expect("kind query section");
        let body = row_arena_source(&head[prefix_len..]);
        em.out.push_str(&namespace(&body, i));
        em.out.push('\n');
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
            Draw::Traced => "return loft_bvh_march(leaf, LoftRay(ray.o, ray.d, span.x, span.y), world_words[b + 6u]);".into(),
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
            if let Some(slot) = layout.lobes.iter().position(|(name, _)| *name == "spectral_dielectric") {
                optics_cases.push(format!(r"
    case {i}u: {{
      let row = kind{i}_load_row(h.at);
      let xf = loft_xf_inv(kind{i}_loft_pose(row));
      let b = kind{i}_q_material(row, loft_xf_pt(xf, h.p), loft_xf_vec(xf, select(-h.n, h.n, h.entering)){uv});
      return Optics(b.l{slot}_0, b.l{slot}_1, b.l{slot}_2);
    }}"));
            }
        }
    }
    let _ = writeln!(em.out, r"
fn world_local(kind: u32, at: u32, point: vec3<f32>) -> vec3<f32> {{
  switch kind {{ {} default: {{ return point; }} }}
}}
fn world_field(kind: u32, at: u32, point: vec3<f32>) -> f32 {{
  switch kind {{ {} default: {{ return 1e6; }} }}
}}
fn loft_bvh_intersect(leaf: LoftLeafRef, ray: LoftRay) -> f32 {{
  let at = world_at(leaf);
  let b = at + bitcast<u32>(world_words[2u + 2u*leaf.kind_id]) - 7u;
  let lo = vec3(world_words[b],world_words[b+1u],world_words[b+2u]);
  let hi = vec3(world_words[b+3u],world_words[b+4u],world_words[b+5u]);
  let span = loft_bvh_span(ray, lo, hi);
  if span.x > span.y {{ return -1.0; }}
  switch leaf.kind_id {{ {} default: {{ return -2.0; }} }}
}}
fn world_eval(h: PathHit, wo: vec3<f32>, wi: vec3<f32>) -> vec3<f32> {{
  switch h.kind {{ {} default: {{ return vec3<f32>(0.0); }} }}
}}", local_cases.join("\n"), field_cases.join("\n"), intersect_cases.join("\n"), eval_cases.join("\n"));
    if spectral {
        let _ = writeln!(em.out, r"
struct Optics {{ base: f32, dispersion: f32, absorb: vec3<f32> }}
fn world_optics(h: PathHit) -> Optics {{
  switch h.kind {{ {} default: {{ return Optics(0.0, 0.0, vec3<f32>(0.0)); }} }}
}}", optics_cases.join("\n"));
    }
    if let Entry::Primary { transport, jitter } = entry {
        helper.push(jitter);
        if let Primary::Style(style) = transport {
            style.types(&mut em);
            helper.push(style.shade);
        }
        helper.push(key);
        let (fns, consts) = reachable(p, &[], &helper);
        em.luts = luts;
        em.library(&fns, &consts);
        primary(&mut em.out, transport, jitter, helper[0], path.bounces);
        return Ok(em.out);
    }
    let convert = if spectral {
        let mut text = include_str!("spectral_path.wgsl").to_owned();
        for name in ["spectral_path", "spectral_transition", "spectral_ior", "spectral_band", "spectral_xyz"] {
            let key = p.linked(f.name.span, name)?;
            helper.push(key);
            text = text.replace(&fn_symbol(name), &fn_symbol(key));
        }
        em.out.push_str(&text);
        emit_deposit(&mut em, p.linked(f.name.span, "deposit_caustic")?)?;
        let display = p.linked(f.name.span, "agx_display")?;
        helper.push(display);
        format!("{}(sum / f32(samples))", fn_symbol(display))
    } else {
        "path_aces(sum / f32(samples))".to_owned()
    };
    let (fns, consts) = reachable(p, &[f], &helper);
    em.luts = luts;
    em.library(&fns, &consts);
    em.func(f, &fn_symbol(key));
    let bounces = path.bounces;
    let _ = writeln!(em.out, r"
@vertex fn vs_present(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {{
  let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
  return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}}
@fragment fn fs_present(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {{
  if frame.cold.x == LOFT_NONCE {{ discard; }}
  let samples = max(frame.cold.w, 1u);
  var sum = vec3<f32>(0.0);
  for (var i = 0u; i < samples; i++) {{
    let seed = path_hash(u32(pos.x) + u32(pos.y)*frame.size.x) ^ path_hash(i);
    let jitter = vec2<f32>(path_random(seed, 0xfffffff1u), path_random(seed, 0xfffffff2u)) - 0.5;
    let primary = path_camera(pos.xy + jitter, seed);
    sum += {}(primary.o, primary.d, seed, {bounces}u);
  }}
  return vec4<f32>({convert}, 1.0);
}}
", fn_symbol(key));
    Ok(em.out)
}

fn primary(out: &mut String, transport: Primary<'_>, jitter: &str, sky: &str, bounces: u32) {
    let (groups, estimate, resolve) = match transport {
        Primary::Physical(sample) => (String::new(), format!(
            "sum += {}(ray.o, ray.d, seed, {bounces}u);", fn_symbol(sample)), String::new()),
        Primary::Style(super::Style { hit, summary, estimate, shade }) => (
            format!(r"
struct InkGroup {{
  hit: loft_type_{hit},
  light: loft_type_{summary},
  count: u32,
}}
"),
            format!(r"
      if h.t < 0.0 {{
        if h.t == -1.0 {{ sum += {}(ray.d); }}
        continue;
      }}
      let wo = -ray.d;
      let hit = loft_type_{hit}(h.p, world_local(h.kind, h.at, h.p), h.n, wo, vec2f(0.0),
        world_eval(h, wo, h.n) * 3.14159265, world_eval(h, wo, vec3f(0.0)));
      let dx = length(loft_ray(pixel + vec2f(1.0, 0.0)) - ray.d);
      let dy = length(loft_ray(pixel + vec2f(0.0, 1.0)) - ray.d);
      let footprint = h.t * max(dx, dy) / max(abs(dot(h.n, ray.d)), 0.1);
      let sun = normalize(frame.sun.xyz);
      let visibility = world_visibility(0u, LoftRay(h.p + h.n * 0.0002, sun, 0.0001, 1e6));
      let light = loft_type_{summary}(sun, dot(visibility, vec3f(0.2126, 0.7152, 0.0722)),
        loft_params[0].xyz, vec3f(0.0), 1.0, footprint, h.t, packed);
      let estimate = {}(hit, light, array<u32, 4>(px.x, px.y, seed, {bounces}u));
      var g = 0u;
      while g < group_count {{
        if bins[g].light.id == packed {{ break; }}
        g++;
      }}
      if g == group_count {{
        bins[g] = InkGroup(hit, estimate, 1u);
        group_count++;
      }} else {{
        bins[g].light.key_vis += estimate.key_vis;
        bins[g].light.indirect_irradiance += estimate.indirect_irradiance;
        bins[g].count++;
      }}
", fn_symbol(sky), fn_symbol(estimate)),
            format!(r"
  for (var g = 0u; g < group_count; g++) {{
    let count = f32(bins[g].count);
    var light = bins[g].light;
    light.key_vis /= count;
    light.indirect_irradiance /= count;
    sum += {}(bins[g].hit, light) * count;
  }}
", fn_symbol(shade)),
        ),
    };
    let bins = if groups.is_empty() { "" } else { "var bins: array<InkGroup, 4>; var group_count = 0u;" };
    let _ = writeln!(out, r"
{groups}
struct PathPrimaryOut {{
  @location(0) vis: vec2u,
  @location(1) hdr: vec4f,
  @builtin(frag_depth) depth: f32,
}}
@vertex fn vs_primary(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4f {{
  let uv = vec2f(f32((vi << 1u) & 2u), f32(vi & 2u));
  return vec4f(uv * 2.0 - 1.0, 0.0, 1.0);
}}
@fragment fn fs_path_primary(@builtin(position) pos: vec4f) -> PathPrimaryOut {{
  if frame.cold.x == LOFT_NONCE {{ discard; }}
  let px = vec2u(pos.xy);
  let samples = max(frame.cold.w, 1u);
  let sites = min(samples, 4u);
  let key = path_hash(px.x + px.y * frame.size.x);
  var sum = vec3f(0.0);
  var nearest = 1e30;
  var depth = 1.0;
  var id = LOFT_EMPTY;
  {bins}
  for (var site = 0u; site < sites; site++) {{
    let pixel = pos.xy + {}(key, site);
    let ray = path_camera(pixel, key ^ path_hash(site));
    let h = world_trace(ray.o, ray.d, 1e6);
    let packed = (h.kind << 24u) | h.row;
    if h.t >= 0.0 && h.t < nearest {{
      nearest = h.t;
      id = packed;
      let clip = frame.view_proj * vec4f(h.p, 1.0);
      depth = clamp(clip.z / clip.w, 0.0, 1.0);
    }}
    // Reuse the primary site as spp increases. The sample index changes
    // the transport dimensions, while at most four row identities need bins.
    for (var i = site; i < samples; i += sites) {{
      let seed = key ^ path_hash(i);
      {estimate}
    }}
  }}
  {resolve}
  return PathPrimaryOut(vec2u(id, bitcast<u32>(nearest)), vec4f(sum / f32(samples), 1.0), depth);
}}
", fn_symbol(jitter));
}

fn emit_deposit(em: &mut Emitter<'_, '_>, key: &str) -> Res<()> {
    let kernel = em.p.kernels.get(key).ok_or_else(|| Diag::new(Code::E011, Span::NONE,
        "missing caustic photon deposit kernel", "load the spectral_caustic engine"))?;
    let Some(table) = kernel.f.params.first() else { return Err(Diag::new(Code::E003, kernel.f.name.span, "deposit needs a table", "use deposit_caustic(bins, bin, flux)")); };
    if kernel.writes.get(&table.name.name) != Some(&loft_parse::ast::Determinism::Det) || kernel.writes.len() != 1 || table.name.name != "bins" {
        return Err(Diag::new(Code::E504, kernel.f.name.span, "caustic deposit requires one deterministic table named bins", "use integer atomic accumulation"));
    }
    let mut f = kernel.f.clone();
    f.params.remove(0);
    em.func(&f, "deposit_caustic");
    Ok(())
}
