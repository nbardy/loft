// Lane 2.1 check input: per corpus scene, the TS compiler's table/recur/compute plan (the reference) and the authored
// kernels as graphs (what loft-core plans). Until the recorder writes Kernel bodies and the wire decodes them (wave 1
// integration note), this walk of the TS objects is the bridge, as dump_math.ts is for lane 1.3.
// Usage: bun rust/crates/loft-core/dump_kernels.ts [SCENE...] > out/lr21/kernels.jsonl   (default: examples/**/*.ts)
import { Glob } from "bun";
import { resolve } from "node:path";
import { div, floor, mod, time, variable, vec2, type Expr } from "../../../src/expr.ts";
import { recurTable } from "../../../src/map/recur.ts";
import type { TableSource } from "../../../src/expr.ts";

const MAX_FILL = 1 << 16; // vec4s: TS rest fills larger than this are not dumped
const scenes = Bun.argv.length > 2 ? Bun.argv.slice(2) : [...new Glob("examples/**/*.ts").scanSync(".")].sort();
const [{ subjectOf }, { choose }, { compileScene }] = await Promise.all([
  import("../../../src/design.ts"), import("../../../src/cli/variant.ts"), import("../../../src/compile/index.ts"),
]);

for (const path of scenes) {
  let line: unknown;
  try { line = dump(path, await import(resolve(path)) as Record<string, unknown>); }
  catch (e) { line = { scene: path, outcome: `refused: ${e instanceof Error ? `${e.constructor.name}: ${e.message.slice(0, 160)}` : String(e).slice(0, 160)}` }; }
  console.log(JSON.stringify(line));
}

