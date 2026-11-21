//! Lower the shade callback against the existing raster and World contracts.
use super::{Aov, Emitter, LutRef, SHADE_ENTRY, TWIN_BINDINGS, fn_symbol, paths, reachable};
use crate::{Prog, bvh::Bvh2, eval::RowVal, fn_name_arg, program::{Instance, KindTable, abi},
    require_schedule_fn, stage0};
use loft_ir::{Code, Diag, Res, Span};
use loft_parse::ast::{Arg, Expr, ExprKind, Geo, PassDecl, Space, Ty};
use std::{collections::BTreeSet, fmt::Write};

#[derive(Clone, Copy)]
pub(crate) enum Visibility {
    Center,
    SubpixelEdges,
}

#[derive(Clone, Copy)]
pub(crate) enum Lighting<'a> {
    Bsdf(&'a str),
    World { callback: &'a str, visibility: Visibility },
}

pub(crate) fn lighting<'a>(
    p: &Prog, types: &mut Vec<Ty>, pass: &'a PassDecl, overrides: &[(String, f64)],
) -> Res<Lighting<'a>> {
    let mut names = BTreeSet::new();
    for arg in &pass.args {
        if let Some(name) = &arg.name
            && (!["light", "shade", "samples", "sun"].contains(&name.name.as_str())
                || !names.insert(name.name.as_str()))
        {
            return Err(Diag::new(Code::E003, name.span, format!("unknown or duplicate shade_tiles argument {}", name.name),
                "use one shade or light callback, a sun direction and optional samples"));
        }
    }
    let mut positional = pass.args.iter().filter(|arg| arg.name.is_none());
    if !matches!(positional.next(), Some(Arg { value: Expr { kind: ExprKind::Name(name), .. }, .. }) if name == "Vis64")
        || positional.next().is_some()
    {
        return Err(Diag::new(Code::E003, pass.verb.span, "shade_tiles needs one Vis64 input", "pass Vis64 first"));
    }
    let visibility = match pass.args.iter().find(|a| a.name.as_ref().is_some_and(|n| n.name == "samples")) {
        None => Visibility::Center,
        Some(a) => match stage0(p, types, &a.value, overrides)? {
            RowVal::U(1) | RowVal::I(1) => Visibility::Center,
            RowVal::U(4) | RowVal::I(4) => Visibility::SubpixelEdges,
            _ => return Err(Diag::new(Code::E602, a.value.span, "shade_tiles samples must be 1 or 4",
                "use samples: 4 for subpixel edge coverage")),
        },
    };
    match (names.contains("light"), names.contains("shade"), visibility) {
        (true, false, Visibility::Center) => Ok(Lighting::Bsdf(fn_name_arg(p, pass, "light")?)),
        (false, true, visibility) => {
            let callback = fn_name_arg(p, pass, "shade")?;
            let direction = Ty::Geo(Geo::Dir, Space::Named("World".into()));
            require_schedule_fn(p, types, callback, pass.verb.span,
                &[Ty::Struct("World".into()), Ty::Struct("Hit".into()), direction, Ty::Bool, Ty::VecN(2)], Ty::VecN(3),
                "write fn shade(w: World, h: Hit, wo: d3@World, ink: bool, px: v2) -> rgb")?;
            Ok(Lighting::World { callback, visibility })
        }
        _ => Err(Diag::new(Code::E003, pass.verb.span, "shade_tiles needs either light or shade",
            "use shade: callback for World queries and subpixel coverage")),
    }
}

