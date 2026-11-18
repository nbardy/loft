// Lane 2.3 check input: ranged checks as graphs plus the TS decision (`src/ranged.ts` `decide`) for each. Until the
// wire repair lets the recorder's jobs decode (wave 1 integration note), this walk of the TS graph is the bridge.
// The walk is dump_graph.ts.
// Usage: bun rust/crates/loft-core/dump_checks.ts > out/lr23/checks.json
import { compileScene } from "../../../src/compile/index.ts";
import { Probe } from "../../../src/check.ts";
import * as E from "../../../src/expr.ts";
import { always, eventually, type Quantified, type Ranged } from "../../../src/ranged.ts";
import * as demo from "../../../examples/ranged.ts";
import { graph } from "./dump_graph.ts";

const { time, ge, gt, lt, le, sub, mul, add } = E;
const x = E.variable("x", "f32"), y = E.variable("y", "f32");
const wave = E.param("lr23.wave", 1.5, { min: 0.5, max: 2 });

type Case = { name: string; check: Ranged; boxes?: number };
const cases: Case[] = [
  { name: "claw", check: demo.checks[0] as Ranged },
  { name: "stone", check: demo.checks[1] as Ranged },
  { name: "mix-cancel", check: always([time, 1, 1], ge(E.mix(2 ** 56, 1, time), 0.5)) },
  { name: "nested-instant", check: always([time, 0, 1], eventually([x, 0, 0], gt(time, x))) },
  { name: "eventually-never", check: eventually([time, -1, 0], gt(time, 0)) },
  { name: "sine-floor", check: always([time, 0, 6], ge(add(E.sin(mul(time, 2)), 1.25), 0.3)) },
  { name: "sine-param", check: always([wave, 0.5, 2], always([time, 0, 3], ge(sub(2.2, mul(wave, E.cos(time))), 0.1))) },
  { name: "bump-eventually", check: eventually([time, 0, 4], ge(E.exp(mul(E.neg(sub(time, 2.7)), sub(time, 2.7))), 0.99)) },
  { name: "abs-and", check: always([time, -2, 2], E.and(ge(E.abs(time), -0.5), le(mul(time, time), 4.5))) },
  { name: "forall-exists", check: always([y, 0, 1], eventually([time, 0, 2], ge(sub(0.05, E.abs(sub(time, mul(y, 1.7)))), 0))) },
];
for (const q of [always, eventually]) {
  for (const [k, c] of [E.gt(time, 0), E.lt(time, 0), E.not(ge(time, 0)), E.or(E.lt(time, 0), E.gt(time, 0)), E.not(E.lt(time, 0))].entries())
    cases.push({ name: `zero-${q.name}-${k}`, check: q([time, 0, 0], c) });
}
for (const boxes of [0, 1, 2, 6, 31]) {
  const c = E.gt(sub(time, x), mul(sub(time, x), 1));
  cases.push({ name: `budget-${boxes}`, check: always([time, 0, 1], eventually([x, 0, 1], c), { boxes }), boxes });
}

const probe = new Probe(compileScene(demo.default), {});
const out = cases.map(({ name, check, boxes }) => {
  const { slots, held, nodes, slot, walk } = graph(name);
  const formula = (q: Quantified): unknown => {
    const body = q.body.kind === "atom" ? { margin: walk(q.body.margin), test: walk(q.body.condition) } : formula(q.body.q);
    return q.leaves.reduceRight<unknown>((b, l) => {
      const k = slot(l.label, 0);
      return { q: q.kind, leaf: k, lo: walk(l.lo), hi: walk(l.hi), body: b };
    }, body);
  };
  const f = formula(check.formula);
  const d = check.decide(probe);
  return {
    name, slots, held, nodes, formula: f, boxes: boxes ?? 20000, precision: 1e-4,
    ts: { holds: d.holds, margin: Number.isFinite(d.margin) ? d.margin : null, bounds: d.bounds.map((b) => (Number.isFinite(b) ? b : null)), witness: d.witness, boxes: d.work.boxes, ms: d.work.ms, spent: d.spent.kind },
  };
});
console.log(JSON.stringify({ cases: out }));
