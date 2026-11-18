// `solve`'s rounds in the Rust core (rust/loft-core/src/solve.rs): src/geom/solve.ts hands its compiled problem over
// in one call, and gets back the numbers, the steps, the rounds and each hold's samples, or the typed failure, which
// it throws as the SolveError the TypeScript rounds would have thrown. A hold the tape cannot evaluate (a library
// function, a table, a loop) is a named refusal and the TypeScript rounds run.

import type { Env, Expr, LibFn, NumTy } from "../expr.ts";
import { SolveError, type HoldLine } from "../geom/solve.ts";
import { call, KERNEL } from "./ffi.ts";
import { refuse } from "./switch.ts";
import { packTape, type Leaves } from "./tape_pack.ts";
import { leaves as leavesOf } from "../expr.ts";
import { Writer } from "./wire.ts";

/** What solve.ts hands over: its problem as it stands before the first round. */
export type SolveHandoff = {
  readonly lines: readonly {
    readonly hold: HoldLine;
    readonly size: number;
    readonly reads: readonly { readonly k: number; readonly d: number }[];
    readonly pairs: readonly (readonly [number, number])[];
    /** The margin, its gradient in the jets it reads and its Hessian's upper triangle (`withGrad`'s roots). */
    readonly roots: readonly Expr[];
    samples: number[];
    readonly dense: readonly number[];
  }[];
  readonly channels: readonly { readonly offset: number; readonly n: number; readonly h: number }[];
  readonly jetNames: readonly (readonly string[])[];
  readonly t0: number;
  readonly delta: number;
  /** The knot gap of the finest unknown. */
  readonly h: number;
  readonly Q: Float64Array;
  readonly b: Float64Array;
  /** The start; set to the answer. */
  readonly x: Float64Array;
  readonly frame: (t: number, vars?: Env["vars"]) => Env;
  /** The jet function (solve.ts `JET`), whose nodes the conflict certificate reads. */
  readonly jet: LibFn;
  /** `budget.ms` and the `performance.now()` the solve started at, if it has a budget. */
  readonly budget: { readonly ms: number; readonly start: number } | undefined;
};

/** The Rust half of `solve`: its rounds, and the verification of the returned curves. */
export type NativeSolve = { readonly rounds: typeof rounds; readonly verify: typeof verify };
export const solveRust: NativeSolve = { rounds: (s) => rounds(s), verify: (v) => verify(v) };

/** The rounds' result, or undefined when refused (and reported): then the TypeScript rounds run. */
function rounds(s: SolveHandoff): { readonly rounds: number; readonly steps: number } | undefined {
  const w = new Writer();
  w.f64(s.budget === undefined ? -1 : Math.max(0, s.budget.ms - (performance.now() - s.budget.start)));
  w.f64(s.t0); w.f64(s.delta); w.f64(s.h);
  w.u32(s.channels.length);
  for (const c of s.channels) { w.u32(c.offset); w.u32(c.n); w.f64(c.h); }
  w.f64s(s.Q); w.f64s(s.b); w.f64s(s.x);
  const slots = new Map(s.jetNames.flatMap((names, k) => names.map((nm, d) => [nm, 4 * k + d] as const)));
  const at0 = s.frame(0);
  const leaves: Leaves = {
    variable: (name) => slots.get(name),
    input: (spec) => {
      if (spec.name === "time") return "time";
      const v = at0.inputs[spec.name] ?? (spec.def.length === 1 ? spec.def[0]! : spec.def);
      return typeof v === "number" ? [v] : v;
    },
    param: (spec) => at0.params[spec.name] ?? spec.def,
  };
  w.u32(s.lines.length);
  for (const line of s.lines) {
    const hold = line.hold;
    w.u32(hold.kind === "equal" ? 0 : 1);
    w.f64(hold.kind === "equal" ? hold.within : 0);
    w.u32(line.size);
    w.u32(line.reads.length);
    for (const r of line.reads) { w.u32(r.k); w.u32(r.d); }
    w.u32(line.pairs.length);
    for (const [p, q] of line.pairs) { w.u32(p); w.u32(q); }
    w.f64s(line.samples); w.f64s(line.dense);
    for (const roots of [line.roots, line.roots.slice(0, 1)]) {
      const refused = packTape(w, roots, leaves);
      if (refused !== undefined) { refuse("solve", `a hold reads ${refused}`); return undefined; }
    }
    const cert = certificate(line, s.jet);
    w.u32(cert.length);
    for (const c of cert) { w.u32(c.jet); w.f64(c.low); w.f64(c.high); }
  }
  const o = call(KERNEL.solve, w.finish());
  if (o.kind === "failed") {
    const r = o.r, kind = r.u32();
    switch (kind) {
      case 0: throw new SolveError({ kind: "unsolved", reason: "budget", ms: s.budget!.ms });
      case 1: {
        const line = s.lines[r.u32()]!, at = r.f64(), detail = r.u32();
        throw new SolveError({ kind: "unreadable", hold: line.hold, at, detail: ["non-finite margin", "non-finite derivative", "non-finite spline derivative"][detail]! });
      }
      case 2: { const ls = r.u32s(), worst = r.f64(); throw new SolveError({ kind: "infeasible", conflict: [...ls].map((i) => s.lines[i]!.hold), worst }); }
      case 3: { const worst = r.f64(), line = s.lines[r.u32()]!; throw new SolveError({ kind: "unsolved", reason: "search", worst, hold: line.hold }); }
      default: throw new Error(`loft core: solve failed with kind ${kind}`);
    }
  }
  const r = o.r;
  s.x.set(r.f64s());
  const steps = r.u32(), rounds = r.u32();
  for (const line of s.lines) line.samples = Array.from(r.f64s());
  return { rounds, steps };
}

