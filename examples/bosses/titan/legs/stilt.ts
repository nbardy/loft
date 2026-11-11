// Stilt legs: long thin segments that swell into knobs at the joints (the base pinches there), striped cream and black
// along their length. They need a higher body to stand in (`numbers`): the feet stay where the walk puts them.

import { abs, add, lift, limb, min, mix, mul, sin, smoothstep, strand, sub, vec3, type F } from "../../../../src/index.ts";
import { cut, sides } from "../ink.ts";
import type { LegsOption } from "../slots.ts";

export const stilt: LegsOption = {
  about: "long thin striped legs, knobbed at the joints, the body raised on them",
  numbers: { ride: 2.6, legLengths: [5.2, 4.9, 2.2] },
  draw: ({ n, walk, body, local }) => sides.flatMap((s, si) => n.legRoots.map((x, i) => {
    const leg = limb({ root: local([x, -0.1, s * n.carapace.width * 0.85]), target: walk.foot(si * 4 + i), lengths: n.legLengths, bend: add(body.y, mul(body.z, s * 0.4)), foot: mul(body.y, -1) });
    const nearKnot = (m: F): F => leg.knots.slice(1, 3).reduce((d: F, kn) => min(d, abs(sub(m, kn))), lift<"f32">(1));
    return strand({
      name: `leg${s < 0 ? "L" : "R"}${i}`, spine: leg.spine, stations: 32, tips: ["round", { point: 0.15 }], section: { aspect: 0.9 }, up: body.y,
      girth: (m) => add(mul(sub(1, mul(m, 0.5)), 0.32), mul(smoothstep(0.06, 0, nearKnot(m)), 0.3)),
      paint: (m) => mix(vec3(...n.ink.cream), vec3(...n.ink.black), cut(sin(mul(m, 70)), 0.55)),
      detail: { along: 80, around: 10 },
    });
  })),
};
