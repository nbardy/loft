//! Lower the shade callback against the existing raster and World contracts.
use super::{Aov, Emitter, LutRef, SHADE_ENTRY, TWIN_BINDINGS, fn_symbol, paths, reachable, render_template};
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
    out += &render_template(include_str!("../../../../stdlib/core/tiled_shade.wgsl").strip_suffix('\n').unwrap(), &[
        ("SHADE", shade), ("TWIN_BINDINGS", TWIN_BINDINGS), ("TWIN", twin),
    ]);
    out
}

pub(crate) const HALO_BINDINGS: &str = "@group(1) @binding(3) var hdr: texture_storage_2d<rgba16float, write>;\n";

const NEIGHBORS: &str = include_str!("../../../../stdlib/core/vis_neighbors.wgsl");

pub fn classify_module(visibility: Visibility) -> String {
    match visibility {
        Visibility::Center => super::classify_module(),
        // Classify also clears the HDR target's alpha, so present can tell an unshaded pixel from a shaded one by that
        // alpha alone (shade stores 1.0) instead of rereading the 3x3 visibility neighborhood on every sky pixel.
        Visibility::SubpixelEdges => super::classify_with_halo(
            include_str!("../../../../stdlib/core/classify_halo.wgsl").strip_suffix('\n').unwrap(), HALO_BINDINGS),
    }
}

pub fn present_module(
    p: &Prog, types: &[Ty], sky: &str, aov: Aov, paper: Option<&str>, film: Option<&str>, visibility: Visibility,
) -> String {
    let edges = matches!(visibility, Visibility::SubpixelEdges);
    // Shade stores alpha 1.0 at exactly the pixels whose 3x3 visibility neighborhood is covered, and classify zeroed
    // every other pixel's alpha this frame, so beauty present reads that alpha rather than the neighborhood.
    let mut source = super::emit_present(p, types, sky, aov, paper, film, edges && aov == Aov::Beauty);
    if edges && aov != Aov::Beauty {
        source = source.replace("textureLoad(vis_tex, px, 0).x == LOFT_EMPTY",
            "textureLoad(vis_tex, px, 0).x == LOFT_EMPTY && loft_vis_neighborhood(px, LOFT_EMPTY).x == LOFT_EMPTY");
        source.push_str(NEIGHBORS);
    }
    source
}
