//! Lower the shade callback against the existing raster and World contracts.
use super::{Emitter, LutRef, SHADE_ENTRY, TWIN_BINDINGS, fn_symbol, paths, reachable, render_template};
use loft_check::{Prog, program::{KindTable, abi}};
use loft_ir::{Res, Span};
use loft_parse::ast::Ty;
use std::{collections::BTreeSet, fmt::Write};

/// `edges:` of `shade_tiles`: `none`, `taps` (four BVH rays per edge pixel) or `coverage` (analytic, no extra rays).
#[derive(Clone, Copy)]
pub enum Visibility {
    Center,
    SubpixelEdges,
    Coverage,
}

#[derive(Clone, Copy)]
pub enum Lighting<'a> {
    Bsdf(&'a str, Visibility),
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
    Ok(render_template(include_str!("../../../../stdlib/core/tiled_world.wgsl"), &[("SOURCE", &source)]))
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
    let mut out = render_template(include_str!("../../../../stdlib/core/tiled_hit.wgsl"), &[
        ("HEAD", head), ("WORLD", world), ("LIBRARY", &em.out), ("CALLBACK", &callback),
    ]);
    let (shade, alpha) = match visibility {
        Visibility::Center => (r"
  if v.x == LOFT_EMPTY || (v.x >> 24u) != kp.kind_id { return; }
  let color = loft_shade_hit(LoftLeafRef(kp.kind_id, v.x & 0xffffffu), center, bitcast<f32>(v.y), -center.d, vec2f(px));
", "1.0"),
        Visibility::SubpixelEdges => {
            out.push_str(NEIGHBORS);
            let _ = writeln!(out, "const LOFT_PRIMARY_WORLD = {primary_world}u;");
            let _ = writeln!(out, "fn loft_edge_sky(rd: vec3f) -> vec3f {{ return {sky}(rd, normalize(frame.sun.xyz)); }}");
            out.push_str(include_str!("shade_edges.wgsl"));
            (r"
  if v.x != LOFT_EMPTY && (v.x >> 24u) != kp.kind_id { return; }
  let near = loft_vis_neighborhood(px, v.x);
  if near.x == LOFT_EMPTY || (near.x >> 24u) != kp.kind_id { return; }
  let color = loft_shade_samples(center, vec2f(px), v, near.y != 0u);
", "1.0")
        }
        Visibility::Coverage => {
            out.push_str(NEIGHBORS);
            // The World path resolves any row's field, so every pair of rows gets coverage.
            out += &render_template(COVERAGE, &[("USABLE", "true"),
                ("SETUP", "  let leaf = LoftLeafRef(key >> 24u, key & 0xffffffu);\n  let at = world_at(leaf);"),
                ("FIELD", "world_field(leaf.kind_id, at, x)")]);
            (concat!(r"
", include_str!("shade_coverage_owner.wgsl"), r"  let color = loft_shade_hit(LoftLeafRef(kp.kind_id, v.x & 0xffffffu), center, bitcast<f32>(v.y), -center.d, vec2f(px));
"), "loft_alpha")
        }
    };
    out += &render_template(include_str!("../../../../stdlib/core/tiled_shade.wgsl").strip_suffix('\n').unwrap(), &[
        ("SHADE", shade), ("ALPHA", alpha), ("TWIN_BINDINGS", TWIN_BINDINGS), ("TWIN", twin),
    ]);
    out
}

/// `edges: coverage` on the `light:` path: a ray kind's module (it has its own `trace` field) also owns the halo
/// pixels next to its rows and encodes coverage in the HDR alpha. It can resolve only rows of its own kind, so an
/// occluder of another kind is left to that kind's own-silhouette side.
pub fn bsdf_coverage(source: &str) -> String {
    const CENTER: &str = "  if v.x == LOFT_EMPTY || (v.x >> 24u) != kp.kind_id { return; }\n";
    const STORE: &str = "  textureStore(hdr, px, vec4<f32>(";
    let (head, rest) = source.split_once(SHADE_ENTRY).expect("kind shade entry");
    let (entry, rest) = rest.split_once(CENTER).expect("kind shade visibility test");
    let (body, rest) = rest.split_once(STORE).expect("kind shade store");
    let (line, tail) = rest.split_once('\n').expect("kind shade store line");
    let shade = line.strip_suffix(", 1.0));").expect("kind shade opaque store");
    let coverage = render_template(COVERAGE, &[("USABLE", "(key >> 24u) == kp.kind_id"),
        ("SETUP", "  let row = load_row(key & 0xffffffu);\n  let xf = loft_xf_inv(loft_pose(row));"),
        ("FIELD", "q_trace(row, loft_xf_pt(xf, x))")]);
    format!("{head}{NEIGHBORS}{coverage}{SHADE_ENTRY}{entry}{}{body}{STORE}{shade}, loft_alpha));\n{tail}",
        include_str!("shade_coverage_owner.wgsl"))
}

const NEIGHBORS: &str = include_str!("../../../../stdlib/core/vis_neighbors.wgsl");
const COVERAGE: &str = include_str!("shade_coverage.wgsl");
