// Legs: three segments by inverse kinematics from a root on the body's side to the walk's foot, pinched and banded at
// the joints (a cream band before each, a black ring at it), cream dactyls to a black point.

import { abs, add, keys, lift, limb, max, min, mix, mul, sin, smoothstep, strand, sub, vec3, type F } from "../../../../src/index.ts";
import type { Reaching } from "../../../../src/check.ts";
import { cut, mottle, sides } from "../ink.ts";
import type { Numbers } from "../numbers.ts";
import type { Rig } from "../rig.ts";
import type { Legs, LegsOption } from "../slots.ts";

export const base: LegsOption = {
  about: "three tapering segments, pinched and banded at the joints",
  draw: (rig) => banded(rig, 4),
};

/** Pair i's three segment lengths: the legs' own, times the pair's `legSize.length` (exactly the legs' own at 1). */
export const pairLengths = (n: Numbers, i: number): readonly [number, number, number] => {
  const k = n.legSize.length[i]!;
  return [n.legLengths[0] * k, n.legLengths[1] * k, n.legLengths[2] * k];
};

/** The base legs for the first `pairs` of the rig's four feet a side (the rest stay in the rig's footing, undrawn). */
export function banded({ n, walk, body, local }: Rig, pairs: number): Legs {
    const I = n.ink;
    const segs = keys([[0, 1], [0.4, 0.85], [0.8, 0.5], [1, 0.18]]);
    const limbs: Reaching[] = [];
    const things = sides.flatMap((s, si) => n.legRoots.slice(0, pairs).map((x, i) => {
      const k = si * 4 + i;
      const root = local([x, -0.1, s * n.carapace.width * 0.85]);
      const spec = { root, target: walk.foot(k), lengths: pairLengths(n, i), bend: add(body.y, mul(body.z, s * 0.4)), foot: mul(body.y, -1) };
      const leg = limb(spec);
      limbs.push({ name: `leg${s < 0 ? "L" : "R"}${i}`, limb: spec, outward: mul(body.z, s) });
      const nearKnot = (m: F): F => leg.knots.slice(1, 3).reduce((d: F, kn) => min(d, abs(sub(m, kn))), lift<"f32">(1));
      const pinch = (m: F) => sub(1, mul(smoothstep(0.05, 0, nearKnot(m)), 0.3));
      return strand({
        name: `leg${s < 0 ? "L" : "R"}${i}`, spine: leg.spine, stations: 24, girth: (m) => mul(mul(segs(m).x, 0.5 * n.legSize.girth[i]!), pinch(m)), tips: ["round", { point: 0.1 }],
        section: { aspect: 0.8 }, up: body.y,
        paint: (m, v) => {
          const top = mix(vec3(...I.brick), vec3(...I.brickDark), mottle(mul(m, 0.7), v));
          const shell = mix(vec3(...I.cream), top, cut(sin(mul(v, Math.PI * 2)), -0.2));
          // A cream band before each joint and a black ring at it; the dactyl cream to a dark point.
          const band = sub(1, cut(nearKnot(m), 0.028));
          const ring = sub(1, cut(nearKnot(m), 0.012));
          const tip = cut(m, 0.84);
          return mix(mix(mix(shell, vec3(...I.cream), max(band, tip)), vec3(...I.black), ring), vec3(...I.black), cut(m, 0.98));
        },
        detail: { along: 64, around: 12 },
      });
    }));
    return { things, limbs };
}
