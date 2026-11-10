// Woodblock: a Japanese colour print. Bodies are flat areas from a fixed palette of five inks (indigo, a paler blue,
// cream, one vermilion accent, black), each chosen by the lit value of the paint in bands, so nothing on a body is a
// gradient. Black key lines are cut from the opaque scene's depth and normal edges by a post pass, with a weight that
// wanders like a carved line; the colour block is printed slightly out of register with the key block; then paper.
// The sea is flat blues with printed lines along the swell's contours and clawed foam where it meets a body.

import { abs, add, dot, fract, length, max, min, mix, mul, param, saturate, smoothstep, sub, vec4, type Expr } from "../expr.ts";
import { noise2, hash22, luminance } from "../lib/index.ts";
import { fwidth } from "../lib/wgsl.ts";
import type { PostSpec } from "../compile/context.ts";
import type { Rgb, Style } from "../style.ts";
import { edgeOf, POST_COMMON, rgb, wgslRgb } from "./shared.ts";

/** The five inks. Everything a body shows is one of these (antialiased edges aside). */
export const WOODBLOCK_PALETTE = {
  black: [0.075, 0.07, 0.09],
  indigo: [0.13, 0.2, 0.37],
  pale: [0.47, 0.6, 0.72],
  cream: [0.93, 0.88, 0.75],
  vermilion: [0.82, 0.25, 0.16],
} as const satisfies Record<string, Rgb>;

/** Sea inks: the print's water is its own blues (not body inks), darker in the troughs. */
const SEA = { deep: [0.12, 0.2, 0.38], mid: [0.2, 0.33, 0.52], line: [0.62, 0.72, 0.8], haze: [0.7, 0.76, 0.78] } as const satisfies Record<string, Rgb>;

const P = WOODBLOCK_PALETTE;

/**
 * Value bands, in lit luminance: below `black` the key block's black, then indigo, the pale blue, and cream. A paint
 * whose red stands above both its green and blue by `warm` prints vermilion whatever its value.
 */
const BAND = { black: 0.1, indigo: 0.34, pale: 0.6, warm: 0.28 } as const;

/**
 * The key-line pass. A pixel is inked when, within the line's half-width, the opaque scene steps away from it (a
 * silhouette: only the NEAR side is inked, so an edge gets one line, not one on each surface) or turns sharply (a
 * crease). The half-width wanders with low-frequency noise in world space, so the line swells and thins like a cut
 * one, and it is thinner far away. The colour block is read slightly out of register; the key line is not.
 */
const KEYLINE = /* wgsl */ `${POST_COMMON}
const WB_INK: vec3<f32> = ${wgslRgb(P.black)};
const WB_DIRS = array<vec2<f32>, 8>(vec2(1.0, 0.0), vec2(-1.0, 0.0), vec2(0.0, 1.0), vec2(0.0, -1.0),
  vec2(0.7071, 0.7071), vec2(-0.7071, 0.7071), vec2(0.7071, -0.7071), vec2(-0.7071, -0.7071));

@fragment fn fs(in: PostIn) -> @location(0) vec4<f32> {
  let px = in.px.xy;
  // Pixel sizes are relative to an 800-pixel-high frame, so a smaller render keeps the same picture.
  let scale = max(frame.viewport.y / 800.0, 0.375);
  let colour = post_color((px + vec2<f32>(1.4, -1.0) * scale) * frame.viewport.zw).rgb;
  if (!sty_solid(px) || sty_submerged(px)) { return vec4<f32>(colour, 1.0); }
  let p = scene_position(px);
  let z = sty_view_depth(px);
  // Half-width in pixels: 1 to 2.6 at the reference size, wandering along the line.
  let wobble = sty_noise(p.xz * 1.3 + vec2<f32>(p.y * 1.7, 0.0));
  let w = scale * (1.0 + 1.6 * wobble * wobble) * clamp(18.0 / z, 0.6, 1.4);
  let n = sty_normal(px, 1.5 * scale);
  var ink = 0.0;
  for (var k = 0u; k < 8u; k++) {
    let q = px + WB_DIRS[k] * w;
    let d = sty_view_depth(q) - z;
    // A step: large against the depth (a plane receding from the eye is not one) and in world units.
    let silhouette = min(smoothstep(0.02, 0.05, d / z), smoothstep(0.1, 0.25, d));
    // A crease: the surface turns by more than about 55 degrees within the line's width, with no step.
    let nq = sty_normal(px + WB_DIRS[k] * w * 0.7, 1.5 * scale);
    let crease = (1.0 - smoothstep(0.45, 0.6, dot(n, nq))) * (1.0 - smoothstep(0.05, 0.15, abs(d) / z));
    ink = max(ink, max(silhouette, crease));
  }
  return vec4<f32>(mix(colour, WB_INK, ink), 1.0);
}
`;

/**
 * Paper: fibres (screen-fixed value noise stretched along x, the direction the paper was laid) and a fine tooth that
 * shows most in the pale inks, where a print's paper shows through.
 */
const PAPER = /* wgsl */ `${POST_COMMON}
@fragment fn fs(in: PostIn) -> @location(0) vec4<f32> {
  let c = post_color(in.uv).rgb;
  let scale = max(frame.viewport.y / 800.0, 0.375);
  let q = in.px.xy / scale;
  let fibre = sty_noise(q * vec2<f32>(0.08, 0.7)) * 0.6 + sty_noise(q * vec2<f32>(0.03, 0.25)) * 0.4;
  let tooth = sty_hash(vec2<i32>(in.px.xy));
  let light = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
  let g = (fibre - 0.5) * 0.07 + (tooth - 0.5) * 0.035 * (0.4 + light);
  return vec4<f32>(c * (1.0 + g), 1.0);
}
`;

