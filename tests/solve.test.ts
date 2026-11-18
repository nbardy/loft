// Motion from constraints (src/geom/solve.ts) and rigid parts between keys.
import { expect, test } from "bun:test";
import { leap, NUMBERS } from "../examples/second/heron_leap.ts";
import { compileScene } from "../src/compile/index.ts";
import { plane, Probe, scene, toon } from "../src/index.ts";
import { add, compileJs, cos, evaluate, ge, max, mul, sqrt, sub, substitute, time, vec3, length, type Expr } from "../src/expr.ts";
import { derivative } from "../src/grad.ts";
import { blendFrames, turnedAbout } from "../src/geom/placing.ts";
import { equal, solve, spline, SolveError, TOLERANCE } from "../src/geom/solve.ts";
import { always, eventually, type Ranged } from "../src/ranged.ts";
import * as X from "../src/expr.ts";
const E = { ...X, eventually };
import type { Frame } from "../src/map/types.ts";

const at = (e: Expr, t: number) => evaluate(e, { inputs: { time: t } }) as number;
const ROD = 2, TURN = Math.PI / 6;

test("the solve cache retains the newest 128 successes even through a long stream of distinct problems", () => {
  const run = (target: number) => solve({
    over: [0, 1], vary: { lruTest: spline(4, target) },
    hold: ({ lruTest }) => [always([time, 0, 0], equal(lruTest, target))],
    prefer: { near: { to: { lruTest: target }, weight: 1 } },
  });
  expect(run(0).report.samples).toBe(1);
  for (let i = 1; i <= 512; i++) {
    expect(run(i).report.samples).toBe(1);
    if (i % 64 === 0) expect(run(0).report.samples).toBe(0);
  }
  // With 0 kept hot, the other 127 slots hold 386..512. A FIFO would have evicted 0 and an unbounded cache would hit 385.
  expect(run(386).report.samples).toBe(0);
  expect(run(385).report.samples).toBe(1);
  expect(run(387).report.samples).toBe(1);
});

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
    hold: ({ a }) => [always([time, 0, 0], equal(a, 0)), always([time, 1, 1], equal(a, TURN)), always([time, 0, 0], equal(derivative(a, time), 0)), always([time, 1, 1], equal(derivative(a, time), 0))],
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
      always([time, 0, 0], equal(y, 1.9)), always([time, 0, 0], equal(derivative(y, time), 0)),
      always([time, 0.6, 1.2], equal(derivative(derivative(y, time), time), -9.81)),
      always([time, 1.8, 1.8], equal(y, 2.6)), always([time, 1.8, 1.8], equal(derivative(y, time), 0)),
      always([time, 1.2, 1.2], equal(y, 1.4)),
    ],
    prefer: { jerk: 1 },
  });
  const acc = derivative(derivative(r.curves.y, time), time);
  for (const t of [0.6, 0.75, 0.9, 1.05, 1.2]) expect(Math.abs(at(acc, t) + 9.81)).toBeLessThan(TOLERANCE * 10);
  expect(Math.abs(at(r.curves.y, 1.2) - 1.4)).toBeLessThan(TOLERANCE);
  for (const h of r.report.holds) expect(h.worst).toBeLessThanOrEqual(TOLERANCE);
});

// A fine-grid endpoint can cross a knot when the frame clock stores it in f32.
test("a successful solve meets every hold on its fine grid at stored frame times", () => {
  const { solved } = leap(NUMBERS);
  for (const [index, hold] of solved.holds.entries()) {
    const [a, b] = hold.times.kind === "at" ? [hold.times.t, hold.times.t]
      : hold.times.kind === "during" ? [hold.times.from, hold.times.to] : [0, 1.8];
    const fn = compileJs([hold.margin]);
    const steps = Math.round((b - a) / 0.15 * 64);
    let worst = 0;
    for (let i = 0; i <= steps; i++) {
      const t = i === steps ? b : a + (b - a) * i / steps;
      const value = fn({ inputs: { time: Math.fround(t) }, vars: {}, params: {} })[0]!;
      const components = Array.isArray(value) ? value : [Number(value)];
      const miss = hold.kind === "equal" ? Math.max(...components.map(Math.abs)) : -components[0]!;
      worst = Math.max(worst, miss);
    }
    expect(worst).toBeLessThanOrEqual(solved.report.holds[index]!.worst);
    expect(worst).toBeLessThanOrEqual(hold.kind === "equal" ? hold.within : 0);
  }
});

