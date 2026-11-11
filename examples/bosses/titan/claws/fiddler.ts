// A fiddler: the crusher three times the cutter, its palm cream with black bands. The rig places the ship by this
// claw's own grip depth (its offer), and the claw asks the rig for one number: a bigger hit disc.
// - Claw Crush lands the palm centred on the disc, so the palm's footprint grows with the crusher: the base disc (3.4)
//   fits the base crusher (1.4, 0.0% outside) and not much more. Measured with this claw (`loft check`): a 2.1 crusher
//   is 3.5% outside a 5.1 disc and 0.6% outside 5.4; this 2.4 crusher is 1.5% outside 6.2 (the limit is 2%). A bigger
//   claw hits a bigger area: the disc is the game's hit shape, so this is a gameplay change the variant states.
// - The cutter keeps the base grab's reach. Measured: a 0.8 cutter closes on the beam and keeps it (middle 0.058 off);
//   a 0.7 cutter closes (0.095 off) but its arm cannot follow the ship as the hold lifts it (0.199 off at t = 11.3).
//   Moving the wrist nearer (grabWristReach 3.2) made the 0.7 cutter worse (0.192 off at the close), not better.

import { fract, mix, mul, sub, vec3 } from "../../../../src/index.ts";
import { cut } from "../ink.ts";
import { clawsOf } from "./base.ts";

export const fiddler = clawsOf({
  about: "a fiddler: crusher three times the cutter, palms banded cream and black, a bigger crush disc",
  numbers: { crusher: 2.4, cutter: 0.8, crushRadius: 6.2 },
  palm: (I, m) => mix(vec3(...I.cream), vec3(...I.black), cut(sub(0.5, fract(mul(m, 5))), 0.25)),
});
