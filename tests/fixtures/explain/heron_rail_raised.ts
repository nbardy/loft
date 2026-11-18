// The heron's leap (examples/second/heron_leap.ts), solved for its rail, then checked against the rail raised 0.2 m:
// the same motion, the same holds, a rail it was not solved for. Some clearance holds fail, and
// `loft check tests/fixtures/explain/heron_rail_raised.ts --explain` says which line, when, and how to see it.
import { always, Expr, mix, plane, scene, strand, time, toon, vec3 } from "../../../src/index.ts";
import { holds, leap, NUMBERS } from "../../../examples/second/heron_leap.ts";

const raised = { ...NUMBERS, rail: { ...NUMBERS.rail, y: NUMBERS.rail.y + 0.2 } };
const { solved, bird } = leap(NUMBERS);

const steel = [0.35, 0.37, 0.4] as const;
const bar = (name: string, a: readonly [number, number, number], b: readonly [number, number, number], r: number) =>
  strand({ name, spine: (m) => mix(vec3(...a), vec3(...b), m), stations: 4, girth: r, tips: ["round", "round"], paint: steel, detail: { along: 12, around: 12 } });
const { x, y, r } = raised.rail;

export default scene({
  ground: plane, style: toon({ bands: 3, outline: 0.03 }), sun: [-0.4, 0.8, 0.45], period: 2.4,
  things: [...bird.things, bar("rail", [x, y, -1.4], [x, y, 1.4], r), bar("postL", [x, 0, -1.4], [x, y, -1.4], r), bar("postR", [x, 0, 1.4], [x, y, 1.4], r)],
  cameras: { side: { kind: "perspective", eye: [0.5, 2.6, 10], target: [2.2, 1.4, 0], fovY: 0.62 } },
});

// A bare hold is meant over the whole span, as `solve` reads it.
export const checks = holds(raised, solved.curves).map((h) => (h instanceof Expr ? always([time, 0, 1.8], h) : h));
