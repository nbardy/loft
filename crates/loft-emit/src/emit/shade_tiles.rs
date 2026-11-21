//! Lower the shade callback against the existing raster and World contracts.
use super::{Aov, Emitter, LutRef, SHADE_ENTRY, TWIN_BINDINGS, fn_symbol, paths, reachable};
use loft_check::{Prog, program::{KindTable, abi}};
use loft_ir::{Res, Span};
use loft_parse::ast::Ty;
use std::{collections::BTreeSet, fmt::Write};

#[derive(Clone, Copy)]
pub enum Visibility {
    Center,
    SubpixelEdges,
}

#[derive(Clone, Copy)]
pub enum Lighting<'a> {
    Bsdf(&'a str),
    World { callback: &'a str, visibility: Visibility },
}

// The tiled and screen integrators share WGSL queries. The tiled bindings use
// the existing read-only LUT table for the World rows, roots, nodes and leaves.
pub fn world_source(
    p: &Prog, types: &[Ty], kinds: &[KindTable], queries: &[(String, String)], span: Span,
) -> Res<String> {
    let mut em = Emitter::new(p, types);
    em.header(abi::NONCE_PLACEHOLDER);
    let header = em.out.len();
    paths::emit_world(&mut em, kinds, queries, false, paths::query_source(), span)?;
    let source = em.out[header..].lines().filter(|line| !line.starts_with("@group(2)"))
        .collect::<Vec<_>>().join("\n")
        .replace("world_luts[", "luts[")
        .replace("world_words[", "luts[bitcast<u32>(luts[0]) + ")
        .replace("loft_bvh_nodes[root_id]", "loft_tiled_node(root_id)")
        .replace("loft_bvh_nodes[stack[depth]]", "loft_tiled_node(stack[depth])")
        .replace("loft_bvh_nodes[first]", "loft_tiled_node(first)")
        .replace("loft_bvh_nodes[first + 1u]", "loft_tiled_node(first + 1u)")
        .replace("loft_bvh_leaves[first + j]", "loft_tiled_leaf(first + j)")
        .replace("world_lamps[id]", "loft_tiled_lamp(id)")
        .replace("arrayLength(&world_lamps)", "bitcast<u32>(luts[4])");
    Ok(format!(r"
const LENS_RADIUS = 0.0;
const FOCUS = 1.0;
fn loft_tiled_node(id: u32) -> LoftBvhNode {{
  let i = bitcast<u32>(luts[1]) + 8u * id;
  return LoftBvhNode(vec3f(luts[i], luts[i+1u], luts[i+2u]), bitcast<u32>(luts[i+3u]),
    vec3f(luts[i+4u], luts[i+5u], luts[i+6u]), bitcast<u32>(luts[i+7u]));
}}
fn loft_tiled_leaf(id: u32) -> LoftLeafRef {{
  let i = bitcast<u32>(luts[2]) + 2u * id;
  return LoftLeafRef(bitcast<u32>(luts[i]), bitcast<u32>(luts[i+1u]));
}}
fn loft_tiled_lamp(id: u32) -> PathLamp {{
  let i = bitcast<u32>(luts[3]) + 8u * id;
  return PathLamp(vec4f(luts[i], luts[i+1u], luts[i+2u], luts[i+3u]),
    vec4f(luts[i+4u], luts[i+5u], luts[i+6u], luts[i+7u]));
}}
{source}
"))
}

pub fn module(
    p: &Prog, types: &[Ty], luts: &[LutRef], original: &str, world: &str, callback: &str, sky: &str,
    visibility: Visibility, primary_world: u32,
) -> String {
    let (head, shade) = original.split_once(SHADE_ENTRY).expect("kind shade entry");
    let (_, twin) = shade.split_once(TWIN_BINDINGS).expect("kind twin entry");
    let (functions, constants) = reachable(p, &[], &[sky, callback]);
    let symbols = head.lines().filter_map(|line| {
        let word = ["fn ", "const "].iter().find_map(|prefix| line.strip_prefix(prefix))?;
        word.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).next()
    }).collect::<BTreeSet<_>>();
    let functions = functions.into_iter().filter(|f| !symbols.contains(fn_symbol(&f.name.name).as_str())).collect::<Vec<_>>();
    let constants = constants.into_iter().filter(|name| !symbols.contains(format!("loft_const_{name}").as_str())).collect();
    let mut em = Emitter::new(p, types);
    em.luts = luts;
    em.library(&functions, &constants);
    let callback = fn_symbol(callback);
    let sky = fn_symbol(sky);
    let mut out = format!(r"{head}{world}
{}
fn loft_eval_hit(hit: loft_type_Hit, wo: vec3f, wi: vec3f) -> vec3f {{
  let leaf = LoftLeafRef(hit.kind_id, hit.row_id);
  let n = hit.n;
  return world_eval(PathHit(hit.t, hit.p, n, hit.kind_id, world_at(leaf), true, hit.row_id), wo, wi);
}}
fn loft_shade_hit(leaf: LoftLeafRef, ray: LoftRay, t: f32, wo: vec3f, px: vec2f) -> vec3f {{
  let at = world_at(leaf);
  let p = ray.o + ray.d * t;
  var g = vec3f(0.0);
  // Keep both taps of each axis together for the primary surface normal.
  for (var axis = 0u; axis < 3u; axis++) {{
    var e = vec3f(0.0);
    e[axis] = 0.0001;
    g[axis] = world_field(leaf.kind_id, at, p + e) - world_field(leaf.kind_id, at, p - e);
  }}
  if !(dot(g, g) > 0.0) {{ return vec3f(0.0); }}
  let n = normalize(select(-g, g, dot(g, ray.d) < 0.0));
  let hit = loft_type_Hit(t, p, n, vec2f(0.0), world_local(leaf.kind_id, at, p),
    leaf.kind_id, leaf.row_id, vec4f(0.0));
  return {callback}(0u, hit, wo, false, px);
}}
", em.out);
    let shade = match visibility {
        Visibility::Center => r"
  if v.x == LOFT_EMPTY || (v.x >> 24u) != kp.kind_id { return; }
  let color = loft_shade_hit(LoftLeafRef(kp.kind_id, v.x & 0xffffffu), center, bitcast<f32>(v.y), -center.d, vec2f(px));
",
        Visibility::SubpixelEdges => {
            out.push_str(NEIGHBORS);
            let _ = writeln!(out, "const LOFT_PRIMARY_WORLD = {primary_world}u;");
            let _ = writeln!(out, "fn loft_edge_sky(rd: vec3f) -> vec3f {{ return {sky}(rd, normalize(frame.sun.xyz)); }}");
            out.push_str(include_str!("shade_edges.wgsl"));
            r"
  if v.x != LOFT_EMPTY && (v.x >> 24u) != kp.kind_id { return; }
  let near = loft_vis_neighborhood(px, v.x);
  if near.x == LOFT_EMPTY || (near.x >> 24u) != kp.kind_id { return; }
  let color = loft_shade_samples(center, vec2f(px), v, near.y != 0u);
"
        }
    };
    let _ = write!(out, r"
@compute @workgroup_size(8, 8) fn shade(@builtin(workgroup_id) wg: vec3u, @builtin(local_invocation_id) lid: vec3u) {{
  if frame.cold.x == LOFT_NONCE {{ return; }}
  let tile = tiles[kp.kind_id * frame.size.w + wg.x];
  let px = vec2u(tile & 0xffffu, tile >> 16u) * 8u + lid.xy;
  if any(px >= frame.size.xy) {{ return; }}
  let v = textureLoad(vis_tex, px, 0).xy;
  let center = LoftRay(frame.eye.xyz, loft_ray(vec2f(px) + 0.5), 0.0001, 1e6);
{shade}  textureStore(hdr, px, vec4f(color, 1.0));
}}
{TWIN_BINDINGS}{twin}");
    out
}

const NEIGHBORS: &str = r"
// A halo pixel belongs to the first covered neighbor. Exactly one kind writes
// any pixel, including where different kinds meet or a tile crosses the edge.
fn loft_vis_neighborhood(px: vec2u, center: u32) -> vec2u {
  let limit = vec2i(frame.size.xy) - 1;
  var owner = center;
  var edge = 0u;
  for (var y = -1; y <= 1; y++) {
    for (var x = -1; x <= 1; x++) {
      let key = textureLoad(vis_tex, clamp(vec2i(px) + vec2i(x, y), vec2i(0), limit), 0).x;
      if key != center { edge = 1u; }
      if owner == LOFT_EMPTY { owner = key; }
    }
  }
  return vec2u(owner, edge);
}
";

pub fn classify_module(visibility: Visibility) -> String {
    let source = super::classify_module();
    match visibility {
        Visibility::Center => source,
        Visibility::SubpixelEdges => source.replace(
            "if v != LOFT_EMPTY { atomicOr(&mask, 1u << (v >> 24u)); }",
            r"if v != LOFT_EMPTY { atomicOr(&mask, 1u << (v >> 24u)); }
    // Only border lanes read the halo; each corner also reads its diagonal.
    let local = g.xy & vec2u(7);
    let step = vec2i(select(select(0, 1, local.x == 7u), -1, local.x == 0u),
                     select(select(0, 1, local.y == 7u), -1, local.y == 0u));
    for (var i = 1u; i <= 3u; i++) {
      let delta = step * vec2i(i32(i & 1u), i32(i >> 1u));
      let q = vec2i(g.xy) + delta;
      if any(delta != vec2i(0)) && all(q >= vec2i(0)) && all(q < vec2i(frame.size.xy)) {
        let neighbor = textureLoad(vis_tex, q, 0).x;
        if neighbor != LOFT_EMPTY { atomicOr(&mask, 1u << (neighbor >> 24u)); }
      }
    }",
        ),
    }
}

pub fn present_module(
    p: &Prog, types: &[Ty], sky: &str, aov: Aov, paper: Option<&str>, film: Option<&str>, visibility: Visibility,
) -> String {
    let mut source = super::emit_present(p, types, sky, aov, paper, film);
    match visibility {
        Visibility::Center => source,
        Visibility::SubpixelEdges => {
            source = source.replace("textureLoad(vis_tex, px, 0).x == LOFT_EMPTY",
                "textureLoad(vis_tex, px, 0).x == LOFT_EMPTY && loft_vis_neighborhood(px, LOFT_EMPTY).x == LOFT_EMPTY");
            source.push_str(NEIGHBORS);
            source
        }
    }
}
