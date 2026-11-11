// Spires: instead of the atoll's flat mesa, three leaning stone needles on a low moss cairn. A tall, broken silhouette
// against the base's flat one; it rides the same shell frame.

import { abs, fract, mix, mul, place, rigid, sin, strand, sub, vec3 } from "../../../../src/index.ts";
import { cut } from "../ink.ts";
import type { BackOption } from "../slots.ts";

export const spires: BackOption = {
  about: "three leaning stone spires on a low moss cairn",
  draw: ({ n, shell }) => {
    const I = n.ink, [cx, cz] = n.atoll.centre;
    const cairn = place(strand({
      name: "cairn", spine: (m) => mix(vec3(cx - 3.6, 0.7, cz), vec3(cx + 2.6, 0.9, cz), m), stations: 6, girth: 6.4,
      section: { aspect: 0.42, squareness: 1.6 }, tips: [{ round: 0.5 }, { round: 0.5 }], up: [0, 1, 0],
      paint: (m, v) => mix(vec3(...I.moss), vec3(...I.mossDeep), cut(mul(sin(mul(m, 31)), sin(mul(v, 37))), 0.4)),
      detail: { along: 24, around: 32 },
    }), rigid(shell));
    const spire = (k: number, x: number, z: number, h: number, r: number, lean: number) => place(strand({
      name: `spire${k}`, spine: (m) => mix(vec3(cx + x, 0.6, cz + z), vec3(cx + x + lean, 1.5 + h, cz + z - lean * 0.5), m), stations: 6,
      girth: (m) => mul(r, sub(1, mul(m, 0.8))), section: { squareness: 3 }, tips: [{ round: 0.1 }, { point: 0.25 }], up: [1, 0, 0],
      // Stone with dark courses, a black seam down one face.
      paint: (m, v) => mix(mix(vec3(...I.stone), vec3(...I.bark), sub(1, cut(abs(sub(fract(mul(m, 8)), 0.5)), 0.06))), vec3(...I.black), sub(1, cut(abs(sub(v, 0.25)), 0.02))),
      detail: { along: 32, around: 16 },
    }), rigid(shell));
    return [cairn, spire(0, -0.5, 0.6, 5.2, 1.4, 0.6), spire(1, -2.8, -1.8, 3.6, 1.05, -0.5), spire(2, -1.8, 2.6, 2.6, 0.8, 0.4)];
  },
};
