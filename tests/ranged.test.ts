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
import serpent, { headClearance, S } from "../examples/serpent.ts";

import * as E from "../src/expr.ts";
import { always, eventually } from "../src/ranged.ts";
import { RangeRefusal, Unbounded } from "../src/interval.ts";
import { checkOutcome, formatChecks, runChecks } from "../src/check.ts";
import { fwidth, wgslFn } from "../src/lib/wgsl.ts";
import { mkdirSync, mkdtempSync, writeFileSync, unlinkSync, rmdirSync } from "node:fs";
import { resolve } from "node:path";
import { scene, strand, toon, plane, world } from "../src/index.ts";

/** The twin's value of an f32 expression at time t (other leaves at their defaults). */
const twin = (c: Compiled, e: Expr<"f32">, t: number): number => new Probe(c, {}).eval([e], t)[0] as number;
const held: ConstructorParameters<typeof Evaluator>[1] = (n) => {
  switch (n.op) {
    case "var": return undefined;
    case "param": return [Math.fround(n.spec.def)];
    case "input": return n.spec.def.map(Math.fround);
  }
};

/** Boxes of time [a, a + w], five points in each: the twin at each must lie in the enclosure, with zero slack. */
function contains(c: Compiled, e: Expr<"f32">, boxes: readonly (readonly [number, number])[]): void {
  for (const [a, w] of boxes) {
    const ev = new Evaluator(new Map([["input:time", [a, a + w]]]), held, "test", { tableFills: c.tableFills, nodes: 0, codomain: 0 });
    const [lo, hi] = ev.scalar(e).at;
    for (let k = 0; k <= 4; k++) {
      const v = twin(c, e, a + (w * k) / 4);
      expect(v).toBeGreaterThanOrEqual(lo);
      expect(v).toBeLessThanOrEqual(hi);
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
  const verdicts = runChecks(demo.default, demo.checks, { compiled: probe.compiled });
  expect(verdicts.map((v) => [v.kind, checkOutcome(v)])).toEqual([["ranged", "fail"], ["ranged", "pass"], ["sampled", "pass"]]);
}, 120000);

const { time, ge } = E;
const probe = new Probe(compileScene(demo.default), {});

test("mix cancellation and wide modulo enclose the rounded twin", () => {
  const e = E.mix(2 ** 56, 1, time);
  const box = new Evaluator(new Map([["input:time", [0.99, 1]]]), held, "mix", { nodes: 0, codomain: 0 });
  expect(box.scalar(e).at[0]).toBeLessThanOrEqual(probe.scalar(e, 1));
  expect(probe.scalar(e, 1)).toBe(0);
  expect(always([time, 1, 1], ge(e, 0.5)).decide(probe).holds).toBe(false);
  const x = E.variable("x", "f32"), y = E.variable("y", "f32");
  const a = 1e19, b = -0.61;
  const mod = E.mod(x, y), read = E.compileJs([mod]);
  const ev = new Evaluator(new Map([["var:x", [a, a * 2]], ["var:y", [-0.63, -0.59]]]), () => undefined, "mod", { nodes: 0, codomain: 0 });
  const [lo, hi] = ev.scalar(mod).at;
  for (let i = 0; i < 64; i++) {
    const actual = read({ vars: { x: a * (1 + i / 64), y: b }, inputs: {}, params: {} })[0] as number;
    expect(actual).toBeGreaterThanOrEqual(lo);
    expect(actual).toBeLessThanOrEqual(hi);
  }
});

test("held leaves use the frame's f32 slots, including vector input overrides", () => {
  const k = E.param("rounding", 16777217), v = E.input("values", "vec2", [1, 16777217]);
  const rod = strand({ name: "rod", spine: (m) => E.vec3(m, k, v.y), girth: 0.1, paint: [1, 1, 1] });
  const c = compileScene(scene({ ground: plane, style: toon(), things: [rod], sun: [0, 1, 0] }));
  for (const p of [new Probe(c, {}), new Probe(c, { values: [2, 16777216.9] })]) {
    for (const leaf of [k, v.y]) {
      const gap = sub(leaf, 16777216);
      expect(p.scalar(gap)).toBe(0);
      expect(always([time, 0, 1], ge(gap, 0.5)).decide(p).holds).toBe(false);
    }
  }
});

test("zero robustness preserves strict, negated and quantified boolean truth", () => {
  const x = E.variable("x", "f32");
  for (const q of [always, eventually]) {
    for (const c of [E.gt(time, 0), E.lt(time, 0), E.not(ge(time, 0)), E.or(E.lt(time, 0), E.gt(time, 0))]) {
      expect(q([time, 0, 0], c).decide(probe).holds).toBe(false);
    }
    expect(q([time, 0, 0], E.not(E.lt(time, 0))).decide(probe).holds).toBe(true);
  }
  expect(always([time, 0, 1], eventually([x, 0, 0], E.gt(time, x))).decide(probe).holds).toBe(false);
  expect(eventually([time, -1, 0], E.gt(time, 0)).decide(probe).holds).toBe(false);
});

test("dependent range ends refuse the named bound leaf", () => {
  const x = E.variable("x", "f32");
  expect(() => always([time, 0, 1], always([x, 0, E.add(time, 0)], E.le(x, 0.5))).decide(probe)).toThrow(RangeRefusal);
});

test("non-finite interval endpoints raise typed overflow", () => {
  const ev = new Evaluator(new Map([["input:time", [2.39, 4.39]]]), () => undefined, "overflow", { nodes: 0, codomain: 0 });
  const e = E.length(E.vec3(E.exp(E.pow(time, time)), 0, 0));
  expect(() => ev.scalar(e)).toThrow(Unbounded);
  expect(() => ev.scalar(e)).toThrow(/overflow/);
});

test("all nested enclosures share the box cap and an exhausted verdict survives formatting", () => {
  const x = E.variable("x", "f32");
  const c = E.gt(sub(time, x), E.mul(sub(time, x), 1));
  for (const boxes of [0, 1, 2, 6, 31]) {
    const check = always([time, 0, 1], eventually([x, 0, 1], c), { boxes });
    const d = check.decide(probe);
    expect(d.work.boxes).toBeLessThanOrEqual(boxes);
    const [v] = runChecks(demo.default, [check], { compiled: probe.compiled });
    expect(v!.pass).toBeUndefined();
    expect(v).toMatchObject({ kind: "ranged", holds: "undecided", bounds: d.bounds, witness: d.witness, spent: { kind: "boxes", boxes } });
    expect(formatChecks([v])).not.toContain("FAIL");
  }
});

test("serpent 3b stops at its five-second wall budget", () => {
  const p = new Probe(compileScene(serpent), {});
  const rise = E.param("serpent.neckRise", S.neckRise);
  const check = always([rise, 2.5, 4.5], always([time, 0, 10], ge(headClearance, 0)), { ms: 5000 });
  const start = performance.now();
  const d = check.decide(p);
  const elapsed = performance.now() - start;
  console.info(`serpent 3b: ${elapsed.toFixed(1)} ms, ${d.work.boxes} boxes, ${d.holds}: ${d.spent.kind}`);
  expect(elapsed).toBeLessThanOrEqual(5500);
  expect(d).toMatchObject({ holds: "undecided", spent: { kind: "ms", ms: 5000 } });
}, 120000);

test("raw shaders, GPU-only functions and CPU tables report the refused construct", () => {
  const raw = wgslFn({ name: "customShader", ret: "f32", wgsl: "fn customShader(x: f32) -> f32 { return x; }" });
  const table = E.fetch({ name: "cpuOnly", role: "data", entries: 1, lanes: 1, prepare: () => () => {} }, 0, 0);
  for (const [value, name] of [[raw(time), "customShader"], [fwidth(time), "loft_fwidth"], [table.x, "cpuOnly"]] as const) {
    const v = always([time, 0, 1], ge(value, 0)).run(probe);
    expect(v).toMatchObject({ kind: "ranged", holds: "undecided" });
    expect(v.measured).toContain(name);
  }
});

test("loft check prints a recurrence refusal and counts it separately", () => {
  mkdirSync("out", { recursive: true });
  const dir = mkdtempSync(resolve("out/ranged-cli-")), file = resolve(dir, "scene.ts");
  try {
    writeFileSync(file, [
      'import rope from "../../examples/rope.ts";',
      'import { always } from "../../src/ranged.ts";',
      'import { time, gt } from "../../src/expr.ts";',
      'export default { ...rope, loop: { kind: "open" } };',
      'export const checks = [always([time, 0, 1], gt(time, 2))];',
    ].join("\n"));
    const r = Bun.spawnSync([process.execPath, "src/cli/main.ts", "check", file], { env: { ...process.env, LOFT_NO_GPU: "1" } });
    const output = r.stdout.toString() + r.stderr.toString();
    expect(output).toContain("undecided: recur");
    expect(output).toContain("0 passed, 0 failed, 1 undecided");
    expect(output).not.toContain("failing checks");
  } finally { unlinkSync(file); rmdirSync(dir); }
});

test("random operator trees enclose the compiled CPU twin with zero slack", () => {
  let seed = 0x18f32;
  const random = () => ((seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0) / 2 ** 32);
  const x = E.variable("fuzzX", "f32"), y = E.variable("fuzzY", "f32");
  type Scalar = Expr<"f32">;
  type Rule = (a: Scalar, b: Scalar, c: Scalar) => Scalar;
  const rules: Rule[] = [
    (a, b) => add(a, b), (a, b) => sub(a, b), (a, b) => E.mul(a, b), (a, b) => E.div(a, b), (a, b) => E.min(a, b), (a, b) => E.max(a, b), (a, b) => E.pow(a, b), (a, b) => E.mod(a, b), (a, b) => E.atan2(a, b), (a, b) => E.step(a, b),
    E.neg, E.abs, E.sign, E.floor, E.fract, E.sqrt, E.exp, E.log, E.sin, E.cos, E.tan, E.saturate,
    E.mix, E.clamp, E.smoothstep, (a, b) => E.length(E.vec2(a, b)),
    (a, b, c) => E.distance(E.vec2(a, b), E.vec2(b, c)), (a, b, c) => E.dot(E.vec2(a, b), E.vec2(b, c)),
    (a, b, c) => E.cross(E.vec3(a, b, c), E.vec3(b, a, c)).z, (a, b) => E.normalize(E.vec2(a, b)).x,
    (a, b, c) => E.select(E.lt(a, b), c, a), (a, b, c) => E.select(E.and(E.ge(a, b), E.not(E.gt(c, b))), a, b),
    (a, b, c) => E.select(E.or(E.le(a, b), E.eq(b, c)), a, c), (a, b, c) => E.select(E.ne(a, b), c, b),
  ];
  const leaf = () => [x, y, E.f(random() * 8 - 4)][Math.floor(random() * 3)]!;
  const tree = (depth: number): Scalar => {
    if (!depth || random() < 0.35) return leaf();
    return rules[Math.floor(random() * rules.length)]!(tree(depth - 1), tree(depth - 1), tree(depth - 1));
  };
  const count = Number(process.env.LOFT_RANGED_FUZZ_TREES ?? 8192);
  const covered = rules.map(() => 0);
  let comparisons = 0;
  for (let start = 0; start < count; start += 32) {
    const lo = random() * 6 - 3, hi = lo + random(), bot = random() * 6 - 3, top = bot + random();
    const box = new Map<string, readonly [number, number]>([["var:fuzzX", [lo, hi]], ["var:fuzzY", [bot, top]]]);
    const roots = Array.from({ length: Math.min(count - start, 32) }, (_, i) => rules[(start + i) % rules.length]!(tree(2), tree(2), tree(2)));
    const read = E.compileJs(roots);
    const bounds = roots.map((root, i) => {
      try {
        const v = new Evaluator(box, () => undefined, "fuzz", { nodes: 0, codomain: 0 }).scalar(root).at;
        covered[(start + i) % rules.length]++;
        return v;
      } catch (e) { if (!(e instanceof Unbounded)) throw e; }
    });
    for (let j = 0; j < 40; j++) {
      const px = j < 4 ? (j & 1 ? hi : lo) : lo + random() * (hi - lo);
      const py = j < 4 ? (j & 2 ? top : bot) : bot + random() * (top - bot);
      const values = read({ vars: { fuzzX: px, fuzzY: py }, inputs: {}, params: {} });
      bounds.forEach((b, i) => {
        if (!b) return;
        const v = values[i] as number;
        if (!(b[0] <= v && v <= b[1])) throw new Error(`tree ${start + i}, rule ${(start + i) % rules.length} at (${px}, ${py}): ${v} outside [${b}]`);
        comparisons++;
      });
    }
  }
  expect(Math.min(...covered)).toBeGreaterThan(0);
  console.info(`ranged fuzz: ${count} trees, ${covered.reduce((a, b) => a + b)} enclosed, ${comparisons} twin comparisons`);
}, 120000);
