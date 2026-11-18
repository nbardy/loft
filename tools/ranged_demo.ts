// The ranged checks of examples/ranged.ts and examples/serpent.ts beside the sampled frames they replace: each
// condition's margin through the CPU twin at 60 Hz frames, and the ranged decision (agent_notes/2026-11-18_ranged_checks.md).
//   bun tools/ranged_demo.ts [claw|stone|serpent ...]
import { compileScene, type Compiled } from "../src/compile/index.ts";
import { Probe } from "../src/check.ts";
import { abs, add, distance, ge, lift, min, param, sub, time, type Expr } from "../src/expr.ts";
import { always, type Decision, type Ranged } from "../src/ranged.ts";

/** The margin of `e` (an f32 expression) through the CPU twin at time t with params set by name. */
function twin(c: Compiled, e: Expr<"f32">, t: number, params: Readonly<Record<string, number>> = {}): number {
  const probe = new Probe(c, {});
  const s = probe.state(t);
  const ps = Float32Array.from(s.params);
  for (const [name, v] of Object.entries(params)) {
    const slot = c.artefact.params.find((p) => p.name === name);
    if (slot === undefined) throw new Error(`no param "${name}"`);
    ps[slot.slot] = v;
  }
  return c.cpu([e])(c.frame({ ...s, params: ps }), {})[0] as number;
}
const frames = (t0: number, t1: number): number[] => Array.from({ length: Math.round((t1 - t0) * 60) + 1 }, (_, k) => t0 + k / 60);
const said = (d: Decision): string =>
  `holds ${d.holds}, margin ${d.margin.toFixed(5)} in [${d.bounds[0].toFixed(5)}, ${d.bounds[1].toFixed(5)}] at ${Object.entries(d.witness).map(([k, v]) => `${k}=${v.toFixed(5)}`).join(", ")}; ${d.work.boxes} boxes, ${d.work.nodes} nodes, ${d.work.ms.toFixed(0)} ms (${d.spent.kind})`;
const decide = (c: Compiled, r: Ranged): Decision => r.decide(new Probe(c, {}));

const which = new Set(process.argv.slice(2));
const want = (k: string): boolean => which.size === 0 || which.has(k);

if (want("claw") || want("stone")) {
  const m = await import("../examples/ranged.ts");
  const c = compileScene(m.default);
  if (want("claw")) {
    const tip = m.claw.skeleton.joint(m.claw.skeleton.joints - 1);
    const margin = sub(distance(tip.p, m.SHELL.centre), add(m.SHELL.radius, tip.r));
    const sampled = frames(0, 1).map((t) => ({ t, v: twin(c, margin, t) })).reduce((a, b) => (b.v < a.v ? b : a));
    console.log(`claw: sampled 60 Hz: least margin ${sampled.v.toFixed(5)} at t=${sampled.t.toFixed(5)} (${sampled.v >= 0 ? "pass" : "FAIL"})`);
    console.log(`claw: tip radius ${twin(c, lift(tip.r) as Expr<"f32">, 0.5).toFixed(6)}`);
    console.log(`claw: ranged: ${said(decide(c, m.checks[0]!))}`);
  }
  if (want("stone")) {
    const y = m.stone.y, x = m.stone.x;
    const margin = min(sub(0.02, abs(y)), sub(m.PAD.half, abs(sub(x, m.PAD.middle))));
    let missed = 0, worst = { v: Infinity, speed: 0 };
    for (let i = 0; i <= 200; i++) {
      const speed = 4 + (2 * i) / 200;
      const best = Math.max(...frames(0, 2).map((t) => twin(c, margin, t, { "stone.speed": speed })));
      if (best < 0) missed++;
      if (best < worst.v) worst = { v: best, speed };
    }
    console.log(`stone: sampled 60 Hz: ${missed} of 201 speeds have no frame on the pad; least sampled margin ${worst.v.toFixed(5)} at speed ${worst.speed.toFixed(2)}`);
    console.log(`stone: ranged: ${said(decide(c, m.checks[1]!))}`);
  }
}

if (want("serpent")) {
  const m = await import("../examples/serpent.ts");
  const c = compileScene(m.default);
  const rise = param("serpent.neckRise", m.S.neckRise);
  const sampled = frames(0, 10).map((t) => ({ t, v: twin(c, m.headClearance, t) })).reduce((a, b) => (b.v < a.v ? b : a));
  console.log(`serpent: sampled 60 Hz at the default rise ${m.S.neckRise}: least clearance ${sampled.v.toFixed(5)} at t=${sampled.t.toFixed(3)}`);
  console.log(`serpent: ranged at the default rise: ${said(decide(c, always([time, 0, 10], ge(m.headClearance, 0))))}`);
  console.log(`serpent: ranged over rise in [2.5, 4.5]: ${said(decide(c, m.checks[0]!))}`);
  void rise;
}
