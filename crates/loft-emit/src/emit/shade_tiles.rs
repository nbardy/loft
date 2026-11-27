//! Lower the shade callback against the existing raster and World contracts.
use super::{Emitter, KindModule, LutRef, fn_symbol, paths, reachable, render_template};
use loft_check::{Prog, program::KindTable};
use loft_ir::{Res, Span};
use loft_parse::ast::Ty;
use std::collections::{BTreeMap, BTreeSet};

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

pub struct WorldSource {
    source: String,
    sampled_bakes: BTreeSet<String>,
}

// The tiled module shares query code with the screen module and reads its World data through the raster LUT binding.
pub fn world_source(
    p: &Prog, types: &[Ty], kinds: &[KindTable], queries: &[KindModule],
    fixed: &BTreeMap<(String, String), String>, span: Span,
) -> Res<WorldSource> {
    let mut em = Emitter::new(p, types);
    em.fixed = fixed;
    paths::emit_world(&mut em, kinds, queries, false, paths::WorldStorage::Tiled, span)?;
    Ok(WorldSource { source: em.out, sampled_bakes: em.sampled_bakes })
}

pub fn module(
    p: &Prog, types: &[Ty], luts: &[LutRef], kind: &KindModule, world: &WorldSource, callback: &str, sky: &str,
    visibility: Visibility, primary_world: u32,
) -> String {
    let (functions, constants) = reachable(p, &[], &[sky, callback]);
    let symbols = paths::declarations(&kind.queries);
    let functions = functions.into_iter().filter(|f| !symbols.contains(fn_symbol(&f.name.name).as_str())).collect::<Vec<_>>();
    let constants = constants.into_iter().filter(|name| !symbols.contains(format!("loft_const_{name}").as_str())).collect();
    let mut em = Emitter::new(p, types);
    em.luts = luts;
    em.library(&functions, &constants);
    em.sampled_bakes.extend(world.sampled_bakes.iter().cloned());
    em.sampled_textures();
    let head = format!("{}{}{}", kind.raster_prefix, kind.queries, kind.raster_bindings);
    let callback = fn_symbol(callback);
    let sky = fn_symbol(sky);
    let mut out = render_template(include_str!("../../../../stdlib/core/tiled_hit.wgsl"), &[
        ("HEAD", &head), ("WORLD", &world.source), ("LIBRARY", &em.out), ("CALLBACK", &callback),
    ]);
    let (shade, alpha) = match visibility {
        Visibility::Center => (include_str!("../../../../stdlib/render/shade_world_center.wgsl").to_owned(), "1.0"),
        Visibility::SubpixelEdges => {
            out.push_str(NEIGHBORS);
            out += &render_template(include_str!("../../../../stdlib/render/shade_edges.wgsl"), &[
                ("PRIMARY_WORLD", &primary_world.to_string()), ("SKY", &sky),
            ]);
            (include_str!("../../../../stdlib/render/shade_world_taps.wgsl").to_owned(), "1.0")
        }
        Visibility::Coverage => {
            out.push_str(NEIGHBORS);
            // World queries resolve every row's field, including silhouette pairs from different kinds.
            out += &render_template(COVERAGE, &[
                ("USABLE", "true"),
                ("SETUP", "  let leaf = LoftLeafRef(key >> 24u, key & 0xffffffu);\n  let at = world_at(leaf);"),
                ("FIELD", "world_field(leaf.kind_id, at, x)"),
            ]);
            (format!("{COVERAGE_OWNER}{}", include_str!("../../../../stdlib/render/shade_world_hit.wgsl")), "loft_alpha")
        }
    };
    out += &render_template(include_str!("../../../../stdlib/render/tiled_shade.wgsl"), &[
        ("SHADE", &shade), ("ALPHA", alpha), ("TWIN", &kind.twin),
    ]);
    out
}

/// `edges: coverage` on the `light:` path: a ray kind's module (it has its own `trace` field) also owns the halo
/// pixels next to its rows and encodes coverage in the HDR alpha. It can resolve only rows of its own kind, so an
/// occluder of another kind is left to that kind's own-silhouette side.
pub(super) fn bsdf_coverage() -> String {
    let coverage = render_template(COVERAGE, &[
        ("USABLE", "(key >> 24u) == kp.kind_id"),
        ("SETUP", "  let row = load_row(key & 0xffffffu);\n  let xf = loft_xf_inv(loft_pose(row));"),
        ("FIELD", "q_trace(row, loft_xf_pt(xf, x))"),
    ]);
    format!("{NEIGHBORS}{coverage}")
}

const NEIGHBORS: &str = include_str!("../../../../stdlib/core/vis_neighbors.wgsl");
const COVERAGE: &str = include_str!("../../../../stdlib/render/shade_coverage.wgsl");
pub(super) const COVERAGE_OWNER: &str = include_str!("../../../../stdlib/render/shade_coverage_owner.wgsl");
