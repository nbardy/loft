// Decals derived from the skeleton (src/compile/decals.ts), checked on bodies whose waterline and shadow are known.
import { expect, test } from "bun:test";
import { compile, scene, sea, strand, through, toon, type Artefact } from "../src/index.ts";

const flat = sea({ swell: [] });
const tube = (name: string, a: [number, number, number], b: [number, number, number]) =>
  strand({ name, spine: (m) => through(m, [a, b]), stations: 16, girth: 0.5, tips: "open", paint: [1, 1, 1] });
const poseOf = (a: Artefact) => a.pose({ time: 0, inputs: new Float32Array(Math.max(1, a.inputFloats)), params: Float32Array.from(a.params.map((p) => p.def)) });
const entries = (t: Float32Array) => Array.from({ length: t[0] }, (_, i) => Array.from(t.subarray(4 + i * 8, 12 + i * 8)));

// Catches: waterline cuts computed from the wrong axes or radii (foam inside or beside the body instead of on its
// outline). A round tube lying half-submerged along x cuts the water across its full width at every station.
test("a tube lying awash gets collars across its full width, on its axis", () => {
  const a = compile(scene({ sea: flat, things: [tube("awash", [-4, 0, 0], [4, 0, 0])], style: toon(), sun: [0, 1, 0] }));
  const c = entries(poseOf(a).collars);
  expect(c.length).toBeGreaterThan(10);
  for (const [, cz, , across] of c) {
    expect(Math.abs(cz)).toBeLessThan(1e-4);
    expect(across).toBeCloseTo(0.5, 4);
  }
});

// Catches: missing foam where a neck rises out of the water (a ring parallel to the sea never cuts it), or a collar
// of the wrong size there. A vertical tube cuts the plane in a circle of its radius, centred on its axis.
test("a tube piercing the water vertically gets one round collar at the crossing", () => {
  const a = compile(scene({ sea: flat, things: [tube("pillar", [5, -2, 1], [5, 2.1, 1])], style: toon(), sun: [0, 1, 0] }));
  const c = entries(poseOf(a).collars);
  expect(c.length).toBe(1);
  const [cx, cz, along, across] = c[0];
  expect(cx).toBeCloseTo(5, 4);
  expect(cz).toBeCloseTo(1, 4);
  expect(along).toBeCloseTo(0.5, 4);
  expect(across).toBeCloseTo(0.5, 4);
});

// Catches: shadows cast along the wrong direction (toward the sun, or ignoring height). A bar at height 2 under a sun
// tilted toward +x by 0.5 per unit up throws its shadow 1 unit toward -x.
test("a raised bar casts its shadow along the sun onto the water", () => {
  const a = compile(scene({ sea: flat, things: [tube("bar", [-3, 2, 0.5], [3, 2, 0.5])], style: toon(), sun: [0.5, 1, 0] }));
  const s = entries(poseOf(a).shadows);
  expect(s.length).toBe(15);
  for (const [ax, az, bx, bz, ra, rb, strength] of s) {
    expect(az).toBeCloseTo(0.5, 4);
    expect(bz).toBeCloseTo(0.5, 4);
    expect(ax).toBeGreaterThanOrEqual(-3 - 1 - 1e-4);
    expect(bx).toBeLessThanOrEqual(3 - 1 + 1e-4);
    expect(ra).toBeCloseTo(0.5, 4);
    expect(rb).toBeCloseTo(0.5, 4);
    expect(strength).toBeGreaterThan(0);
  }
  expect(s[0][0]).toBeCloseTo(-4, 4);
  expect(s[s.length - 1][2]).toBeCloseTo(2, 4);
});