// A full-span constant halfway between adjacent f32 controls cannot survive storage to this band.
test("rounding spline controls can refuse a hold that double precision met", () => {
  const target = 1024 + 2 ** -14;
  const run = (within: number) => solve({
    over: [0, 1], vary: { x: spline(4, target) }, hold: ({ x }) => [equal(x, target, within)],
    prefer: { near: { to: { x: target }, weight: 1 } },
  });
  // A looser band in the cache cannot certify the same graph with a tighter band.
  expect(run(0.001).report.holds[0]!.worst).toBeLessThanOrEqual(0.001);
  const e = failure(() => run(TOLERANCE));
  expect(e.failure).toMatchObject({ kind: "unsolved", reason: "verification", hold: { kind: "equal" } });
  expect(e.message).toContain("tests/solve.test.ts");
});

// Catches: an interval hold enforced only at its sample times, the curve dipping through it between them.
test("an interval hold holds between its samples", () => {
  const r = solve({
    over: [0, 1], vary: { y: spline(8) },
    hold: ({ y }) => [always([time, 0.2, 0.2], equal(y, 0)), always([time, 0.5, 0.5], equal(y, 1)), always([time, 0.8, 0.8], equal(y, 0)), ge(y, 0)],
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
  const made: Ranged[][] = [];
  const e = failure(() => solve({
    over: [0, 2], vary: { x: spline(8), y: spline(8) },
    hold: ({ x, y }) => {
      const hs = [
        always([time, 0, 2], ge(x, -1)),                          // innocent
        always([time, 0, 1.3], equal(y, 0)),                      // planted until 1.3 ...
        always([time, 1.2, 1.2], equal(vec3(x, y, 0), [1, 0.7, 0])), // ... but on the deck at 1.2
        always([time, 0, 0], equal(x, 0)),                        // innocent
      ];
      made.push(hs);
      return hs;
    },
    prefer: { jerk: 1 },
  }));
  const f = e.failure;
  if (f.kind !== "infeasible") throw new Error(`expected infeasible, got ${f.kind}`);
  expect(f.conflict.map((h) => h.site)).toEqual([made[0]![1]!.site!, made[0]![2]!.site!]);
  expect(f.conflict.map((h) => h.condition)).toEqual([made[0]![1]!.formula.body, made[0]![2]!.formula.body].map((b) => b.kind === "atom" ? b.condition : undefined!));
  expect(made[0]![1]!.site).toMatch(/tests\/solve\.test\.ts:\d+$/);
  expect(e.message).toContain(made[0]![2]!.site!);
});

// Catches: a hold reading an unknown at another time (a delay), which the solver's jets (the unknowns at the hold's own
// time) would get silently wrong.
test("a hold that reads an unknown at another time is refused", () => {
  const e = failure(() => solve({
    over: [0, 1], vary: { y: spline(6) },
    hold: ({ y }) => [always([time, 0, 0], equal(y, 0)), always([time, 0.9, 0.9], equal(substitute([y], time, sub(time, 0.3))[0] as Expr<"f32">, 1))],
    prefer: { jerk: 1 },
  }));
  expect(e.failure.kind).toBe("unreadable");
});

test("a non-finite hold names the time, including between optimisation samples", () => {
  const atSample = sqrt(sub(0.5, time));
  const between = sqrt(sub(mul(sub(time, 0.5), sub(time, 0.5)), 0.01));
  // The first golden-section point beside t=0 lies strictly between all verification grid points.
  const offset = sub(time, (1 - (Math.sqrt(5) - 1) / 2) / 64);
  const atPeak = add(mul(2, time), sqrt(sub(mul(offset, offset), 1e-15)));
  for (const margin of [atSample, between, atPeak]) {
    const e = failure(() => solve({
      over: [0, 1], vary: { x: spline(4) },
      hold: ({ x }) => [always([time, 0, 0], equal(x, 0)), always([time, 0, 1], ge(margin, 0))], prefer: { jerk: 1 },
    }));
    expect(e.failure.kind).toBe("unreadable");
    expect(e.message).toMatch(/ge from t = 0 to 1.*t = [0-9.]+/);
    expect(e.message).toContain("tests/solve.test.ts");
  }
});

test("a finite margin with an overflowing Hessian is unreadable", () => {
  const e = failure(() => solve({
    over: [0, 1], vary: { x: spline(4) }, hold: ({ x }) => [ge(cos(mul(1e200, x)), -2)], prefer: { jerk: 1 },
  }));
  expect(e.failure).toMatchObject({ kind: "unreadable", at: 0 });
  expect(e.message).toContain("non-finite derivative");
});

test("a stationary start for x squared is a local failure", () => {
  const run = (from: number) => solve({
    over: [0, 1], vary: { x: spline(4, from) }, hold: ({ x }) => [ge(mul(x, x), 1)], prefer: { jerk: 1 },
  });
  const e = failure(() => run(0));
  expect(e.failure.kind).toBe("unsolved");
  expect(e.message).toContain("start the curve on the other side");
  expect(run(1).report.holds[0]!.worst).toBe(0);
});

test("the hop clears its bump and returns ranged checks on the authored windows", () => {
  const bump = max(0, sub(0.3, mul(sub(time, 0.5), sub(time, 0.5), 4)));
  const hop = solve({
    over: [0, 1], vary: { y: spline(6) },
    hold: ({ y }) => [always([time, 0, 0], equal(y, 0)), always([time, 1, 1], equal(y, 0)), ge(y, bump)],
    prefer: { jerk: 1 },
  });
  const probe = new Probe(compileScene(scene({ ground: plane, things: [], style: toon(), sun: [0, 1, 0], cameras: {} })), {});
  expect(hop.report.delta).toBe(4e-6);
  for (const check of hop.holds) expect(check.decide(probe).holds).toBe(true);
  // The inequalities still mean the authored bound, even beside a point equality at contact.
  expect(at(hop.curves.y, 0)).toBeGreaterThan(0);
  expect(at(hop.curves.y, 1)).toBeGreaterThan(0);
});

test("the heron rebuild reuses the structural solve and publishes its checks", () => {
  // Scene modules share a process-wide parameter registry. The heron's toon outline has its own default.
  const result = Bun.spawnSync([process.execPath, "-e", `
    import { heronLeap, leap, NUMBERS } from "./examples/second/heron_leap.ts";
    const built = heronLeap.build(heronLeap.resolve({ variant: "base", picks: [] }));
    const { report } = leap(structuredClone(NUMBERS)).solved;
    console.log(JSON.stringify({ samples: report.samples, steps: report.steps, checks: built.checks.length, holds: report.holds.length }));
  `], { cwd: new URL("../", import.meta.url).pathname });
  expect(result.stderr.toString()).toBe("");
  expect(result.exitCode).toBe(0);
  const { samples, steps, checks, holds } = JSON.parse(result.stdout.toString());
  // A cold solve at a feasible start can also take zero Newton steps. A cache hit takes no samples.
  expect(samples).toBe(0);
  expect(steps).toBe(0);
  expect(checks).toBe(holds);
  expect(checks).toBeGreaterThan(0);
}, 120_000);

test("every heron hold has a ranged proof on both decks", () => {
  const probe = new Probe(compileScene(scene({ ground: plane, things: [], style: toon(), sun: [0, 1, 0], cameras: {} })), {});
  for (const deck of [NUMBERS.deck, { from: 3.4, to: 5.2, top: 1, land: 4 }]) {
    const { solved } = leap({ ...NUMBERS, deck });
    const misses = solved.holds.filter(hold => hold.decide(probe).holds !== true);
    expect(misses.map(hold => hold.name)).toEqual([]);
  }
}, 120_000);

test("the 9 m deck stops on the solve deadline", () => {
  const start = performance.now();
  const e = failure(() => leap({ ...NUMBERS, deck: { land: 9, top: 0.7, from: 8.4, to: 10.2 } }, { ms: 2000 }));
  expect(e.failure).toMatchObject({ kind: "unsolved", reason: "budget" });
  expect(performance.now() - start).toBeLessThan(2500);
}, 120_000);

// Catches: a hold the local search cannot honour (a choice of time or branch) solved as something else without a word,
// and an `and` that loses one side. `eventually`, `or` and an always over a param are refused; `and` is two holds.
test("holds are classified once: and is two, eventually and or are refused by name", () => {
  const spec = (hold: Parameters<typeof solve<"y">>[0]["hold"]) => () => solve({ over: [0, 1], vary: { y: spline(6) }, hold, prefer: { jerk: 1 } });
  const both = spec(({ y }) => [always([time, 0, 0], equal(y, 0)), always([time, 0.2, 0.8], E.and(ge(y, 0.1), E.le(y, 2)))])();
  expect(both.holds.map((h) => h.said)).toEqual(["equal", "ge", "le"]);
  expect(() => spec(({ y }) => [E.eventually([time, 0, 1], ge(y, 1))])()).toThrow(/eventually in a hold/);
  expect(() => spec(({ y }) => [E.or(ge(y, 1), E.le(y, -1))])()).toThrow(/a hold built by or/);
  expect(() => spec(({ y }) => [always([E.param("solve.test.k", 1), 0, 2], ge(y, 0))])()).toThrow(/quantified over solve\.test\.k/);
});
