// A red lacquered bar on a floor, wet across its middle where a cloth has just passed. "Wet" is not a word any style
// knows: it is darker and glossier, said in the two terms every style that cares about gloss understands (`base` and
// `gloss`, 0 matte to 1 mirror). In toon the bar mirrors toon's sky, blurred on the lacquer and sharp on the stripe.
import { abs, mix, mul, plane, scene, smoothstep, strand, sub, toon, vec3, type F } from "../../src/index.ts";

const LACQUER = vec3(0.55, 0.06, 0.05);
const bar = strand({
  name: "bar",
  spine: (m: F) => vec3(mul(sub(m, 0.5), 3), 0.6, 0),
  girth: 0.6,
  section: { aspect: 1, squareness: 8 }, // a rounded square: a box
  tips: { round: 0.08 },
  paint: (m: F) => {
    const wet = sub(1, smoothstep(0.08, 0.1, abs(sub(m, 0.5)))); // 1 on the middle fifth of the length
    return { base: mix(LACQUER, mul(LACQUER, 0.55), wet), gloss: mix(0.75, 0.97, wet) };
  },
});

export default scene({
  ground: plane,
  things: [bar],
  style: toon(),
  sun: [-0.4, 0.8, 0.45],
  cameras: {
    whole: { kind: "perspective", eye: [4, 3, 5], target: [0, 0.6, 0], fovY: 0.6 },
    stripe: { kind: "perspective", eye: [1.2, 1.6, 2], target: [0, 0.6, 0], fovY: 0.5 },
  },
});
