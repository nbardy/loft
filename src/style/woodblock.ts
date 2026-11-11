// Woodblock: a colour print. Bodies are flat areas from a fixed palette of five inks (indigo, a paler blue, cream,
// one vermilion accent, black), each chosen by the lit value of the paint in bands, so nothing on a body is a
// gradient. The black key line is the outline hull: geometry, thick and of even weight, antialiased by the frame's
// own multisampling. (It was a post pass that cut lines from depth and normal edges, one decision per pixel: the lines
// came out ragged and broken beside the hull outlines of the other styles, and it cost 0.37 ms a frame. 2026-11-11.)
// Then paper. The sea is flat blues with printed lines along the swell's contours and clawed foam where it meets a body.

import { abs, add, dot, f, fract, length, max, min, mix, mul, param, saturate, smoothstep, sub, vec4, type Expr } from "../expr.ts";
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
const SEA = { deep: [0.11, 0.18, 0.35], mid: [0.16, 0.26, 0.45], line: [0.58, 0.68, 0.78], haze: [0.7, 0.76, 0.78] } as const satisfies Record<string, Rgb>;

const P = WOODBLOCK_PALETTE;

/**
 * Value bands, in lit luminance: below `black` the key block's black, then indigo, the pale blue, and cream. A paint
 * whose red stands above both its green and blue by `warm` prints vermilion whatever its value.
 */
const BAND = { black: 0.08, indigo: 0.34, pale: 0.6, warm: 0.28 } as const;

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

/** The woodblock's post pass. */
export const WOODBLOCK_POST = {
  paper: { label: "woodblock paper", wgsl: PAPER },
} as const satisfies Record<string, PostSpec>;

/** The palette colour a lit value prints in: black, indigo, pale blue, cream, or vermilion for a warm paint. */
// Every edge here is antialiased by about a pixel and no more (`soft` is tiny): a wider minimum turned a slowly
// changing value (n.l across a grazing flank, a soft paint gradient) into a visible gradient between two inks, which
// the palette test caught on the shell.
function inked(base: Expr<"vec3">, value: Expr<"f32">): Expr<"vec3"> {
  let c: Expr<"vec3"> = rgb(P.black);
  c = mix(c, rgb(P.indigo), edgeOf(value, BAND.black, 1e-5));
  c = mix(c, rgb(P.pale), edgeOf(value, BAND.indigo, 1e-5));
  c = mix(c, rgb(P.cream), edgeOf(value, BAND.pale, 1e-5));
  const warmth = min(sub(base.x, base.y), sub(base.x, base.z));
  return mix(c, rgb(P.vermilion), edgeOf(warmth, BAND.warm, 1e-5));
}

export function woodblock(): Style {
  const outline = param("woodblock.outline", 0.05, { min: 0, max: 0.2 });
  return {
    name: "woodblock",
    // The key line: the hull, this wide in world units and never under 1.5 px.
    outline,
    clear: P.cream,
    post: [WOODBLOCK_POST.paper],
    shade: (ctx) => {
      // Two tones of light (lit, shadow), then the value bands: each paint region prints in at most two inks.
      const lit = edgeOf(dot(ctx.normal, ctx.sun), 0.0, 1e-5);
      const value = mul(luminance(ctx.base), mix(0.66, 1.06, lit));
      return inked(ctx.base, value);
    },
    ink: () => rgb(P.black),
    // No post pass reads a mark.
    mark: () => f(1),
    water: (ctx) => {
      const xz = ctx.world.xz;
      // Two flat blues, trough and crest, split along the swell's own contour (a little wobble, no blobs: the lines
      // must follow the swell, not a noise field).
      const wobble = mul(noise2(mul(xz, 0.6)), 0.08);
      const h = add(ctx.crest, wobble);
      let c: Expr<"vec3"> = mix(rgb(SEA.deep), rgb(SEA.mid), edgeOf(h, 0.2, 0.01));
      // Printed lines along the swell: four per wavelength of the largest swell, bent by the whole height field (so
      // the smaller swells ripple them), heavier toward the crests and absent in the troughs, fading out where they
      // would alias.
      const phase = add(mul(ctx.phase, 5), mul(h, 0.7));
      const fw = max(fwidth(phase), 1e-4);
      const d = abs(sub(fract(add(phase, 0.5)), 0.5)); // 0 on a line, 0.5 halfway between two
      const width = mix(0.025, 0.075, saturate(add(mul(ctx.crest, 0.6), 0.5)));
      // Where the lines crowd (toward the horizon, or seen along the swell) each is replaced by the share of the period
      // it covers: the same tone with no stripes. It used to keep drawing them until three pixels apart, which printed
      // the far sea as a comb.
      const crowd = smoothstep(0.03, 0.1, fw);
      const drawn = sub(1, smoothstep(sub(width, fw), add(width, fw), d));
      const line = mul(mix(drawn, mul(width, 2), crowd), smoothstep(-0.6, -0.1, h));
      c = mix(c, rgb(SEA.line), mul(line, 0.9));
      // Clawed foam, from how close a body is: a collar at the body, and fingers that taper outward along the zero
      // lines of a noise field (thin where |noise| is small, narrower the farther out), with an indigo line round the
      // whole shape and loose specks of spray beyond it.
      const near = ctx.contact;
      const ridge = abs(noise2(mul(xz, 3.4)));
      const shape = max(sub(near, 0.6), sub(mul(near, 0.3), ridge));
      const foam = mul(edgeOf(shape, 0, 0.004), edgeOf(near, 0.02, 0.01));
      const rim = mul(mul(edgeOf(shape, -0.06, 0.004), sub(1, foam)), edgeOf(near, 0.05, 0.01));
      const speck = mul(specks(mul(xz, 8)), mul(smoothstep(-0.4, -0.1, near), sub(1, smoothstep(0.1, 0.3, near))));
      c = mix(c, rgb(P.indigo), rim);
      c = mix(c, rgb(P.cream), max(foam, speck));
      // The far sea fades toward the haze at the horizon as a bokashi (a graded wiping of the block, as the sky's): two
      // flat steps at 70 and 140 units printed hard pale bands across the far sea, the second straight across a
      // maelstrom's view (examples/maelstrom.ts --style woodblock).
      const far = mul(smoothstep(50, 220, ctx.distance), 0.85);
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
