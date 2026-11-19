// Pieces every style and part shares: an antialiased edge, colour helpers, the distance in from a silhouette, and WGSL
// for post passes.
//
// A post pass is raw WGSL (it reads the picture so far, which no expression can), prepended by the host with
// PRELUDE_WGSL, POST_WGSL and `sceneWgsl` (DESIGN.md 3). Everything defined here is prefixed `sty_` so it cannot
// collide with a library function that `sceneWgsl` happens to carry.

import postCommon from "../wgsl/post_common.wgsl" with { type: "text" };
import { add, div, dot, max, min, mul, smoothstep, sub, vec3, type Expr } from "../expr.ts";
import { fwidth } from "../lib/screen.ts";
import type { Rgb, ShadeCtx } from "../style.ts";

/** A colour triple as a vec3 expression, for use inside a style's shading. */
export const rgb = (c: Rgb): Expr<"vec3"> => vec3(c[0], c[1], c[2]);
/** A colour triple as WGSL source text (`vec3<f32>(r, g, b)`), for style code written as raw WGSL. */
export const wgslRgb = (c: Rgb): string => `vec3<f32>(${c.map((x) => x.toFixed(4)).join(", ")})`;

/**
 * 0 below `edge`, 1 above, antialiased over about a pixel of x's change (never narrower than `soft`, never wider
 * than `widest`, in units of x). Fragment stage only (it takes a derivative), so only in uniform control flow.
 */
export function edgeOf(x: Expr<"f32">, edge: number | Expr<"f32">, soft: number, widest = 0.05): Expr<"f32"> {
  // Capped: where x jumps between neighbouring pixels (the contact band does where the search ring starts to find a
  // body) an uncapped width spread that jump into half-covered pixels, a dotted ghost of the silhouette on the water.
  const w = min(max(mul(fwidth(x), 0.75), soft), widest);
  return smoothstep(sub(edge, w), add(edge, w), x);
}

/**
 * How many pixels in from the silhouette a body fragment is, to first order: how far the surface faces the eye
 * (N.V) over how fast that changes across the screen. 0 on the silhouette, growing inward; a band `w` pixels wide along
 * every outline is where this is under `w` (the rim, a line drawn heavier inside the hull, a neon tube's glow).
 *
 * Cannot: see a crease or a fold inside the outline (N.V does not reach 0 there), or measure in the world. It is a
 * fragment's own derivative, so constant across a triangle's quad and rough by a pixel where the mesh is coarse.
 * Going past it: for creases, a post pass on depth and reconstructed normals (`sty_normal` in `POST_COMMON`); for a
 * width in world units divide a world width by `pixelWorld(ctx.world)`, as `weighted` and the rim do.
 */
export function pixelsInside(ctx: ShadeCtx): Expr<"f32"> {
  const facing = dot(ctx.normal, ctx.view);
  return div(facing, max(fwidth(facing), 1e-5));
}

/**
 * WGSL helpers for post passes: a hash of a pixel index, value noise fixed to the screen, the view depth of the opaque
 * scene, and whether a pixel shows the sea in front of whatever opaque thing is behind it.
 */
export const POST_COMMON = postCommon;
