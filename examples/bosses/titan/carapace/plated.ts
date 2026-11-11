// A plated carapace: boxier in section, brick cut into bands of plates by black seams across its length, each plate
// with a cream leading edge. Same strand coordinates as the base (m tail to front), so the face's beard still rides it.

import { abs, fract, mix, mul, place, rigid, sin, strand, sub, through, vec3 } from "../../../../src/index.ts";
import { cut } from "../ink.ts";
import type { CarapaceOption } from "../slots.ts";

export const plated: CarapaceOption = {
  about: "boxy shell in banded plates with black seams",
  draw: ({ n, body }) => {
    const C = n.carapace, I = n.ink;
    const hull = place(strand({
      name: "titan", spine: (m) => through(m, [[C.from, -0.1, 0], [(C.from + C.to) / 2, 0.15, 0], [C.to, 0, 0]]), stations: 8,
      girth: C.width, section: { aspect: C.height / C.width, squareness: 4 }, tips: [{ round: 0.3 }, { round: 0.3 }], up: [0, 1, 0],
      paint: (m, v) => {
        const band = fract(mul(m, 7));
        const plate = mix(vec3(...I.brickDark), vec3(...I.cream), cut(sub(band, 0.86), 0));
        const shell = mix(vec3(...I.cream), plate, cut(sin(mul(v, Math.PI * 2)), -0.2));
        return mix(shell, vec3(...I.black), sub(1, cut(abs(sub(band, 0.5)), 0.46)));
      },
      detail: { along: 56, around: 40 },
    }), rigid(body));
    return { hull, things: [hull] };
  },
};
