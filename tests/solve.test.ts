// Motion from constraints (src/geom/solve.ts) and rigid parts between keys.
import { expect, test } from "bun:test";
import { add, evaluate, mul, sub, substitute, time, vec3, length, type Expr } from "../src/expr.ts";
import { derivative } from "../src/grad.ts";
import { blendFrames, turnedAbout } from "../src/geom/placing.ts";
import { atLeast, equal, solve, spline, SolveError, TOLERANCE, type HoldLine } from "../src/geom/solve.ts";
import type { Frame } from "../src/map/types.ts";

const at = (e: Expr, t: number) => evaluate(e, { inputs: { time: t } }) as number;
const ROD = 2, TURN = Math.PI / 6;

// Catches: a pose between keys that blends a rigid part's points (or its axes without making them unit again) instead
// of turning it, which shortens a 2 m rod turning 30 degrees to 2 cos 15 = 1.932 at the midpoint. Two ways Loft turns a
// part between keys: frames blended (`blendFrames`), and a hinge angle that `solve` makes (a rod about its root).
test("a 2 m rod turning 30 degrees between keys keeps its length at the midpoint", () => {
  const hinge = vec3(0.3, 1, -0.2);
  const k0: Frame = { o: hinge, x: vec3(1, 0, 0), y: vec3(0, 1, 0), z: vec3(0, 0, 1) };
  const k1 = turnedAbout(k0, [0, 0, 1], TURN);
  const mid = blendFrames(k0, k1, 0.5);
  const tip = add(mid.o, mul(mid.x, ROD));
  expect(Math.abs((evaluate(length(sub(tip, mid.o))) as number) - ROD)).toBeLessThan(1e-4);

  const turn = solve({
    over: [0, 1], vary: { a: spline(6) },
    hold: ({ a }) => [equal(a, 0, { at: 0 }), equal(a, TURN, { at: 1 }), equal(derivative(a, time), 0, { at: 0 }), equal(derivative(a, time), 0, { at: 1 })],
    prefer: { jerk: 1 },
  });
  const a = turn.curves.a, rod = turnedAbout(k0, [0, 0, 1], a), end = add(rod.o, mul(rod.x, ROD));
  expect(Math.abs(at(length(sub(end, rod.o)), 0.5) - ROD)).toBeLessThan(1e-4);
  expect(Math.abs(at(a, 0.5) - TURN / 2)).toBeLessThan(1e-4); // symmetric keys: halfway at the midpoint
});

// Catches: a hold met by the solver's own evaluation but not by the expression returned. The flight's acceleration is
// held from a knot (0.6) on; the B-spline's second derivative read exactly at a knot came out 0 when the kernel used
// `abs` (its derivative at 0 is 0), and the returned numbers are f32 constants, not the solver's doubles.
test("holds at instants and on intervals are met by the returned curves, at knots too", () => {
  const r = solve({
    over: [0, 1.8], vary: { y: spline(15) },
    hold: ({ y }) => [
      equal(y, 1.9, { at: 0 }), equal(derivative(y, time), 0, { at: 0 }),
      equal(derivative(derivative(y, time), time), -9.81, { from: 0.6, to: 1.2 }),
      equal(y, 2.6, { at: 1.8 }), equal(derivative(y, time), 0, { at: 1.8 }),
      equal(y, 1.4, { at: 1.2 }),
    ],
    prefer: { jerk: 1 },
  });
  const acc = derivative(derivative(r.curves.y, time), time);
  for (const t of [0.6, 0.75, 0.9, 1.05, 1.2]) expect(Math.abs(at(acc, t) + 9.81)).toBeLessThan(TOLERANCE * 10);
  expect(Math.abs(at(r.curves.y, 1.2) - 1.4)).toBeLessThan(TOLERANCE);
  for (const h of r.report.holds) expect(h.worst).toBeLessThanOrEqual(TOLERANCE);
});

// Catches: an interval hold enforced only at its sample times, the curve dipping through it between them.
test("an interval hold holds between its samples", () => {
  const r = solve({
    over: [0, 1], vary: { y: spline(8) },
    hold: ({ y }) => [equal(y, 0, { at: 0.2 }), equal(y, 1, { at: 0.5 }), equal(y, 0, { at: 0.8 }), atLeast(y, 0)],
    prefer: { jerk: 1 },
  });
  let least = Infinity;
  for (let i = 0; i <= 2000; i++) least = Math.min(least, at(r.curves.y, i / 2000));
  expect(least).toBeGreaterThanOrEqual(-TOLERANCE);
});

const failure = (f: () => unknown): SolveError => {
  try { f(); } catch (e) { if (e instanceof SolveError) return e; throw e; }
  throw new Error("solved; expected a SolveError");
};

// Catches: a conflict named by the wrong holds, or by more than it needs (an innocent hold kept in the set).
test("an infeasible solve names exactly the conflicting holds, by line", () => {
  const made: HoldLine[][] = [];
  const e = failure(() => solve({
    over: [0, 2], vary: { x: spline(8), y: spline(8) },
    hold: ({ x, y }) => {
      const hs = [
        atLeast(x, -1),                                   // innocent
        equal(y, 0, { from: 0, to: 1.3 }),                // planted until 1.3 ...
        equal(vec3(x, y, 0), [1, 0.7, 0], { at: 1.2 }),   // ... but on the deck at 1.2
        equal(x, 0, { at: 0 }),                           // innocent
      ];
      made.push(hs);
      return hs;
    },
    prefer: { jerk: 1 },
  }));
  const f = e.failure;
  if (f.kind !== "infeasible") throw new Error(`expected infeasible, got ${f.kind}`);
  expect(f.conflict).toEqual([made[0]![1]!, made[0]![2]!]);
  expect(f.conflict.map((h) => h.site)).toEqual([made[0]![1]!.site, made[0]![2]!.site]);
  expect(made[0]![1]!.site).toMatch(/tests\/solve\.test\.ts:\d+$/);
  expect(e.message).toContain(made[0]![2]!.site!);
});

// Catches: a hold reading an unknown at another time (a delay), which the solver's jets (the unknowns at the hold's own
// time) would get silently wrong.
test("a hold that reads an unknown at another time is refused", () => {
  const e = failure(() => solve({
    over: [0, 1], vary: { y: spline(6) },
    hold: ({ y }) => [equal(y, 0, { at: 0 }), equal(substitute([y], time, sub(time, 0.3))[0] as Expr<"f32">, 1, { at: 0.9 })],
    prefer: { jerk: 1 },
  }));
  expect(e.failure.kind).toBe("unreadable");
});
