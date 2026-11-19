//! Emit's prelude: every global that generated or library WGSL may read, in one text at the top of every module
//! (graph modules, kernel modules, draw modules). Its bytes are `loft_ir::frame`'s, which the host writes.
//!
//! - `frame` (`@group(0) @binding(0)`): the camera, the viewport, the time, the sun, then the params and inputs that
//!   generated code reads (`frame.params[k / 4]`, `frame.inputs[j]`). The field names are the TS host's, because
//!   library raw WGSL reads them (`camera_ray`, the sea's plane).
//! - `loft_detail` (`@group(0) @binding(1)`): the level-of-detail slots, and `LOFT_DETAIL_SLOT`, the override naming
//!   a draw's first slot (the host sets it per pipeline; the sea's plane reads its rows there).
//! - `view_dir(world)`: the unit direction from the eye toward a world point; `pixel_world(p)`, the world size of a
//!   pixel at p; `sea_footprint(p)`, a pixel's longest footprint on the sea; `copy_id`, `quad_corner` (TS `CORE_WGSL`,
//!   the helpers with no scene content).
//! - `scene_depth(px)`, `scene_position(px)`, `scene_clearance(px)` (TS `SCENE_DEPTH_WGSL`, `SCENE_CLEARANCE_WGSL`): the
//!   opaque scene's resolved depth (r32float, `@group(0) @binding(2)`) and its 8 × 8 tile clearance (r8unorm,
//!   `@group(0) @binding(3)`), which the sea's contact band reads; and `loft_scene_colour` (`@group(0) @binding(4)`), the
//!   pass's colour so far, which Screen covers and the ink composer read, with `post_color(uv)` (TS POST_WGSL's: bilinear,
//!   clamped, through `loft_scene_sampler` at binding 5) for the styles' post passes. The host writes them after the
//!   opaque draws, and before a loading pass that opens with a Screen cover (a post pass).
//!
//! A raw function that reads any other global is refused by `needs`: `Scene { Global { name, needs } }`, naming the
//! decl or prelude change that would give it one. A module is never built with a global left out.
use loft_ir::Error;
use loft_ir::error::{Needs, SceneFault};
use loft_ir::frame;
use loft_ir::graph::{Graph, Op};

/// The prelude of a module whose frame holds `params` param vec4s and `inputs` input vec4s (each at least 1).
pub fn text(params: u32, inputs: u32) -> String {
    format!(
        "struct Frame {{ view_proj: mat4x4<f32>, inv_view_proj: mat4x4<f32>, eye: vec4<f32>, forward: vec4<f32>, \
         viewport: vec4<f32>, time: vec4<f32>, sun: vec4<f32>, params: array<vec4<f32>, {}>, inputs: array<vec4<f32>, {}> }}\n\
         @group(0) @binding(0) var<uniform> frame: Frame;\n\
         @group(0) @binding(1) var<uniform> loft_detail: array<vec4<f32>, {}>;\n\
         override LOFT_DETAIL_SLOT: u32 = 0u;\n\
         fn view_dir(world: vec3<f32>) -> vec3<f32> {{ return normalize(mix(frame.forward.xyz, world - frame.eye.xyz, frame.eye.w)); }}\n\
         fn copy_id(id: f32, i: u32) -> f32 {{ return fract(id + f32(i) * 0.6180340); }}\n\
         fn quad_corner(c: u32) -> vec2<u32> {{ return vec2<u32>(select(0u, 1u, c == 1u || c == 2u || c == 4u), \
         select(0u, 1u, c == 2u || c == 4u || c == 5u)); }}\n\
         fn pixel_world(p: vec3<f32>) -> f32 {{ let clip = frame.view_proj * vec4<f32>(p, 1.0); \
         let fy = length(vec3<f32>(frame.view_proj[0][1], frame.view_proj[1][1], frame.view_proj[2][1])); \
         return 2.0 * clip.w / (frame.viewport.y * fy); }}\n\
         fn sea_footprint(p: vec3<f32>) -> f32 {{ return pixel_world(p) / max(abs(view_dir(p).y), 0.02); }}\n\
         @group(0) @binding(2) var loft_scene_depth: texture_2d<f32>;\n\
         @group(0) @binding(3) var loft_scene_clearance: texture_2d<f32>;\n\
         @group(0) @binding(4) var loft_scene_colour: texture_2d<f32>;\n\
         @group(0) @binding(5) var loft_scene_sampler: sampler;\n\
         fn post_color(uv: vec2<f32>) -> vec4<f32> {{ return textureSampleLevel(loft_scene_colour, loft_scene_sampler, uv, 0.0); }}\n\
         fn scene_depth(px: vec2<f32>) -> f32 {{ let size = vec2<i32>(textureDimensions(loft_scene_depth)); \
         return textureLoad(loft_scene_depth, clamp(vec2<i32>(floor(px)), vec2<i32>(0), size - 1), 0).x; }}\n\
         fn scene_position(px: vec2<f32>) -> vec3<f32> {{ let ndc = vec2<f32>(px.x * frame.viewport.z * 2.0 - 1.0, \
         1.0 - px.y * frame.viewport.w * 2.0); let h = frame.inv_view_proj * vec4<f32>(ndc, scene_depth(px), 1.0); \
         return h.xyz / h.w; }}\n\
         fn scene_clearance(px: vec2<f32>) -> f32 {{ let size = vec2<i32>(textureDimensions(loft_scene_depth)); \
         let c = textureLoad(loft_scene_clearance, clamp(vec2<i32>(floor(px)), vec2<i32>(0), size - 1) / {}, 0).x; \
         return c * {}.0 - {:?}; }}",
        params.max(1),
        inputs.max(1),
        frame::DETAIL_SLOTS,
        CLEARANCE_TILE,
        CLEARANCE_SCALE,
        // TS: (0.5 / 255) * CLEARANCE_SCALE + 0.005, the half step the 8-bit store may have rounded up.
        0.5 / 255.0 * f64::from(CLEARANCE_SCALE) + 0.005
    )
}