// The tiled and screen integrators share WGSL queries. The tiled bindings use
// the existing read-only LUT table for the World rows, roots, nodes and leaves.
pub(crate) fn world_source(
    p: &Prog, types: &[Ty], kinds: &[KindTable], queries: &[(String, String)], instances: &[Instance],
    data: &paths::PathTrace, words: &mut Vec<f32>, span: Span,
) -> Res<String> {
    let overflow = || Diag::new(Code::E602, Span::NONE, "tiled World arena exceeds its address range", "use fewer geometry rows");
    let address = |len: usize| u32::try_from(len).map_err(|_| overflow());
    let base = address(words.len())?;
    words[0] = f32::from_bits(base);
    words.resize(words.len() + 1 + 2 * kinds.len(), 0.0);
    for (i, kind) in kinds.iter().enumerate() {
        words[base as usize + 1 + 2 * i] = f32::from_bits(address(words.len())? - base);
        words[base as usize + 2 + 2 * i] = f32::from_bits(kind.stride);
        words.extend_from_slice(&kind.rows);
    }
    words[base as usize] = f32::from_bits(address(words.len())? - base);
    let media_count = address(data.media.len())?;
    let mut descriptors = vec![address(data.worlds.len())?, media_count];
    let mut nodes = Vec::new();
    let mut leaves = Vec::new();
    for world in &data.worlds {
        let members = instances.iter().filter(|i| world.kinds.contains(&i.kind)).copied().collect::<Vec<_>>();
        let tree = Bvh2::build(&members).map_err(|error| Diag::new(
            Code::E602, Span::NONE, format!("invalid tiled World bounds: {error:?}"), "check the drawable bounds",
        ))?;
        let node_base = address(nodes.len() / 8)?;
        let leaf_base = address(leaves.len() / 2)?;
        let root = if tree.nodes().is_empty() { 0 } else { node_base.checked_add(1).ok_or_else(overflow)? };
        descriptors.extend([root, if world.media { media_count } else { 0 }]);
        for node in tree.nodes() {
            let offset = if node.count == 0 { node_base } else { leaf_base };
            nodes.extend(node.lo.map(f32::to_bits));
            nodes.push(node.left_or_first.checked_add(offset).ok_or_else(overflow)?);
            nodes.extend(node.hi.map(f32::to_bits));
            nodes.push(node.count);
        }
        leaves.extend(tree.leaves().iter().flat_map(|leaf| [leaf.kind_id, leaf.row_id]));
    }
    words.extend(descriptors.into_iter().map(f32::from_bits));
    words.extend_from_slice(data.media.as_flattened().as_flattened());
    words[1] = f32::from_bits(address(words.len())?);
    words.extend(nodes.into_iter().map(f32::from_bits));
    words[2] = f32::from_bits(address(words.len())?);
    words.extend(leaves.into_iter().map(f32::from_bits));
    words[3] = f32::from_bits(address(words.len())?);
    words[4] = f32::from_bits(address(data.lamps.len())?);
    words.extend_from_slice(data.lamps.as_flattened().as_flattened());
    // The native TLAS primitive order is the compiled instance order.
    // World membership is per kind, so the same bounds serve all World views.
    words[5] = f32::from_bits(address(words.len())?);
    let mask_words = data.worlds.len().div_ceil(32);
    words[6] = f32::from_bits(address(mask_words + 2)?);
    for instance in instances {
        words.extend([instance.kind, instance.row].map(f32::from_bits));
        for worlds in data.worlds.chunks(32) {
            let mut mask = 0u32;
            for (i, world) in worlds.iter().enumerate() {
                if world.kinds.contains(&instance.kind) { mask |= 1 << i; }
            }
            words.push(f32::from_bits(mask));
        }
    }
    address(words.len())?;

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

pub(crate) fn module(
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
    let world = world.split_inclusive('\n').filter(|line| {
        !line.starts_with("@group(1)") || !head.lines().any(|original| original == line.trim_end())
    }).collect::<String>();
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

pub(crate) fn classify_module(visibility: Visibility) -> String {
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

pub(crate) fn present_module(
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

#[cfg(test)]
mod tests {
    use crate::program::Pass;
    use loft_ir::Code;
    use std::path::Path;

    #[test]
    fn tiled_world_callback_keeps_raster_passes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
        let fixture = include_str!("../../../../stdlib/engines/capsules.loft");
        let scene = format!("use stdlib::lighting::world::*
{fixture}");
        let scene = scene.replace(
            "schedule Main {",
            "fn probe(w: World, h: Hit, wo: d3@World, ink: bool, px: v2) -> rgb { visible(w, h.p, h.p) }
schedule Main {",
        ).replace("light: sun_sky", "shade: probe, samples: 4");
        let entry = root.join("stdlib/engines/capsules.loft");
        let src = crate::source::Graph::text(&root, &entry, &scene);
        let (program, _, compiled) = crate::compile(&src, &[("count".into(), 1.0)])
            .unwrap_or_else(|e| panic!("{}", e.render(&src)));
        assert!(matches!(&program.passes[..], [Pass::Cull, Pass::Vis(_), Pass::Classify, Pass::Shade(_), Pass::Present]));
        assert!(program.path.is_none());
        assert!(program.modules[0].wgsl.contains(", hit, wo, false, px)"));
        let bundle = crate::bundle::lower(&program, &compiled, (64, 64), |_, _| vec![0; 256]);
        assert!(bundle.is_ok(), "{:?}", bundle.err());

        let src = crate::source::Graph::text(&root, &entry, &scene.replace("samples: 4", "samples: 2"));
        assert_eq!(crate::compile(&src, &[]).err().unwrap().code, Code::E602);
    }
}
