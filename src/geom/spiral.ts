// Shells, horns, claws, teeth and tusks as one family: a cross-section growing along an equiangular (logarithmic)
// spiral, every length scaling together, so the form is self-similar (Raup 1966). A shell coils its section round an
// axis with Raup's parameters (W the growth per whorl, D the inner-to-outer distance ratio, T the translation along
// the axis); a horn is the same law in one plane, a tube whose radius grows with the spiral, posed by the names an art
// director uses (length, curl, taper). Both are closed form.

import { abs, add, cos, exp, fract, lift, log, lt, mul, pow, select, sin, sub, div, sqrt, max, TAU, vec3, type In } from "../expr.ts";
import { surface } from "../map/surface.ts";
import type { F, P3, Surface } from "../map/types.ts";
import { strand, type PaintSpec, type Strand } from "./strand.ts";

export type ShellSpec = {
  /** Whorls from the apex (u = 0) to the aperture (u = 1). */
  readonly turns: number;
  /** Raup's W: how much every length grows in one whorl (above 1). */
  readonly growth: number;
  /** Radius of the aperture across the coil (world units at u = 1). */
  readonly aperture: number;
  /** Raup's D: the section's inner distance from the axis over its outer distance, 0 (touching the axis) to below 1. */
  readonly umbilicus: number;
  /** Raup's T: drop of the section along the axis per unit of its distance from the axis. 0 coils flat (a nautilus). */
  readonly spire: number;
  /** Section height over width. Default 1. */
  readonly aspect?: number;
  /** Coiling seen from the apex: "right" (dextral) or "left". Default right. */
  readonly coil?: "right" | "left";
  /** Growth ridges folded into the section: a whole number per whorl (so self-similarity is exact), and their height as a share of the section. */
  readonly ridges?: { readonly perTurn: number; readonly height: number };
};

/**
 * The shell's surface, apex at the origin and spire along +y, aperture toward -y: (u, v) with u from apex to aperture
 * and v once round the section (0 the outer side). Every point at u + 1 / turns is `growth` times the point at u.
 */
export function shell(spec: ShellSpec): Surface {
  const { turns, growth, aperture, umbilicus: D, spire } = spec;
  if (!(growth > 1)) throw new Error(`shell: growth must be above 1 (every whorl larger than the last), got ${growth}`);
  if (!(D >= 0 && D < 1)) throw new Error(`shell: umbilicus must be in [0, 1), got ${D}`);
  if (!(turns > 0)) throw new Error(`shell: turns must be positive, got ${turns}`);
  const ridges = spec.ridges ?? { perTurn: 1, height: 0 };
  if (!Number.isInteger(ridges.perTurn) || ridges.perTurn < 1) throw new Error(`shell: ridges.perTurn must be a positive whole number, got ${ridges.perTurn}`);
  const hand = (spec.coil ?? "right") === "right" ? 1 : -1;
  const a = aperture, b = aperture * (spec.aspect ?? 1);
  const centre = (a * (1 + D)) / (1 - D);
  const lnW = Math.log(growth);
  const point = (u: F, v: F): P3 => {
    const scale = exp(mul(sub(u, 1), lnW * turns));
    const theta = mul(u, TAU * turns * hand);
    // The section turns with v in the sense that makes d/du x d/dv face out of the shell for either hand.
    const phi = mul(v, -TAU * hand);
    // A ridge is a sharp crest once per 1 / (turns perTurn) of u: the section swells there.
    const rib = add(1, mul(pow(abs(sin(mul(u, Math.PI * turns * ridges.perTurn))), 6), ridges.height));
    const r = add(centre, mul(cos(phi), mul(rib, a)));
    const y = add(-spire * centre, mul(sin(phi), mul(rib, b)));
    return mul(vec3(mul(r, cos(theta)), y, mul(r, sin(theta))), scale);
  };
  return surface(point, { u: ["cap", "free"], v: "periodic" });
}

/** Growth-line coordinate of a shell point: whorls from the apex, for painting bands and ridges in its own coordinates. */
export const whorl = (u: F, turns: number): F => mul(u, turns);
/** 0..1 within each of `perTurn` ridges per whorl, for paint that follows the folded ridges. */
export const ridgePhase = (u: F, turns: number, perTurn: number): F => fract(mul(u, turns * perTurn));

export type SpiralTubeSpec = {
  readonly name: string;
  /** Length along the centre line. */
  readonly length: In<"f32">;
  /** Total turn of the centre line from root to tip, radians (0 straight, a tusk; 2 a ram's sweep). */
  readonly curl: In<"f32">;
  /** Tip radius over root radius (the spiral's growth over the horn, so the curl tightens as it tapers). */
  readonly taper: In<"f32">;
  /** Radius at the root. */
  readonly girth: In<"f32">;
  /** Direction the horn leaves its root along, and the direction it curls toward (made perpendicular to `out`); in its local frame. */
  readonly out: V3;
  readonly toward: V3;
  /** Sideways drift (out of the curl's plane) as a share of length: a corkscrew. Default 0. */
  readonly twist?: In<"f32">;
  readonly paint: PaintSpec;
  readonly detail?: { readonly along?: number; readonly around?: number };
};

