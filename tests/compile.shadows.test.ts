// Cast shadows derived from the skeleton (src/draw/shadows.ts), checked on a body whose shadow is known.
import { expect, test } from "bun:test";
import { compileScene, scene, sea, strand, through, toon } from "../src/index.ts";
import { stateAt } from "./compile.harness.ts";

// Catches: shadows cast along the wrong direction (toward the sun, or ignoring height), and broken chain links (the
// flags that let a capsule's patch yield its joint to its neighbour, so a joint is not darkened twice). A bar at height
// 2 under a sun tilted toward +x by 0.5 per unit up throws its shadow 1 unit toward -x.
test("a raised bar casts one linked chain of capsules along the sun onto the water", () => {
  const bar = strand({ name: "bar", spine: (m) => through(m, [[-3, 2, 0.5], [3, 2, 0.5]]), stations: 16, girth: 0.5, tips: "open", paint: [1, 1, 1] });
  const c = compileScene(scene({ sea: sea({ swell: [] }), things: [bar], style: toon(), sun: [0.5, 1, 0] }));
  const t = c.frame(stateAt(c.artefact, 0)).table(c.table("shadows"));
  const s = Array.from({ length: t[0] }, (_, i) => Array.from(t.subarray(4 + i * 12, 16 + i * 12)));
  expect(s.length).toBe(15);
  s.forEach(([ax, az, bx, bz, ra, rb, strength, , prev, next], i) => {
    expect(az).toBeCloseTo(0.5, 4);
    expect(bz).toBeCloseTo(0.5, 4);
    expect(ax).toBeGreaterThanOrEqual(-3 - 1 - 1e-4);
    expect(bx).toBeLessThanOrEqual(3 - 1 + 1e-4);
    expect(ra).toBeCloseTo(0.5, 4);
    expect(rb).toBeCloseTo(0.5, 4);
    expect(strength).toBeGreaterThan(0);
    expect([prev, next]).toEqual([i > 0 ? 1 : 0, i < 14 ? 1 : 0]);
  });
  expect(s[0][0]).toBeCloseTo(-4, 4);
  expect(s[14][2]).toBeCloseTo(2, 4);
});
