//! The bytes behind emit's prelude (loft-core `emit::prelude`): the frame uniform at `@group(0) @binding(0)` and the
//! level-of-detail table at `@group(0) @binding(1)`. The prelude writes the WGSL that reads them; the host (loft-gpu)
//! writes these bytes. Both read this module, because neither crate links the other.
//!
//! The frame is a fixed header, then the params (4 per vec4) and the inputs (one vec4 each):
//!
//! ```text
//!   0  view_proj      mat4x4  clip from world (columns)
//!  64  inv_view_proj  mat4x4  world from clip
//! 128  eye            vec4    xyz eye; w 1 perspective, 0 orthographic
//! 144  forward        vec4    xyz unit view direction
//! 160  viewport       vec4    width, height, 1 / width, 1 / height, pixels
//! 176  time           vec4    x seconds
//! 192  sun            vec4    xyz unit direction toward the sun
//! 208  params         array<vec4<f32>, P>, then inputs array<vec4<f32>, I>   (P, I at least 1)
//! ```
//!
//! The field names are the TS host's (`src/artefact.ts` PRELUDE_WGSL), because library raw WGSL reads them by name
//! (`frame.inv_view_proj` in the sea's plane, `frame.viewport` in `camera_ray`). The draw templates read the same
//! fields, so there is one camera record, not a frame and a view.

/// Byte offsets of the header's fields, in WGSL struct order.
pub const VIEW_PROJ: usize = 0;
pub const INV_VIEW_PROJ: usize = 64;
pub const EYE: usize = 128;
pub const FORWARD: usize = 144;
pub const VIEWPORT: usize = 160;
pub const TIME: usize = 176;
pub const SUN: usize = 192;
/// The header's size: the params start here.
pub const HEADER: usize = 208;

/// Slots (vec4) of the level-of-detail uniform `loft_detail`: 64 KiB, the default `maxUniformBufferBindingSize`.
/// What a slot holds is the drawing library's (grid cell counts, the sea's rows); the host's LOD plan writes them.
pub const DETAIL_SLOTS: usize = 4096;

/// A lens: what a camera says before the host builds its matrices (loft-gpu `camera.wgsl`), `LENS` vec4s.
///
/// ```text
///   0  (kind, fovy, 0, 0)   kind LOOK: perspective, `fovy` radians; kind MATRIX: rows 1..4 are clip-from-world's columns
///   1  eye     | column 0
///   2  target  | column 1
///   3  up      | column 2
///   4  unused  | column 3
/// ```
///
/// A look camera's near and far planes are its distance / 100 and × 100 (TS src/host/camera.ts, which R0 drew with).
pub const LENS: usize = 5;
pub const LOOK: f32 = 0.0;
pub const MATRIX: f32 = 1.0;

/// The lens of a look camera.
pub fn look(eye: [f32; 3], target: [f32; 3], up: [f32; 3], fovy: f32) -> [f32; 4 * LENS] {
    let mut out = [0.0; 4 * LENS];
    out[..4].copy_from_slice(&[LOOK, fovy, 0.0, 0.0]);
    for (k, v) in [eye, target, up].iter().enumerate() {
        out[4 * (k + 1)..4 * (k + 1) + 3].copy_from_slice(v);
    }
    out
}

/// The camera part of the header, as the host sets it. `viewport` is derived from the view size by the host.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub view_proj: [f32; 16],
    pub inv_view_proj: [f32; 16],
    pub eye: [f32; 4],
    pub forward: [f32; 4],
    pub sun: [f32; 4],
}

/// The header's bytes for `camera`, a viewport (w, h, 1/w, 1/h) and a time.
pub fn header(camera: &Camera, viewport: [f32; 4], time: f32) -> [u8; HEADER] {
    let mut out = [0u8; HEADER];
    let mut put = |at: usize, xs: &[f32]| {
        for (k, x) in xs.iter().enumerate() {
            out[at + 4 * k..at + 4 * k + 4].copy_from_slice(&x.to_le_bytes());
        }
    };
    put(VIEW_PROJ, &camera.view_proj);
    put(INV_VIEW_PROJ, &camera.inv_view_proj);
    put(EYE, &camera.eye);
    put(FORWARD, &camera.forward);
    put(VIEWPORT, &viewport);
    put(TIME, &[time]);
    put(SUN, &camera.sun);
    out
}
