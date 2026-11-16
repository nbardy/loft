// march (src/draw/march.ts) on the CPU against a sphere's closed form. Catches: `iterate`'s `until` ending the loop
// on the wrong state (a hit recorded one step late or never), and a near miss that reports the gap of the last sample
// instead of the least one (which would put a reflection's soft edge in the wrong place).
import { expect, test } from "bun:test";
import { bound, compileJs, f, length, march, sub, variable, vec3, type P3 } from "../src/index.ts";

const C: [number, number, number] = [0, 1, 0];
const sphere = (p: P3) => sub(length(sub(p, vec3(...C))), 1);
const hitOf = march(sphere, { within: bound.sphere(C, 1.2), steps: 64 });
const O = variable("o", "vec3"), D = variable("d", "vec3");
const h = hitOf({ o: O, d: D, t0: f(0), t1: f(50), width: () => f(1e-3) });
const run = compileJs([h.t, h.gap]);
const cast = (o: number[], d: number[]) => run({ vars: { o, d }, inputs: {}, params: {} }) as number[];

test("march hits a sphere where its closed form does, and a miss reports the least clearance", () => {
  // Straight at the centre from 4 away: the surface is 3 away.
  const [t, gap] = cast([0, 1, 4], [0, 0, -1]);
  expect(gap).toBe(0);
  expect(Math.abs(t! - 3)).toBeLessThan(1e-4);
  // Passing 1.1 from the centre: misses by 0.1, nearest at t = 4 (beside the centre). The gap is the closest the
  // surface can come between two samples (where their spheres of free space meet), so at most the true 0.1; measured
  // 0.0879 with these steps. The last sample alone would say 0.10 or more.
  const [tm, gm] = cast([1.1, 1, 4], [0, 0, -1]);
  expect(gm!).toBeGreaterThan(0.08);
  expect(gm!).toBeLessThanOrEqual(0.1);
  expect(Math.abs(tm! - 4)).toBeLessThan(0.1);
});
