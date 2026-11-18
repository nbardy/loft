// Lane 2.3 solve input: every corpus scene with holds (heron_leap base and moved, heron_leap_refused, mistake 35) as
// graphs over the unknowns' jets, plus what `src/geom/solve.ts` returns for it: the curves at 181 times, the ranged
// verdict of each returned hold, or the refusal. The bridge until the wire carries Solve decls (§3.2).
// Usage: bun rust/crates/loft-core/dump_solves.ts > out/lr23/solves.json
import { compileScene } from "../../../src/compile/index.ts";
import { Probe } from "../../../src/check.ts";
import * as E from "../../../src/expr.ts";
import { always } from "../../../src/ranged.ts";
import { equal, holdsOf, JET, solve, SolveError, spline, type SolveSpec } from "../../../src/geom/solve.ts";
import { heronLeap, leapSpec, NUMBERS, sceneFor } from "../../../examples/second/heron_leap.ts";
import { graph } from "./dump_graph.ts";

const { time } = E;
const cases: { name: string; spec: SolveSpec<string> }[] = [
  { name: "heron_leap", spec: leapSpec(NUMBERS) as SolveSpec<string> },
  { name: "heron_leap moved", spec: leapSpec({ ...NUMBERS, ...heronLeap.spec.variants.moved!.numbers } as typeof NUMBERS) as SolveSpec<string> },
  // heron_leap_refused.ts: the rail raised to 3.2 m.
  { name: "heron_leap_refused", spec: leapSpec({ ...NUMBERS, rail: { ...NUMBERS.rail, y: 3.2 } }) as SolveSpec<string> },
  // tests/fixtures/mistakes/35_solve_conflict.ts.
  {
    name: "mistake 35", spec: {
      over: [0, 2], vary: { y: spline(8) }, prefer: { jerk: 1 },
      hold: ({ y }) => [always([time, 0, 1.3], equal(y!, 0)), always([time, 1.2, 1.2], equal(y!, 0.7))],
    },
  },
];

// Built after the first solve (a cache hit then), so that solve is timed cold.
let probe: Probe | undefined;
const out = cases.map(({ name, spec }) => {
  const g = graph(name);
  g.slot("time", 0);
  const keys = Object.keys(spec.vary).sort();
  const jets = Object.fromEntries(keys.map((k) => {
    const names = [0, 1, 2, 3].map((d) => `loft.solve.${k}.d${d}`);
    names.forEach((nm) => g.slot(nm, 0));
    return [k, E.lib<"f32">(JET, time, ...names.map((nm) => E.variable(nm, "f32")))];
  }));
  const lines = spec.hold(jets).flatMap((h, i) => holdsOf(h, `hold ${i + 1}`));
  const holds = lines.map((l) => {
    const [a, b] = l.times.kind === "at" ? [l.times.t, l.times.t] : l.times.kind === "during" ? [l.times.from, l.times.to] : spec.over;
    const body = always([time, a, b], l.condition).formula.body;
    if (body.kind !== "atom") throw new Error("a nested hold");
    return { site: l.site, within: l.kind === "equal" ? l.within : null, margin: g.walk(l.margin), check: g.walk(body.margin), test: g.walk(body.condition), times: [a, b] };
  });
  const near = spec.prefer.near;
  const problem = {
    over: spec.over, vary: keys.map((k) => ({ n: spec.vary[k]!.n, from: g.walk(spec.vary[k]!.from) })), delta: spec.delta ?? 4e-6,
    jerk: spec.prefer.jerk ?? 0, acceleration: spec.prefer.acceleration ?? 0, near_weight: near?.weight ?? 0,
    near: near ? Object.keys(near.to).sort().map((k) => [keys.indexOf(k), g.walk(E.lift<"f32">(near.to[k]!))]) : [], holds,
  };
  const start = performance.now();
  let ts: unknown;
  try {
    const solved = solve(spec);
    const ms = performance.now() - start;
    const at = Array.from({ length: 181 }, (_, i) => Math.fround(spec.over[0] + ((spec.over[1] - spec.over[0]) * i) / 180));
    const sample = (c: Record<string, E.Expr<"f32">>) => {
      const f = E.compileJs(keys.map((k) => c[k]!));
      return keys.map((_, k) => at.map((t) => f({ vars: {}, inputs: { time: t }, params: {} })[k] as number));
    };
    const curves = sample(solved.curves);
    // How far TS's own answer moves when delta moves by 1e-9 relative: what the method determines, and what it does not.
    const again = sample(solve({ ...spec, delta: (spec.delta ?? 4e-6) * (1 + 1e-9) }).curves);
    const spread = Math.max(...curves.flatMap((c, k) => c.map((v, i) => Math.abs(v - again[k]![i]!))));
    probe ??= new Probe(compileScene(sceneFor(NUMBERS)), {});
    const verdicts = solved.holds.map((h) => {
      const d = h.decide(probe!);
      return { holds: d.holds, margin: Number.isFinite(d.margin) ? d.margin : null, ms: d.work.ms, boxes: d.work.boxes };
    });
    ts = { outcome: "solved", ms, spread, rounds: solved.report.rounds, steps: solved.report.steps, at, curves, verdicts };
  } catch (e) {
    if (!(e instanceof SolveError)) throw e;
    const f = e.failure, which = (h: { margin: E.Expr; times: unknown }) => lines.findIndex((l) => l.margin.id === h.margin.id && JSON.stringify(l.times) === JSON.stringify(h.times));
    ts = { outcome: f.kind === "unsolved" ? `unsolved ${f.reason}` : f.kind, ms: performance.now() - start, message: e.message,
      holds: f.kind === "infeasible" ? f.conflict.map(which) : "hold" in f ? [which(f.hold)] : [] };
  }
  return { name, slots: g.slots, held: g.held, nodes: g.nodes, solve: problem, ts };
});
console.log(JSON.stringify({ cases: out }));
