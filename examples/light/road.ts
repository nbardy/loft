// A road of lamps, one every 4 units, each behind a screen cut in six petals: one function lights the whole road, by
// the same `floor` repetition a field uses to repeat a shape. Only the nearest lamp lights a point, so each pool fades
// before the midpoint (`seam`); without it a body half way would see its light jump from one side to the other.
import { abs, add, atan2, cos, floor, lamp, mul, plane, scene, smoothstep, strand, sub, vec3, woodblock, type Light } from "../../src/index.ts";

const SPACING = 4;
const road: Light = (p) => {
  const x = mul(floor(add(mul(p.x, 1 / SPACING), 0.5)), SPACING); // the nearest lamp's x
  const near = lamp(vec3(x, 3, 0), { colour: [1.4, 1.1, 0.7], reach: 1.5 })(p);
  const out = mul(near.toward, -1); // from the lamp toward p
  const petals = smoothstep(0.3, 0.5, abs(cos(mul(atan2(out.z, out.x), 3)))); // six openings round the lamp
  const seam = smoothstep(SPACING / 2, SPACING / 2 - 0.4, abs(sub(p.x, x))); // fade before the next lamp takes over
  return { ...near, amount: mul(mul(near.amount, petals), seam) };
};

/** A post under every lamp, and a walker's two legs between two of them. */
const posts = [-8, -4, 0, 4, 8].map((x) => strand({ name: `post${x < 0 ? "m" : ""}${Math.abs(x)}`, spine: (m) => vec3(x, mul(m, 2.9), -0.4), girth: 0.06, tips: "round", paint: [0.3, 0.25, 0.2] }));
const figure = strand({ name: "figure", spine: (m) => vec3(2, add(0.1, mul(m, 1.6)), 0.6), girth: (m) => add(0.12, mul(m, 0.1)), tips: "round", paint: [0.7, 0.3, 0.2] });

export default scene({
  ground: plane,
  things: [...posts, figure],
  style: woodblock(),
  sun: { toward: [0.2, 0.6, 0.7], amount: [0.1, 0.1, 0.2] },
  lights: [road],
  cameras: {
    along: { kind: "perspective", eye: [-3, 6, 11], target: [1, 0, 0], fovY: 0.75 },
    above: { kind: "perspective", eye: [0.5, 14, 0.5], target: [0, 0, 0], fovY: 0.9 },
  },
});