/**
 * The bounds `diagnose` (solve.ts) certifies conflicts with, in its order: constant limits on a jet of one spline
 * component, read from the hold's shape alone (so they are data for the kernel).
 */
function certificate(line: SolveHandoff["lines"][number], jetFn: LibFn): { jet: number; low: number; high: number }[] {
  const out: { jet: number; low: number; high: number }[] = [];
  const node = line.hold.margin.node;
  if (node.op !== "call" || node.fn !== "sub") return out;
  for (const side of [0, 1]) {
    const term = node.args[side]!, limit = node.args[1 - side]!.node;
    if (limit.op !== "const") continue;
    const components: readonly Expr<NumTy>[] = (term.node.op === "vec" ? term.node.args : [term]) as readonly Expr<NumTy>[];
    if (!components.every((e) => e.ty === "f32")) continue;
    for (let i = 0; i < line.size; i++) {
      const jet = components[components.length === 1 ? 0 : i]!;
      if (jet.node.op !== "lib" || jet.node.fn !== jetFn) continue;
      const value = limit.v[limit.v.length === 1 ? 0 : i]!;
      const width = line.hold.kind === "equal" ? line.hold.within : 0;
      const radius = width + 4 * Number.EPSILON * (Math.abs(value) + width);
      const low = line.hold.kind === "equal" ? value - radius : side === 0 ? value : -Infinity;
      const high = line.hold.kind === "equal" ? value + radius : side === 0 ? Infinity : value;
      out.push({ jet: jet.id, low, high });
    }
  }
  return out;
}

/**
 * The returned curves measured as solve.ts measures them (its `reports`): per hold, the worst violation on its fine
 * grid, `{ t, v }`, read in hold order (a hold whose margin is not finite somewhere throws its SolveError when read).
 * Undefined when refused (and reported): then TypeScript measures.
 */
function verify(v: { readonly holds: readonly HoldLine[]; readonly dense: readonly (readonly number[])[]; readonly budget: { readonly ms: number; readonly start: number } | undefined; readonly frameAt: (deps: ReturnType<typeof leavesOf>) => (t: number) => Env }): ((i: number) => { t: number; v: number }) | undefined {
  const w = new Writer();
  w.f64(v.budget === undefined ? -1 : Math.max(0, v.budget.ms - (performance.now() - v.budget.start)));
  w.u32(v.holds.length);
  for (const [i, hold] of v.holds.entries()) {
    const at0 = v.frameAt(leavesOf([hold.margin]))(0);
    w.u32(hold.kind === "equal" ? 0 : 1);
    w.f64s(v.dense[i]!);
    const refused = packTape(w, [hold.margin], {
      variable: () => undefined,
      input: (spec) => {
        if (spec.name === "time") return "time";
        const x = at0.inputs[spec.name] ?? (spec.def.length === 1 ? spec.def[0]! : spec.def);
        return typeof x === "number" ? [x] : x;
      },
      param: (spec) => at0.params[spec.name] ?? spec.def,
    });
    if (refused !== undefined) { refuse("solve", `a solved hold reads ${refused}`); return undefined; }
  }
  const o = call(KERNEL.verify, w.finish());
  if (o.kind === "failed") throw new SolveError({ kind: "unsolved", reason: "budget", ms: v.budget!.ms });
  const out = v.holds.map((hold) => {
    const status = o.r.u32();
    if (status === 0) { const t = o.r.f64(), x = o.r.f64(); return { t, v: x }; }
    const at = o.r.f64();
    return new SolveError({ kind: "unreadable", hold, at, detail: "non-finite margin" });
  });
  return (i) => { const r = out[i]!; if (r instanceof SolveError) throw r; return r; };
}
