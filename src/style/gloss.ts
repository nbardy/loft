// A body reflecting the scene's sky: four taps of the sky round the reflected view ray, mixed over the colour by a
// Fresnel weight.

import { abs, add, cross, dot, lift, lt, max, mix, mul, normalize, pow, saturate, select, sub, vec3, type Expr, type In } from "../expr.ts";
import type { ShadeCtx } from "../style.ts";

type V = Expr<"vec3">;

/** A constant 0: a channel no paint gave, read with its default. */
const zero = (x: In<"f32">): boolean => (typeof x === "number" ? x === 0 : x.node.op === "const" && x.node.v.every((c) => c === 0));

/**
 * The sky and sun mirrored in a body, blurred as gloss falls, Fresnel-weighted.
 *
 * The law: r = 2 (n.v) n - v. seen = the mean of sky(normalize(r +/- a t1)) and sky(normalize(r +/- a t2)), t1 and t2
 * unit and perpendicular to r, a = 0.02 + 0.5 alpha the cone's half-angle in radians, alpha = max(0.02, (1 - gloss)^2)
 * the roughness: the same lobe a light's highlight uses, so the sky's reflection and a lamp's glint widen together.
 * F = f0 + (1 - f0)(1 - n.v)^5 with f0 = mix(0.04, 1, metal) (Schlick). The result is
 * mix(colour, seen * mix(1, base, metal), mix(gloss, 1, metal) F): a dielectric reflects white and weakly face on, a
 * metal reflects in its own colour and fully. With gloss and metal the constant 0 the colour comes back unchanged and
 * nothing is built (a channel no paint gives costs nothing).
 *
 * Parameters: gloss and metal are expressions, 0 to 1 (a style passes ctx.channel("gloss", 0)); `seen` is what is
 * reflected along a direction, the style's sky by default (`ctx.sky`).
 *
 * Cannot: reflect the ground as itself (a ray below the horizon sees the sky's own colour below the horizon), other
 * bodies, or the body itself (a cavity reflects open sky); a blur wider than four taps can hold (near gloss 0 the four
 * taps show as faint ghosts of a sharp sky feature); antialias a sharp feature reflected by a curved body near gloss 1
 * (a sun disc sparkles: keep gloss under 0.95, or give the sky no sharp feature). It runs on the CPU: no derivative.
 *
 * Going past it: pass `seen` (a tracer, a second sky, a painted studio of softboxes as a function of direction); take
 * more taps for a wider blur; colour the Fresnel term per wavelength (a thin film, as a `scatter` does); an anisotropic
 * cone stretched along the body's own m (brushed metal). Relatives: a reflection quantised to two inks (a print of
 * chrome); a reflection of the sky's gradient only, without its features.
 */
export function glossed(colour: V, ctx: ShadeCtx, o: { readonly gloss: In<"f32">; readonly metal?: In<"f32">; readonly seen?: (dir: V) => V }): V {
  if (zero(o.gloss) && zero(o.metal ?? 0)) return colour;
  const gloss = lift<"f32">(o.gloss), metal = lift<"f32">(o.metal ?? 0), seen = o.seen ?? ((d: V) => ctx.sky(d));
  const r = sub(mul(ctx.normal, mul(dot(ctx.normal, ctx.view), 2)), ctx.view);
  const side = select(lt(abs(r.y), 0.9), vec3(0, 1, 0), vec3(1, 0, 0));
  const t1 = normalize(cross(r, side)), t2 = cross(r, t1);
  const rough = sub(1, gloss);
  const a = add(0.02, mul(max(mul(rough, rough), 0.02), 0.5));
  const tap = (t: V, s: number) => seen(normalize(add(r, mul(t, mul(a, s)))));
  const sky = mul(add(add(tap(t1, 1), tap(t1, -1)), add(tap(t2, 1), tap(t2, -1))), 0.25);
  const f0 = mix(0.04, 1, metal);
  const fresnel = add(f0, mul(sub(1, f0), pow(sub(1, saturate(dot(ctx.normal, ctx.view))), 5)));
  return mix(colour, mul(sky, mix(vec3(1, 1, 1), ctx.base, metal)), mul(mix(gloss, 1, metal), fresnel));
}