/** The woodblock's two post passes, by name (the tests run the key line alone on a flat region). */
export const WOODBLOCK_POST = {
  keyline: { label: "woodblock key line", wgsl: KEYLINE },
  paper: { label: "woodblock paper", wgsl: PAPER },
} as const satisfies Record<string, PostSpec>;

/** The palette colour a lit value prints in: black, indigo, pale blue, cream, or vermilion for a warm paint. */
function inked(base: Expr<"vec3">, value: Expr<"f32">): Expr<"vec3"> {
  let c: Expr<"vec3"> = rgb(P.black);
  c = mix(c, rgb(P.indigo), edgeOf(value, BAND.black, 0.004));
  c = mix(c, rgb(P.pale), edgeOf(value, BAND.indigo, 0.004));
  c = mix(c, rgb(P.cream), edgeOf(value, BAND.pale, 0.004));
  const warmth = min(sub(base.x, base.y), sub(base.x, base.z));
  return mix(c, rgb(P.vermilion), edgeOf(warmth, BAND.warm, 0.004));
}

export function woodblock(): Style {
  const outline = param("woodblock.outline", 0.0, { min: 0, max: 0.1 });
  return {
    name: "woodblock",
    // The hull is only the key block's minimum (1.5 px); the key-line pass draws the weight.
    outline,
    clear: P.cream,
    post: [WOODBLOCK_POST.keyline, WOODBLOCK_POST.paper],
    shade: (ctx) => {
      // Two tones of light (lit, shadow), then the value bands: each paint region prints in at most two inks.
      const lit = edgeOf(dot(ctx.normal, ctx.sun), 0.08, 0.01);
      const value = mul(luminance(ctx.base), mix(0.56, 1.06, lit));
      return inked(ctx.base, value);
    },
    // The key block's black: the hull merges with the key line drawn over it.
    ink: () => rgb(P.black),
    water: (ctx) => {
      const xz = ctx.world.xz;
      // Two flat blues, trough and crest, split along a wandering contour.
      const h = add(ctx.crest, mul(noise2(mul(xz, 0.35)), 0.25));
      let c: Expr<"vec3"> = mix(rgb(SEA.deep), rgb(SEA.mid), edgeOf(h, 0.05, 0.01));
      // Printed lines along the swell's contours, wider on the crests, fading out where they would alias.
      const phase = add(mul(h, 3.2), mul(noise2(mul(xz, 1.1)), 0.18));
      const fw = max(fwidth(phase), 1e-4);
      const d = abs(sub(fract(add(phase, 0.5)), 0.5)); // 0 on a contour, 0.5 halfway between two
      const width = mix(0.06, 0.16, saturate(add(mul(ctx.crest, 0.5), 0.5)));
      const line = mul(sub(1, smoothstep(sub(width, fw), add(width, fw), d)), sub(1, smoothstep(0.18, 0.4, fw)));
      c = mix(c, rgb(SEA.line), mul(line, 0.85));
      // Clawed foam: the contact foam's band, its edge torn into fingers by ridged noise, with an indigo line round
      // it and loose specks of spray in the band beyond it.
      const ridge = abs(noise2(mul(xz, 2.6)));
      const reach = sub(mul(ctx.foam, 1.25), mul(ridge, 0.55));
      const foam = edgeOf(reach, 0.2, 0.01);
      const rim = mul(edgeOf(reach, 0.08, 0.01), sub(1, foam));
      const speck = mul(specks(mul(xz, 7)), smoothstep(0.02, 0.08, ctx.foam));
      c = mix(c, rgb(P.indigo), rim);
      c = mix(c, rgb(P.cream), max(foam, speck));
      // The far sea fades in two flat steps toward the haze at the horizon.
      const far = add(mul(edgeOf(ctx.distance, 70, 1), 0.45), mul(edgeOf(ctx.distance, 140, 1), 0.4));
      c = mix(c, rgb(SEA.haze), far);
      // Opaque: a print shows nothing under its water.
      return vec4(c, 1);
    },
    shadow: (ctx) => vec4(rgb(P.indigo), mul(edgeOf(ctx.amount, 0.3, 0.01), 0.5)),
    sky: (ctx) => {
      // Bokashi: cream at the horizon graded to the pale blue, with a band of indigo printed across the top.
      const up = ctx.dir.y;
      const above = mix(mix(rgb(P.cream), rgb(SEA.haze), smoothstep(0.0, 0.12, up)), rgb(P.indigo), smoothstep(0.18, 0.5, up));
      return mix(above, rgb(SEA.deep), sub(1, smoothstep(-0.002, 0.0, up)));
    },
  };
}

/** Round specks of spray, one per cell of p at a hashed spot, on about a third of the cells; 0..1 coverage. */
function specks(p: Expr<"vec2">): Expr<"f32"> {
  const h = hash22(p);
  const at = sub(sub(fract(p), 0.5), mul(sub(h, 0.5), 0.5));
  const r = length(at);
  const radius = mul(smoothstep(0.62, 0.9, h.x), 0.22);
  const fw = max(fwidth(r), 1e-4);
  return sub(1, smoothstep(sub(radius, fw), add(radius, fw), r));
}