/**
 * A tapered tube on an equiangular (logarithmic) spiral. Named for its shape, not for a use: a horn, a claw's finger,
 * a tooth, a tusk, a tendril and a moustache have all been drawn with it, which is also a warning: reach for it by
 * habit and everything pointed in your picture will be the same curve.
 *
 * The shape. The centre line turns through `curl` radians in the plane of `out` and `toward`, and the radius goes from
 * `girth` at the root to `girth * taper` at the tip. The two are tied by one law: with e^(k theta) = 1 + m (taper - 1),
 * arc length is linear in m (0 at the root, 1 at the tip) and the radius is girth * e^(k theta), the spiral's own growth.
 * So the form is self-similar, as a shell's is: it tightens exactly as it thins.
 *
 * Parameters: `length`, `curl`, `taper`, `girth`, `twist` take numbers, expressions or host inputs (so they can move
 * in time); `out` and `toward` are fixed directions.
 *
 * What it cannot do, because of that one law:
 * - curl that does not follow the taper: a thick tight coil, a thin lazy one, a curl that reverses (an S), a straight
 *   run and then a hook. With `taper` 1 it is a plain circular arc.
 * - leave its plane properly: `twist` is a sideways drift growing as m^2, not a helix of constant pitch.
 * - a radius that is not linear along its length: no swelling club, no waist, no ridges.
 * - any section but round, and any ends but an open root and a point.
 *
 * Going past it is one step down, not a rewrite: `spiralLine` below gives this tube's centre line, radius and up
 * direction as `strand` takes them, so `strand({ name, ...spiralLine(o), section, tips, girth, paint })` changes the
 * section, the ends or the radius and keeps the curve. For another curve, write your own line with the same shape of
 * result ({ spine, girth, up }). The recipe behind this one: choose the heading along the line, theta(m), and the
 * spine is length times the integral of (cos theta, sin theta). Relatives worth trying:
 * - theta linear in m: a circular arc; add a constant drift along `up` and it is a helix (a corkscrew, a spring).
 * - theta growing as m^2 (a clothoid): straight at the root, curling faster and faster: a fern head, a tendril, a
 *   scorpion's tail. No closed form: sample it once with `tabulate`, or lay it through points with `through`.
 * - radius from the axis growing by a fixed step per turn (Archimedes' spiral): turns evenly spaced, a rope coil or a
 *   watch spring, where this one's turns crowd toward the tip.
 * - theta = curl * sin(pi m) or any curve of your own: S-bends and hooks.
 * A strand, so it has a skeleton, casts a shadow and carries body coordinates (m along, v round) for paint.
 */
export function spiralTube(spec: SpiralTubeSpec): Strand {
  return strand({
    name: spec.name, stations: 12, ...spiralLine(spec),
    tips: ["open", { point: 0.12 }],
    paint: spec.paint,
    detail: { along: spec.detail?.along ?? 24, around: spec.detail?.around ?? 12 },
  });
}

/**
 * `spiralTube`'s centre line, its radius along it and its up direction, named as `strand` takes them, so the tube is
 * never a dead end (see `spiralTube` for the law, its limits and its relatives). Spread it into your own `strand` and
 * override what you need; or add to its `spine` (a kink, a wobble in time) or multiply its `girth` (a swelling, ridges):
 * both are functions of m, the share of the way from root (0) to tip (1).
 */
export function spiralLine(spec: Pick<SpiralTubeSpec, "length" | "curl" | "taper" | "girth" | "out" | "toward" | "twist">): { readonly spine: (m: F) => P3; readonly girth: (m: F) => F; readonly up: P3 } {
  const L = lift<"f32">(spec.length), curl = lift<"f32">(spec.curl), taper = lift<"f32">(spec.taper), girth = lift<"f32">(spec.girth);
  const [o, w, sd] = basis(spec.out, spec.toward);
  const out = lift<"vec3">(o), toward = lift<"vec3">(w), side = lift<"vec3">(sd);
  const k = div(log(taper), max(curl, 1e-4));
  // Near taper 1 the spiral is a circle (k -> 0): use the limits, so the slider can pass through 1.
  const flat = lt(abs(k), 1e-4);
  const kk = select(flat, 1e-4, k);
  const q = sqrt(add(1, mul(kk, kk)));
  const rho0 = select(flat, div(L, max(curl, 1e-4)), div(mul(L, kk), mul(q, sub(taper, 1))));
  const spine = (m: F): P3 => {
    const g = add(1, mul(m, sub(taper, 1)));
    const theta = select(flat, mul(m, curl), div(log(g), kk));
    // rho0 (k - i) / q (e^(k theta) e^(i theta) - 1): real part along `out`, imaginary toward `toward`.
    const re = sub(mul(g, cos(theta)), 1), im = mul(g, sin(theta));
    const along = div(mul(rho0, add(mul(kk, re), im)), q);
    const across = div(mul(rho0, sub(mul(kk, im), re)), q);
    return add(add(mul(out, along), mul(toward, across)), mul(side, mul(mul(L, lift<"f32">(spec.twist ?? 0)), mul(m, m))));
  };
  return { spine, girth: (m) => mul(girth, add(1, mul(m, sub(taper, 1)))), up: side };
}

type V3 = readonly [number, number, number];

/** Unit `out`, `toward` made perpendicular to it, and their cross product. */
function basis(out: V3, toward: V3): [V3, V3, V3] {
  const unitOf = (v: readonly number[], what: string): V3 => {
    const l = Math.hypot(v[0], v[1], v[2]);
    if (!(l > 1e-6)) throw new Error(`spiralTube: ${what} must be a non-zero direction not parallel to out`);
    return [v[0] / l, v[1] / l, v[2] / l];
  };
  const o = unitOf(out, "out");
  const d = o[0] * toward[0] + o[1] * toward[1] + o[2] * toward[2];
  const w = unitOf([toward[0] - d * o[0], toward[1] - d * o[1], toward[2] - d * o[2]], "toward");
  return [o, w, [o[1] * w[2] - o[2] * w[1], o[2] * w[0] - o[0] * w[2], o[0] * w[1] - o[1] * w[0]]];
}