function dump(path: string, mod_: Record<string, unknown>): unknown {
  const c = compileScene(choose(subjectOf(mod_, path), path, { kind: "default" }).scene), a = c.artefact;
  const nodes: unknown[] = [], loops: unknown[] = [], at = new Map<number, number>(), loopAt = new Map<unknown, number>();
  const res: { name: string; rows: number; lanes: number; shape?: unknown; samples?: number[] }[] = [], resAt = new Map<unknown, number>();
  const subst = new Map<string, Expr>(); // var name -> expression it stands for (a grid index, a step clock)
  const resource = (key: unknown, r: (typeof res)[number]): number => {
    let k = resAt.get(key);
    if (k === undefined) { k = res.push(r) - 1; resAt.set(key, k); }
    return k;
  };
  const table = (t: TableSource): number => {
    const rt = recurTable(t);
    if (rt !== undefined && rt.part === "prev") return table(rt.state.table);
    return resource(t, { name: t.name, rows: rt?.part === "kept" ? rt.state.entries : t.entries, lanes: t.lanes });
  };
  const walk = (e: Expr): number => {
    const seen = at.get(e.id);
    if (seen !== undefined) return seen;
    const n = e.node, ty = e.ty;
    const rec =
      n.op === "const" ? { k: "lit", ty, v: n.v.map(Number) }
      : n.op === "var" ? (subst.has(n.name) ? { k: "same", of: walk(subst.get(n.name)!) } : { k: "var", ty, name: n.name })
      : n.op === "input" ? (n.spec.name === "time" ? { k: "time", ty } : { k: "input", ty, name: n.spec.name })
      : n.op === "param" ? { k: "param", ty, name: n.spec.name }
      : n.op === "row" ? { k: "fetch", ty, res: resource(n.row, { name: `row:${n.row.label}`, rows: n.row.samples.length / 4, lanes: 1, samples: [...n.row.samples] }), lane: 0, index: walk(n.x) }
      : n.op === "fetch" ? { k: "fetch", ty, res: table(n.table), lane: n.lane, index: walk(n.index) }
      : n.op === "call" ? { k: "prim", ty, fn: n.fn, args: n.args.map(walk) }
      : n.op === "vec" ? { k: "vec", ty, args: n.args.map(walk) }
      : n.op === "get" ? { k: "get", ty, a: walk(n.a), sel: [...n.sel].map((ch) => "xyzw".indexOf(ch)) }
      : n.op === "lib" ? { k: "raw", ty, name: n.fn.name, gpuOnly: n.fn.js === "gpu-only", args: n.args.map(walk) }
      : { k: "loop", ty, loop: loopOf(n.loop), comp: n.k };
    const k = (rec as { k: string }).k === "same" ? (rec as { of: number }).of : nodes.push(rec) - 1;
    at.set(e.id, k);
    return k;
  };
  const loopOf = (l: Extract<Expr["node"], { op: "iterate" }>["loop"]): number => {
    let k = loopAt.get(l);
    if (k !== undefined) return k;
    const count = walk(l.count), init = l.init.map(walk), state = l.state.map(walk), index = walk(l.index);
    const next = l.next.map(walk), stop = l.until === undefined ? undefined : walk(l.until(l.next));
    k = loops.push({ count, init, state, index, next, stop }) - 1;
    loopAt.set(l, k);
    return k;
  };

  const kernels: unknown[] = [], tsTables: unknown[] = [], fills: Record<string, number[]> = {};
  const rest = c.frame({
    time: 0, curves: a.curves,
    inputs: (() => { const x = new Float32Array(a.inputFloats); for (const s of a.inputs) x.set(s.def, s.slot); return x; })(),
    params: (() => { const x = new Float32Array(a.params.reduce((n, q) => Math.max(n, q.slot + 1), 0)); for (const q of a.params) x[q.slot] = q.def; return x; })(),
  });
  for (const [t, p] of c.tableFills) {
    const rt = recurTable(t), frameValues = /^loft_frame_\d+$/.test(t.name);
    tsTables.push({ name: t.name, entries: t.entries, lanes: t.lanes, role: t.role, kind: p.canon.kind, placed: p.kind, fill: p.fill.kind, frameValues });
    if (frameValues || rt?.part === "even" || rt?.part === "odd") continue; // lowering products / the host's replay
    if (rt?.part === "kept") {
      const s = rt.state, r = table(t), i = variable(`_lr21_${s.name}_i`, "f32");
      if (s.index.ty === "vec2") subst.set((s.index.node as { name: string }).name, vec2(mod(i, s.shape.kind === "grid" ? s.shape.w : 1), floor(div(i, s.shape.kind === "grid" ? s.shape.w : 1))));
      const idx = s.index.ty === "vec2" ? i : s.index;
      subst.set((s.clock.node as { name: string }).name, time); // the step's own time is the Steps kernel's clock
      const keep = s.kept.kind === "every" ? 1 : s.kept.every;
      kernels.push({ res: r, when: "load", fill: { map: { start: 0, count: s.entries, i: walk(idx), lanes: s.startLanes.map(walk) } } });
      kernels.push({ res: r, when: { steps: { rate: s.rate, from: s.from, count: s.total, keep, states: s.states } }, fill: { map: { start: 0, count: s.entries, i: walk(idx), lanes: s.stepLanes.map(walk) } } });
      continue;
    }
    const r = table(t);
    if (!("program" in t)) { kernels.push({ res: r, when: "derived", fill: { opaque: `prepare (${t.role})` } }); continue; }
    const prog = t.program;
    switch (prog.kind) {
      case "map": {
        let start = 0;
        prog.runs.forEach((run, k) => {
          const i = variable(`_lr21_${t.name}_r${k}`, "f32");
          kernels.push({ res: r, when: "derived", fill: { map: { start, count: run.count, i: walk(i), lanes: run.lanes(i).map(walk) } } });
          start += run.count;
        });
        break;
      }
      case "scan": {
        const state = prog.init.map((e, j) => variable(`_lr21_${t.name}_s${j}`, e.ty)), i = variable(`_lr21_${t.name}_i`, "f32");
        const step = prog.step(state, i);
        kernels.push({ res: r, when: "derived", fill: { scan: { first: prog.first, steps: prog.steps, init: prog.init.map(walk), state: state.map(walk), i: walk(i), next: step.next.map(walk), lanes: step.lanes.map(walk) } } });
        break;
      }
      case "cpu": kernels.push({ res: r, when: "derived", fill: { opaque: `cpu: ${prog.why}` } }); break;
    }
    if (p.canon.kind === "map" || p.canon.kind === "scan") {
      // A table that reads unbaked state (a recurrence) has no CPU reading before a GPU bake: no reference fill.
      if (t.entries * t.lanes <= MAX_FILL) try { fills[t.name] = [...new Uint32Array(rest.table(t).slice(0, t.entries * t.lanes * 4).buffer.slice(0))]; } catch { /* unbaked */ }
    }
  }
  for (const pass of a.compute?.passes ?? []) {
    const key = (x: { key: string; kind: string; entries?: number; lanes?: number; size?: readonly number[] }): number =>
      resource(`compute:${x.key}`, { name: x.key, rows: x.kind === "buffer" ? x.entries! : 0, lanes: x.kind === "buffer" ? x.lanes! : 0, ...(x.kind === "texture" ? { shape: { texture: x.size } } : {}) });
    const w = pass.when;
    const when = w.kind === "build" || w.kind === "load" ? "load" : w.kind === "frame" ? "frame" : { steps: { rate: w.rate, from: 0, count: w.count, keep: w.keep, states: 1 + Math.ceil(w.count / w.keep) } };
    for (const [, out] of pass.writes) kernels.push({ res: key(out), when, fill: { raw: { name: pass.name, dispatch: pass.dispatch, reads: pass.reads.map(([, r]) => key(r)) } } });
  }
  return {
    scene: path, outcome: "built", nodes, loops, resources: res, kernels, fills,
    ts: { tables: tsTables, stages: a.stages.map((s) => s.label), compute: (a.compute?.passes ?? []).map((p) => ({ name: p.name, when: p.when.kind })),
      recurrences: (a.recurrences ?? []).map((r) => ({ name: r.name, entries: r.entries, lanes: r.lanes, total: r.total, kept: r.kept, states: r.states })) },
  };
}
