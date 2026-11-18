// What a shader stage knows of the screen and the camera, for the library's own looks and drawables: raw WGSL functions
// (src/text), so the CPU twin and the checks refuse them by name and line. Not public: an author writes the same
// one-line raw function in their own `wgsl` text (docs/guide.md, "Raw functions").
import type { F, P3 } from "../map/types.ts";
import type { In } from "../expr.ts";
import { wgsl } from "../text/index.ts";

const screen = wgsl`
fn pixel_rate(x: f32) -> f32 { return fwidth(x); }
fn toward_point(p: vec3<f32>) -> vec3<f32> { return view_dir(p); }
fn eye_point() -> vec3<f32> { return frame.eye.xyz; }
fn pixel_size(p: vec3<f32>) -> f32 { return pixel_world(p); }`;

/**
 * How much `x` changes from one pixel to the next (WGSL `fwidth`): the width to antialias an edge of `x` with.
 * Fragment stage only, and only in uniform control flow (WGSL's derivative rule).
 */
export const fwidth: (x: In<"f32">) => F = screen.pixel_rate!;
/** Unit direction from the eye toward a world point (the camera's forward direction when orthographic). */
export const viewDir: (p: In<"vec3">) => P3 = screen.toward_point!;
/** The eye position in world space. */
export const eyePosition: () => P3 = screen.eye_point!;
/** World size of one pixel at a world point. */
export const pixelWorld: (p: In<"vec3">) => F = screen.pixel_size!;
