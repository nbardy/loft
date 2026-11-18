// Ranged checks (src/interval.ts, src/ranged.ts). Two results others build on:
// - every enclosure holds the CPU twin's value at points inside its box. An unsound operator rule fails this: a form
//   whose interval was left [0, 0] (it emptied every enclosure it touched), comparisons parsed by Bun as type
//   arguments (every one came out "either"), a quaternion sign hull that lost the rotation.
// - the decisions of examples/ranged.ts land on their closed-form answers (agent_notes/2026-11-18_ranged_checks.md):
//   a search that pruned on the wrong side, or negated `eventually` once too often, moves the margin or its sign.
import { expect, test } from "bun:test";
import { compileScene, type Compiled } from "../src/compile/index.ts";
import { Probe } from "../src/check.ts";
import { add, distance, sub, type Expr } from "../src/expr.ts";
import { Evaluator } from "../src/interval.ts";
import * as demo from "../examples/ranged.ts";
import serpent, { headClearance } from "../examples/serpent.ts";

/** The twin's value of an f32 expression at time t (other leaves at their defaults). */
const twin = (c: Compiled, e: Expr<"f32">, t: number): number => new Probe(c, {}).eval([e], t)[0] as number;
const held = (n: { op: string; spec?: { def: number | readonly number[] } }) => (n.op === "var" ? undefined : typeof n.spec!.def === "number" ? [n.spec!.def] : [...n.spec!.def]);

/** Boxes of time [a, a + w], five points in each: the twin at each must lie in the enclosure, to f32 rounding. */
function contains(c: Compiled, e: Expr<"f32">, boxes: readonly (readonly [number, number])[]): void {
  for (const [a, w] of boxes) {
    const ev = new Evaluator(new Map([["input:time", [a, a + w]]]), held as never, "test", { nodes: 0, codomain: 0 });
    const [lo, hi] = ev.scalar(e).at;
    for (let k = 0; k <= 4; k++) {
      const v = twin(c, e, a + (w * k) / 4);
      // The twin folds constants in f32: allow 1e-4 of the value's size.
      const slack = 1e-4 * Math.max(1, Math.abs(v));
      expect(v).toBeGreaterThanOrEqual(lo - slack);
      expect(v).toBeLessThanOrEqual(hi + slack);
    }
  }
}

test("enclosures hold the CPU twin: a claw's tip joint through a scan, the serpent's head over its neck's frames", () => {
  const c = compileScene(demo.default);
  const tip = demo.claw.skeleton.joint(demo.claw.skeleton.joints - 1);
  contains(c, sub(distance(tip.p, demo.SHELL.centre), add(demo.SHELL.radius, tip.r)), [[0, 1], [0.45, 0.1], [0.5, 0.01], [0.508, 0.001]]);
  const s = compileScene(serpent);
  // 1.3231 s: where the first station's quaternion has w = 0 and the chain's sign flips (the case split).
  contains(s, headClearance, [[1, 0.1], [1.32, 0.01], [5.3, 0.05], [7, 0.001]]);
}, 120000);

test("examples/ranged.ts decides as the closed forms say", () => {
  const probe = new Probe(compileScene(demo.default), {});
  const claw = demo.checks[0]!.decide(probe);
  expect(claw.holds).toBe(false);
  expect(claw.margin).toBeCloseTo(-0.35, 3);
  expect(claw.witness.time!).toBeCloseTo(0.508333, 4);
  const stone = demo.checks[1]!.decide(probe);
  expect(stone.holds).toBe(true);
  // 0.01557 from the theory script's grid; the proven bounds must hold it.
  expect(stone.bounds[0]).toBeLessThanOrEqual(0.01558);
  expect(stone.bounds[1]).toBeGreaterThanOrEqual(0.01556);
  expect(stone.witness["stone.speed"]!).toBeCloseTo(6, 6);
}, 120000);
