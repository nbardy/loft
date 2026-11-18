// Lane 2.3 check input: ranged checks as graphs plus the TS decision (`src/ranged.ts` `decide`) for each. Until the
// wire repair lets the recorder's jobs decode (wave 1 integration note), this walk of the TS graph is the bridge.
// A fetch at a constant entry is inlined from its table program, as `src/interval.ts` does.
// Usage: bun rust/crates/loft-core/dump_checks.ts > out/lr23/checks.json
import { compileScene } from "../../../src/compile/index.ts";
import { Probe } from "../../../src/check.ts";
import * as E from "../../../src/expr.ts";
import { always, eventually, type Quantified, type Ranged } from "../../../src/ranged.ts";
import * as demo from "../../../examples/ranged.ts";
import type { Expr, Fn, TableSource } from "../../../src/expr.ts";

const FNS: Fn[] = [
  "add", "sub", "mul", "div", "neg", "min", "max", "pow", "mod", "atan2", "step",
  "abs", "sign", "floor", "fract", "sqrt", "exp", "log", "sin", "cos", "tan", "saturate", "normalize",
  "mix", "clamp", "smoothstep", "length", "distance", "dot", "cross",
  "lt", "gt", "le", "ge", "eq", "ne", "and", "or", "not", "select",
  "ceil", "trunc", "roundEven", "rem", "inverseSqrt", "exp2", "log2", "asin", "acos", "atan", "sinh", "cosh", "tanh",
];
const SEL = "xyzw";
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
  const slots: string[] = [], held: number[] = [], nodes: unknown[] = [], at = new Map<number, number>();
  const slot = (s: string, v: number): number => {
    if (!slots.includes(s)) { slots.push(s); held.push(Math.fround(v)); }
    return slots.indexOf(s);
  };
  const entry = (t: TableSource, k: number): readonly Expr[] => {
    if (!("program" in t)) throw new Error(`${name}: table ${t.name} has no program`);
    const p = t.program;
    switch (p.kind) {
      case "map": { let s = 0; for (const r of p.runs) { if (k < s + r.count) return r.lanes(E.f(k - s)); s += r.count; } throw new Error("past the table"); }
      case "scan": {
        let state = p.init;
        for (let j = 0; j < p.steps; j++) { const s = p.step(state, E.f(j)); if (p.first + j === k) return s.lanes; state = s.next; }
        throw new Error("outside the scan");
      }
    }
  };
  const walk = (e: Expr): number => {
    const seen = at.get(e.id); if (seen !== undefined) return seen;
    const n = e.node;
    let rec: unknown;
    switch (n.op) {
      case "const": rec = { k: "lit", v: n.v.map(Number) }; break;
      case "var": rec = { k: "leaf", slot: slot(n.name, 0) }; break;
      case "param": rec = { k: "leaf", slot: slot(n.spec.name, n.spec.def) }; break;
      case "input": if (n.spec.def.length !== 1) throw new Error(`${name}: vector input`); rec = { k: "leaf", slot: slot(n.spec.name, n.spec.def[0]!) }; break;
      case "call": rec = { k: "call", fn: FNS.indexOf(n.fn), args: n.args.map(walk) }; break;
      case "vec": rec = { k: "vec", w: { vec2: 2, vec3: 3, vec4: 4 }[n.ty], args: n.args.map(walk) }; break;
      case "get": rec = { k: "get", a: walk(n.a), sel: [...n.sel].map((ch) => SEL.indexOf(ch)) }; break;
      case "fetch": {
        // A constant entry (folded by the twin here; an index reading a leaf is outside the bridge and throws).
        const k = E.compileJs([n.index])({ vars: {}, inputs: {}, params: {} })[0] as number;
        if (E.everything([n.index]).some((c) => ["var", "input", "param"].includes(c.node.op))) throw new Error(`${name}: fetch at a leaf-dependent entry`);
        const lanes = entry(n.table, Math.trunc(k));
        const id = walk(lanes[n.lane]!);
        at.set(e.id, id);
        return id;
      }
      case "iterate": {
        // A loop of constant count, unrolled (the tape has no Loop until the arena passes; plan §1.10).
        const l = n.loop, count = E.compileJs([l.count])({ vars: {}, inputs: {}, params: {} })[0] as number;
        let state = l.init, alive: Expr<"bool"> = E.lt(E.f(0), E.f(1));
        for (let i = 0; i < count; i++) {
          const next = l.step(state, E.f(i));
          state = l.until === undefined ? next : next.map((v, j) => E.select(alive, v as Expr<"f32">, state[j] as Expr<"f32">));
          if (l.until) alive = E.and(alive, E.not(l.until(next)));
        }
        const id = walk(state[n.k]!);
        at.set(e.id, id);
        return id;
      }
      default: throw new Error(`${name}: node ${n.op} is outside lane 2.3's bridge`);
    }
    nodes.push(rec); at.set(e.id, nodes.length - 1); return nodes.length - 1;
  };
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