/// src/artefact.ts `CLEARANCE_TILE` and `CLEARANCE_SCALE`: the clearance map's tile side (pixels) and its scale.
const CLEARANCE_TILE: u32 = 8;
const CLEARANCE_SCALE: u32 = 64;

/// What a global outside the prelude needs, by the TS host's naming of the places it came from.
pub fn needs(name: &str) -> Needs {
    match name {
        "loft_cast" | "soft_box" => Needs::Draw,
        "shade" | "ink" | "mark" | "sky" | "outline_world" | "ink_width" | "sea_height" | "sea_gradient"
        | "sea_shadow" | "ground_colour" | "SUN" => Needs::Style,
        n if n.starts_with("loft_cast_") || n.ends_with("_soft_box") => Needs::Draw,
        n if n.starts_with("loft_passtex_") => Needs::Pass,
        n if n.starts_with("loft_compute_") => Needs::Kernel,
        n if n.starts_with("t_") => Needs::Table,
        _ => Needs::Prelude,
    }
}

/// Every raw function's globals against the prelude, whether or not Naga runs at emit (`Naga::Load` leaves the rest
/// of validation to the host): a raw function with no text (a name a TS draw module defines, `loft_cast`) or one that
/// reads a name the prelude does not declare is `Scene { Global { name, needs } }`, at the first node calling it. Any
/// other fault in its source is Naga's to report, at emit or at load.
pub fn globals(g: &Graph) -> Result<(), Error> {
    let caller = |r: u32| g.nodes.iter().position(|n| n.op == Op::Raw(r)).and_then(|n| g.site(n as u32).cloned());
    let base = format!("alias lf = f32;\n{}\n", text(1, 1));
    (0..g.raws.len() as u32).try_for_each(|r| {
        let raw = &g.raws[r as usize];
        let refuse = |name: &str| Error::Scene {
            site: caller(r),
            fault: SceneFault::Global { name: name.into(), needs: needs(name) },
        };
        if g.sym(raw.text).trim().is_empty() {
            return Err(refuse(g.sym(raw.name)));
        }
        let mut src = base.clone();
        for d in super::raw_closure(g, r) {
            for read in g.raws[d as usize].reads.iter() {
                if let loft_ir::graph::Read::Table(k) = *read {
                    src.push_str(&super::accessor(g, k, None));
                    src.push('\n');
                }
            }
        }
        for d in super::raw_closure(g, r) {
            src.push_str(g.sym(g.raws[d as usize].text));
            src.push('\n');
        }
        match naga::front::wgsl::parse_str(&src) {
            Ok(_) => Ok(()),
            Err(e) => unbound(e.message()).map_or(Ok(()), |name| Err(refuse(name))),
        }
    })
}

/// The identifier a Naga front-end message says has no definition (`no definition in scope for identifier: `X``).
/// Naga's message is the only place this is known; it is read here, once, at the boundary.
pub fn unbound(msg: &str) -> Option<&str> {
    msg.strip_prefix("no definition in scope for identifier: `")?.strip_suffix('`')
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The prelude's struct and `loft_ir::frame`'s offsets are one contract: a field moved in one and not the other
    /// would make the host write the camera where the shader reads the time.
    #[test]
    fn frame_offsets_match_loft_ir() {
        let m = naga::front::wgsl::parse_str(&text(3, 2)).expect("the prelude parses");
        let mut layouter = naga::proc::Layouter::default();
        layouter.update(m.to_ctx()).expect("layout");
        let (_, ty) = m.types.iter().find(|(_, t)| t.name.as_deref() == Some("Frame")).expect("struct Frame");
        let naga::TypeInner::Struct { members, .. } = &ty.inner else { panic!("Frame is a struct") };
        let at = |name: &str| members.iter().find(|x| x.name.as_deref() == Some(name)).map(|x| x.offset as usize);
        let want = [
            ("view_proj", frame::VIEW_PROJ),
            ("inv_view_proj", frame::INV_VIEW_PROJ),
            ("eye", frame::EYE),
            ("forward", frame::FORWARD),
            ("viewport", frame::VIEWPORT),
            ("time", frame::TIME),
            ("sun", frame::SUN),
            ("params", frame::HEADER),
        ];
        for (name, off) in want {
            assert_eq!(at(name), Some(off), "{name}");
        }
        assert_eq!(at("inputs"), Some(frame::HEADER + 3 * 16));
    }

    #[test]
    fn unbound_names_the_identifier() {
        let e = naga::front::wgsl::parse_str("fn f() -> f32 { return loft_passtex_glow; }").unwrap_err();
        assert_eq!(unbound(e.message()), Some("loft_passtex_glow"));
        assert_eq!(needs("loft_passtex_glow"), Needs::Pass);
    }
}
