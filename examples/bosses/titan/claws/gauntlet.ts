// Gauntlet claws (the mask direction): the base pincer's mechanism (so its offer, the Grab and Claw Crush are the
// base claw's), dressed as samurai sleeve armour: the arms in overlapping lacquer lames edged in red, a cream cord
// binding at the elbow, palms of black lacquer plate with a red lacing band and a cream cord round the wrist, fingers
// red to a cream point.
//
// At rest the claws are held low and wide, palms barely folded in: the mask's mouth stays clear from the face camera
// (the base claws fold across the base face's mouth).

import { abs, add, fract, mix, mul, sin, sub, vec3, type F } from "../../../../src/index.ts";
import { cut, TAU } from "../ink.ts";
import { clawsOf } from "./base.ts";

/** Lames along an arm. */
const LAMES = 9;
const cord = (m: F, at: F | number, w: number): F => sub(1, cut(abs(sub(m, at)), w));

export const gauntlet = clawsOf({
  about: "lacquer gauntlets: laced plate palms, laminated arms, cord-bound, held low and wide",
  palm: (I, m, v) => {
    // Black lacquer plate, a red laced band across the middle, a cream cord at the wrist end, the underside red.
    const band = mul(cut(m, 0.42), sub(1, cut(m, 0.62)));
    const lace = sub(1, cut(abs(sin(add(mul(m, 40), mul(v, TAU * 5)))), 0.3));
    const plate = mix(mix(vec3(...I.lacquer), mix(vec3(...I.brick), vec3(...I.cream), mul(lace, 0.7)), band), vec3(...I.black), cord(m, 0.42, 0.01));
    const wrist = cord(m, 0.1, 0.05);
    const under = cut(mul(sin(mul(v, TAU)), -1), 0.55);
    return mix(mix(plate, vec3(...I.brick), under), vec3(...I.cream), wrist);
  },
  arm: (I, m, v, elbow) => {
    const p = fract(mul(m, LAMES));
    const lame = mix(mix(vec3(...I.lacquer), vec3(...I.brick), cut(p, 0.9)), vec3(...I.black), sub(1, cut(p, 0.05)));
    const twist = sub(1, cut(abs(sin(add(mul(m, 200), mul(v, TAU * 2)))), 0.35));
    const rope = mix(vec3(...I.cream), vec3(...I.bark), mul(twist, 0.8));
    return mix(lame, rope, cord(m, elbow, 0.04));
  },
  // Each lame flares 25% at its lower edge; pinched under the elbow's cord.
  armProfile: (m) => add(0.9, mul(fract(mul(m, LAMES)), 0.25)),
  finger: (I, m) => mix(mix(vec3(...I.brick), vec3(...I.cream), cut(m, 0.62)), vec3(...I.black), cut(m, 0.965)),
  rest: { wrist: (n, side, size) => [n.carapace.to + 0.9 * size, -0.35, side * (n.carapace.width * 0.62 + 0.9 * size)], fold: 0.15 },
});
