// Lines printed along the swell: what a cut or printed sea follows.
//
// The law: the largest swell's phase (`WaterCtx.phase`, one cycle a wavelength) times `perWave`, bent by the height
// field (h = crest + a little noise, times 0.7), gives lines of constant phase that run along the crests and ripple with
// the smaller swells. A line's half width goes from `width[0]` of its spacing in the troughs to `width[1]` at the
// crests, and none is printed in the deep troughs (h under -0.6, fading in by -0.1). Where lines crowd on screen
// (toward the horizon, or seen along the swell; their phase changing by more than 0.03 to 0.1 a pixel) each is replaced
// by the share of its period it covers: the same tone, no stripes, no comb.
//
// Parameters: `perWave` and `width` are numbers fixed when the style is built; everything else follows the sea.
//
// Cannot: lines across the swell (rays from a body, a current), a sea without swell (phase is 0 everywhere: no lines),
// lines that end (each is a whole contour; only the trough fade breaks it), or a width that changes along a line.
//
// Going past it: the returned `swell` (h) and `off` are the raw material: contours of h itself close into rings (a
// topographic sea); `fract` of the phase read through a noise gives broken, hand-cut lines; a second set at a different
// `perWave` laid at half weight gives the fine lines between the main ones; `off` under a threshold that a body's
// contact level raises gives lines that thicken toward the body (the swell piling up).

import { abs, add, fract, lift, max, mix, mul, saturate, smoothstep, sub, type Expr } from "../expr.ts";
import { noise2 } from "../lib/index.ts";
import { fwidth } from "../lib/screen.ts";
import type { WaterCtx } from "../style.ts";

/**
 * The swell's printed lines here. `cover`: how much of the pixel a line covers, 0..1. `off`: 0 on a line to 0.5 half way
 * between two. `live`: how much of a line prints here (none in the troughs). `swell`: the height the lines are bent by
 * (-1 trough to 1 crest, with a little wobble), for splitting the water into tones along the same contours. `calm`:
 * the sea has no swell, so there are no lines (cover 0, off 0.5); a style whose water is mostly line ink on a swell
 * chooses its calm tone from this (woodblock prints the line ink flat).
 */
export type SwellLines = { readonly cover: Expr<"f32">; readonly off: Expr<"f32">; readonly live: Expr<"f32">; readonly swell: Expr<"f32">; readonly calm: boolean };

/**
 * Whether the sea has no swell: its phase is the constant 0 (`WaterCtx.phase`), a fact of the scene known when the
 * style is built. A style that lays its water along the phase draws nothing from it there; anything it reads off the
 * phase alone is a noise artefact (sketch's labyrinth, woodblock's disc, papercraft's white blobs: friction S3/S4).
 */
export const calmSea = (ctx: WaterCtx): boolean => ctx.phase.node.op === "const";

/**
 * The printed lines of constant phase that follow the swell's crests, for a style that draws the water as lines.
 * Takes the water context, `perWave` (lines per wavelength) and `width` (half-width as a share of line spacing, in the
 * troughs and at the crests); returns the coverage and position of the lines (`SwellLines`).
 */
export function swellLines(ctx: WaterCtx, o: { readonly perWave: number; readonly width: readonly [number, number] }): SwellLines {
  if (!(o.perWave > 0)) throw new Error(`swellLines: perWave must be positive, got ${o.perWave}`);
  // A little wobble, no blobs: the lines must follow the swell, not a noise field.
  const wobble = mul(noise2(mul(ctx.world.xz, 0.6)), 0.08);
  const h = add(ctx.crest, wobble);
  // A sea without swell has a constant phase (0): no lines, as the header says. Friction S3/S4 (2026-11-16): the lines
  // were drawn anyway, and with the phase constant every point sat on line 0, bent off it only by the wobble, so the
  // noise's contours printed instead: sketch's grey labyrinth over the still sea, and woodblock's sea all in line ink
  // but for one noise peak, the "detached dark disc" in front of a body.
  if (calmSea(ctx)) return { cover: lift<"f32">(0), off: lift<"f32">(0.5), live: smoothstep(-0.6, -0.1, h), swell: h, calm: true };
  const phase = add(mul(ctx.phase, o.perWave), mul(h, 0.7));
  const fw = max(fwidth(phase), 1e-4);
  const off = abs(sub(fract(add(phase, 0.5)), 0.5));
  const width = mix(o.width[0], o.width[1], saturate(add(mul(ctx.crest, 0.6), 0.5)));
  // Where the lines crowd each is replaced by the share of the period it covers. Drawing them until three pixels apart
  // printed the far sea as a comb.
  const crowd = smoothstep(0.03, 0.1, fw);
  const drawn = sub(1, smoothstep(sub(width, fw), add(width, fw), off));
  const cover = mul(mix(drawn, mul(width, 2), crowd), smoothstep(-0.6, -0.1, h));
  // `live` is not their crowding: that is a screen derivative, and a foam's edge takes the derivative of whatever shapes
  // it (a derivative of a derivative, which the sea's cases refuse).
  return { cover, off, live: smoothstep(-0.6, -0.1, h), swell: h, calm: false };
}
