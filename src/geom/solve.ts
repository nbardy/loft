// Motion from stated constraints (docs/proposals/2026-11-18_next.md section 5): `solve` chooses the numbers of unknown
// curves so that stated holds are met and a stated preference is least, once, at build time, on the CPU, and returns
// the curves as ordinary expressions of time (`bspline` of numbers), which the model reads, edits and composes like any
// other. It is one source of curves, not the motion model.
//
// The method (the note agent_notes/2026-11-18_solve/NOTE.md has the reasons):
// - Family: each unknown is a uniform cubic B-spline (`bspline`) over the solve's span. Linear in its numbers, so the
//   preference is an exact quadratic; C2, so jerk is defined; local, so a hold near one time moves few numbers.
// - Holds: `equal(a, b, when)` and `atLeast(a, b, when)`, any expressions of the unknowns and `time`. A hold is a
//   margin expression and the times it applies (an instant, an interval, or the whole span), which is what a ranged
//   check reads; nothing about it is particular to this solver.
// - Solver: an augmented Lagrangian (Powell-Hestenes-Rockafellar) over the numbers, each subproblem by Newton steps
//   with Levenberg-Marquardt damping. The first and second derivatives of every hold are symbolic (src/grad.ts), taken
//   in the unknowns' jets (value and three time derivatives at the hold's time) and carried to the numbers by the
//   B-spline weights, so they are exact and cost per jet, not per number. Redundant holds (an interval sampled more
//   often than the family has freedom) are fine for the Lagrangian, where an active-set QP would need independent rows.
// - Intervals: held at sample times (one per knot gap, and the ends), then checked on a grid 64 times finer; a broken
//   stretch found there is sampled sixteen times per knot gap round its worst time, and the solve runs again from where
//   it was (an exchange method), up to 24 rounds. Between the fine grid points nothing is proven: that is the
//   ranged checks' job.
// - Failure: the search is local. Only contradictory bounds on the same spline component over overlapping windows
//   certify infeasibility. The deadline is shared by construction, optimisation, conflict diagnosis and verification.

import { SceneError, siteOf } from "../errors.ts";
import { abs, add, compileJs, everything, fold, ge, le, leaves, lib, lift, max, mul, replace, sub, time, variable, type LibFn, type Env, type Expr, type In, type NumTy, type V } from "../expr.ts";
import { DerivativeError, defineDerivative, derivative } from "../grad.ts";
import { always, type Ranged } from "../ranged.ts";
import { bspline } from "../curve.ts";

type F = Expr<"f32">;

// ---- what a user writes --------------------------------------------------------------------------------------------

/**
 * The family of an unknown curve, and where the search for it starts. One kind so far: a uniform cubic B-spline with
 * `n` numbers over the solve's span, starting from the B-spline nearest `from` (0 when not given).
 */
export type Family = { readonly kind: "spline"; readonly n: number; readonly from: F };

/**
 * An unknown curve for `solve`'s `vary`: a uniform cubic B-spline with `n` numbers (n - 3 cubic pieces) over the
 * solve's span. More numbers bend more sharply and cost more; one number per 0.15 to 0.3 s is a fair start.
 *
 * `from` (an expression of `time`; default 0) is where the search starts, not a preference: the solve is local, so a
 * motion that must go round something on one side (feet over a rail, not under it) starts from a rough curve on that
 * side. The answer does not lean toward it beyond that (for a curve to stay near, use `prefer.near`).
 */
export function spline(n: number, from: In<"f32"> = 0): Family {
  if (!(Number.isInteger(n) && n >= 4 && n <= 64)) throw new SceneError("spline", String(n), "a whole number of values from 4 to 64", "spline(8) is a fair start for a second or two of motion");
  const start = lift<"f32">(from);
  if (start.ty !== "f32") throw new SceneError("spline", `from as a ${start.ty}`, "a number or an f32 expression of time", "one unknown per component: spline(n, curve.x)");
  return { kind: "spline", n, from: start };
}

/** When a hold applies: one instant, an interval, or (left out) the whole span of the solve. */
export type When = { readonly at: number } | { readonly from: number; readonly to: number };
/** `When` classified, with the whole span as its own variant. */
type Times = { readonly kind: "at"; readonly t: number } | { readonly kind: "during"; readonly from: number; readonly to: number } | { readonly kind: "span" };

/**
 * A stated constraint. At its `times`, the absolute value of each `equal` component must be at most `within`, or the scalar
 * `atLeast` margin must be nonnegative.
 * `site` is the line it was written on. The margin is an ordinary expression of the unknowns and `time` (of `time`
 * alone once solved), so a ranged check can read the same hold.
 */
export type HoldLine = {
  readonly times: Times;
  readonly site: string | undefined;
} & (
  | { readonly kind: "equal"; readonly within: number; readonly margin: Expr<NumTy> }
  | { readonly kind: "atLeast"; readonly margin: F }
);

function timesOf(where: string, when: When | { readonly within: number } | undefined): Times {
  if (when === undefined) return { kind: "span" };
  if ("at" in when) {
    if (!Number.isFinite(when.at)) throw new SceneError(where, `at ${when.at}`, "a finite time", "a time in seconds");
    return { kind: "at", t: when.at };
  }
  if (!("from" in when)) {
    if ("within" in when && !("to" in when)) return { kind: "span" };
    throw new SceneError(where, "an incomplete window", "{ at } or { from, to }", "supply both interval ends or omit the window");
  }
  if (!(Number.isFinite(when.from) && Number.isFinite(when.to) && when.to > when.from)) throw new SceneError(where, `from ${when.from} to ${when.to}`, "from < to", "give the interval in increasing order, or { at } for one instant");
  return { kind: "during", from: when.from, to: when.to };
}

const SIZE: Record<NumTy, number> = { f32: 1, vec2: 2, vec3: 3, vec4: 4 };

/**
 * Each component of `a - b` stays within ±`within` (default 1e-5), in its own units, at `when`: an instant `{ at }` (lands, reaches),
 * an interval `{ from, to }` (planted), or the whole span when left out. For `solve`'s `hold`.
 */
export function equal(a: In<NumTy>, b: In<NumTy>, when?: (When & { readonly within?: number }) | { readonly within: number }): HoldLine {
  const site = siteOf(new Error().stack);
  const A = lift<NumTy>(a), B = lift<NumTy>(b);
  if (A.ty !== B.ty && B.ty !== "f32" && A.ty !== "f32") throw new SceneError("equal", `a ${A.ty} and a ${B.ty}`, "two values of one size", "compare like with like: `.xy`, or vec3(...)");
  const within = when?.within ?? TOLERANCE;
  if (!(Number.isFinite(within) && within >= 0))
    throw new SceneError("equal", `within ${within}`, "a finite nonnegative half-width", "give within in the units of each component");
  return { kind: "equal", within, margin: sub(A, B) as Expr<NumTy>, times: timesOf("equal", when), site };
}

/**
 * `a` is at least `b` (numbers) at `when` (as for `equal`): clear by a distance, above a floor, within a reach
 * (`atLeast(reach, distance)`). `solve` aims at the interior margin `delta` (default 4e-6).
 */
export function atLeast(a: In<"f32">, b: In<"f32">, when?: When): HoldLine {
  const site = siteOf(new Error().stack);
  const A = lift<"f32">(a), B = lift<"f32">(b);
  if (A.ty !== "f32" || B.ty !== "f32") throw new SceneError("atLeast", `a ${A.ty} and a ${B.ty}`, "two numbers", "compare one component (`.y`), or a length");
  return { kind: "atLeast", margin: sub(A, B), times: timesOf("atLeast", when), site };
}

/**
 * What `solve` makes least, a weighted sum: `jerk` and `acceleration` (each the integral over the span of the squared
 * third or second time derivative, summed over the unknowns) and `near` (the integral of the squared difference from
 * given curves of time, for the unknowns named: least change from a curve you had).
 */
export type Prefer<K extends string> = {
  readonly jerk?: number;
  readonly acceleration?: number;
  readonly near?: { readonly to: Partial<Record<K, In<"f32">>>; readonly weight: number };
};

export type SolveSpec<K extends string> = {
  /** The span [t0, t1] the unknowns cover, seconds. Outside it each holds its end value. */
  readonly over: readonly [number, number];
  readonly vary: Readonly<Record<K, Family>>;
  /** The holds, written over the unknowns (each an expression of `time`). Called again with the solved curves. */
  readonly hold: (c: Readonly<Record<K, F>>) => readonly HoldLine[];
  readonly prefer: Prefer<K>;
  /** Interior target for atLeast, in each margin's units (default 4e-6). */
  readonly delta?: number;
  /** Wall-clock limit shared by setup, search and verification, in milliseconds. */
  readonly budget?: { readonly ms: number };
};

/** How well one hold is met by the returned curves, measured on the fine grid (and its instant). */
export type HoldReport = { readonly kind: HoldLine["kind"]; readonly site: string | undefined; readonly when: string; readonly worst: number; readonly at: number };

export type SolveReport = {
  /** Interior target for each atLeast, in its margin's units. */
  readonly delta: number;
  /** Wall time of this call in ms, including cache lookup. */
  readonly ms: number;
  /** Newton steps tried (each one evaluation of every sampled hold), over every round. */
  readonly steps: number;
  /** Rounds of the exchange (1 when the first samples were enough). */
  readonly rounds: number;
  /** Sample times the interval holds were held at, in all. */
  readonly samples: number;
  /** Largest sampled |margin| for equal (at most its `within`), or lower-bound violation for atLeast (zero), and when. */
  readonly holds: readonly HoldReport[];
};

export type Solved<K extends string> = {
  /** The solved curves: `bspline(time, over, [numbers])`, ordinary expressions of time. */
  readonly curves: Readonly<Record<K, F>>;
  /** Ranged checks over the solved curves, one always formula per authored hold and window. */
  readonly holds: readonly (HoldLine & Ranged)[];
  readonly report: SolveReport;
};

/** Why `solve` returned no curves. */
export type SolveFailure =
  /** A contradiction between the named linear bounds, including the equality tolerance. */
  | { readonly kind: "infeasible"; readonly conflict: readonly HoldLine[]; readonly worst: number }
  /** The local search stopped without finding a verified curve. */
  | { readonly kind: "unsolved"; readonly reason: "search"; readonly worst: number; readonly hold: HoldLine }
  /** The returned f32 curve misses the authored band or lower bound at a verification point. */
  | { readonly kind: "unsolved"; readonly reason: "verification"; readonly hold: HoldLine; readonly worst: number; readonly at: number }
  | { readonly kind: "unsolved"; readonly reason: "budget"; readonly ms: number }
  /** A hold cannot be evaluated or differentiated on the CPU (raw WGSL, a per-frame table, a function with no derivative). */
  | { readonly kind: "unreadable"; readonly hold: HoldLine; readonly detail: string; readonly at?: number };

/** Default half-width of the equality band, in each component's units. */
export const TOLERANCE = 1e-5;
/** Convergence target for the interior bounds in double precision. */
const AIM = 1e-8;
/**
 * Sampled search residual above which the solver diagnoses a conflict or adds a neighbourhood of samples.
 * Acceptance always uses the authored bound on the returned curve.
 */
const MET = TOLERANCE / 10;
// Keep the optimisation target inside the reported equality band, leaving room for exchange and returned-curve rounding.
const innerBand = (within: number) => within / 2;
// Four settling allowances above contact, still inside the default half-band target of a planted equality (5e-6).
const DEFAULT_DELTA = 4e-6;
/** An atLeast hold near its target between samples gets a sample at the dip (in the margin's own units). */
const NEAR = 1e-3;

const whenText = (t: Times): string => {
  switch (t.kind) {
    case "at": return `at t = ${t.t}`;
    case "during": return `from t = ${t.from} to ${t.to}`;
    case "span": return "over the whole span";
  }
};
/** The first and last time a hold applies. */
function spanOf(t: Times, t0: number, t1: number): [number, number] {
  switch (t.kind) {
    case "at": return [t.t, t.t];
    case "during": return [t.from, t.to];
    case "span": return [t0, t1];
  }
}
const holdText = (h: HoldLine): string => `${h.kind} ${whenText(h.times)}${h.site !== undefined ? ` (${h.site})` : ""}`;

function sameWindow(a: HoldLine, b: HoldLine): boolean {
  return whenText(a.times) === whenText(b.times) && a.kind === b.kind &&
    (a.kind === "atLeast" || (b.kind === "equal" && a.within === b.within));
}

function failureParts(f: SolveFailure): [string, string, string] {
  switch (f.kind) {
    case "infeasible": return [
      `holds that cannot all be met: ${f.conflict.map(holdText).join("; ")}`,
      "holds some curve of the family meets together",
      "drop or loosen one of the named bounds",
    ];
    case "unsolved": switch (f.reason) {
      case "budget": return [`unsolved: budget of ${f.ms} ms spent`, "a solve within its time budget", "increase budget.ms or simplify the holds"];
      case "verification": return [
        `unsolved: the f32 curve for ${holdText(f.hold)} misses its bound by ${f.worst.toPrecision(3)} at t = ${f.at}`,
        "a returned curve meeting the hold on the verification grid",
        f.hold.kind === "equal" ? "set within to an acceptable error in this hold's units, or change the spline family" : "increase delta in the margin's units, or change the spline family",
      ];
      case "search": return [
        `a local search that stopped with ${holdText(f.hold)} off by ${f.worst.toPrecision(3)}`,
        "a solve that settles",
        "give a starting curve with spline(n, from), or start the curve on the other side of the obstacle",
      ];
    }
    case "unreadable": return [`a hold the CPU cannot read: ${holdText(f.hold)}${f.at === undefined ? "" : ` at t = ${f.at}`}: ${f.detail}`, "holds made of expressions the JS twin evaluates", "use a CPU-readable expression with finite values and derivatives throughout its window"];
  }
}

/** A solve refusal (exit 4). Inspect `failure` for a certified conflict, an unreadable hold or an unsolved search. */
export class SolveError extends SceneError {
  constructor(readonly failure: SolveFailure) {
    const [given, expected, fix] = failureParts(failure);
    super("solve", given, expected, fix);
  }
}

/**
 * An unknown's jet while solving: JET(t, v0, v1, v2, v3) is v0, and its derivative in t is JET(t, v1, v2, v3, 0) (a
 * cubic's fourth derivative is 0 within a knot gap). Never in a returned expression; its WGSL exists only because every
 * library function has one.
 */
const JET: LibFn = { name: "loft_solve_jet", ret: "f32", wgsl: "fn loft_solve_jet(t: f32, a: f32, b: f32, c: f32, d: f32) -> f32 { return a; }", js: (_t: number, a: number) => a };
defineDerivative(JET, (args, ds) => add(ds[1] as F, mul(lib<"f32">(JET, args[0]!, args[2]!, args[3]!, args[4]!, 0), ds[0] as F)));

// ---- the problem in numbers ----------------------------------------------------------------------------------------

/** One hold compiled: its margin and the margin's derivative in every unknown number it reads, at a time. */
type Line = {
  readonly hold: HoldLine;
  readonly size: number;
  /** The jet entries the margin reads (unknown k's d-th time derivative at the hold's own time), in root order. */
  readonly reads: readonly Jet[];
  /** The Hessian roots' jet pairs (indices into reads), after the gradient roots. */
  readonly pairs: readonly (readonly [number, number])[];
  readonly withGrad: (env: Env) => (V | boolean)[];
  readonly value: (env: Env) => (V | boolean)[];
  /** Times the hold is held at, this round. */
  samples: number[];
  /** Times it is checked at (the fine grid; the instant for `at`). */
  readonly dense: readonly number[];
};

/** Unknown `k`'s d-th time derivative (d = 0..3) at the time being evaluated: a variable while solving. */
type Jet = { readonly k: number; readonly d: number };
/** An unknown in numbers: where its numbers start in x, how many, its knot gap. */
type Channel = { readonly offset: number; readonly n: number; readonly h: number };

/** One scalar row of the problem at x: its value and its gradient (over all unknown numbers). */
type Rows = {
  readonly eq: Float64Array; readonly eqGrad: Float64Array[]; readonly eqHess: Curvature[];
  readonly ge: Float64Array; readonly geGrad: Float64Array[]; readonly geHess: Curvature[];
};
/** A row's Hessian in the unknown numbers, on the few it touches: `idx` and the dense |idx| x |idx| block `m`. */
type Curvature = { readonly idx: readonly number[]; readonly m: Float64Array };

/** g += c * grad and H += w * grad grad' for one row, over only the numbers it touches (`idx`). */
function addRow(g: Float64Array, H: Float64Array, n: number, grad: Float64Array, idx: readonly number[], c: number, w: number, check: () => void): void {
  for (const i of idx) {
    check();
    const gi = grad[i]!;
    if (gi === 0) continue;
    g[i]! += c * gi;
    for (const j of idx) H[i * n + j]! += w * gi * grad[j]!;
  }
}

/** H += c * the row's Hessian (H is n x n). */
function addCurvature(H: Float64Array, n: number, k: Curvature, c: number, check: () => void): void {
  const L = k.idx.length;
  for (let a = 0; a < L; a++) {
    check();
    for (let b = 0; b < L; b++) H[k.idx[a]! * n + k.idx[b]!]! += c * k.m[a * L + b]!;
  }
}

const comps = (v: V | boolean): number[] => (typeof v === "number" ? [v] : typeof v === "boolean" ? [v ? 1 : 0] : v);

// Gauss-Legendre, four points on [-1, 1]: exact for polynomials to degree 7, so every integral of a squared spline
// derivative over a knot gap is exact.
const GL = [[-0.8611363115940526, 0.3478548451374538], [-0.3399810435848563, 0.6521451548625461], [0.3399810435848563, 0.6521451548625461], [0.8611363115940526, 0.3478548451374538]] as const;

/** The cubic B-spline kernel's d-th derivative at x (d = 0..3), in numbers: the twin of `bspline`'s kernel. */
function kernel(x: number, d: number): number {
  const s = x < 0 ? -1 : 1, a = Math.abs(x);
  if (a >= 2) return 0;
  const odd = d % 2 === 1 ? s : 1;
  if (a < 1) return odd * [(4 - 6 * a * a + 3 * a * a * a) / 6, (-12 * a + 9 * a * a) / 6, (-12 + 18 * a) / 6, 3][d]!;
  const o = 2 - a;
  return odd * [o * o * o / 6, -o * o / 2, o, -1][d]!;
}

/** Cholesky solve of (A + damp I) x = r in place of r; false when not positive definite. */
function choleskySolve(A: Float64Array, n: number, damp: number, r: Float64Array, check: () => void): boolean {
  const L = new Float64Array(n * n);
  for (let i = 0; i < n; i++) {
    check();
    for (let j = 0; j <= i; j++) {
      if ((j & 127) === 0) check();
      let s = A[i * n + j]! + (i === j ? damp : 0);
      for (let k = 0; k < j; k++) s -= L[i * n + k]! * L[j * n + k]!;
      if (i === j) { if (!(s > 0)) return false; L[i * n + i] = Math.sqrt(s); } else L[i * n + j] = s / L[j * n + j]!;
    }
  }
  for (let i = 0; i < n; i++) { check(); let s = r[i]!; for (let k = 0; k < i; k++) s -= L[i * n + k]! * r[k]!; r[i] = s / L[i * n + i]!; }
  for (let i = n - 1; i >= 0; i--) { check(); let s = r[i]!; for (let k = i + 1; k < n; k++) s -= L[k * n + i]! * r[k]!; r[i] = s / L[i * n + i]!; }
  return true;
}

/** Each held input and parameter uses its f32 frame default. The caller supplies the clock (continuous for quadrature). */
function frameAt(deps: ReturnType<typeof leaves>) {
  const inputs = Object.fromEntries(deps.inputs.filter(s => s.name !== "time").map(s => [s.name, s.def.length === 1 ? Math.fround(s.def[0]!) : s.def.map(Math.fround)]));
  const params = Object.fromEntries(deps.params.map(s => [s.name, Math.fround(s.def)]));
  return (t: number, vars: Env["vars"] = {}): Env => ({ inputs: { time: t, ...inputs }, params, vars });
}

class Problem {
  readonly n: number;
  steps = 0;
  /** The augmented Lagrangian's multipliers by row (`keys`), and its penalty weight. */
  private readonly mult = new Map<string, number>();
  private rho = 10;
  constructor(readonly lines: Line[], readonly channels: readonly Channel[], readonly t0: number, readonly jetNames: readonly (readonly string[])[], readonly Q: Float64Array, readonly b: Float64Array, readonly delta: number, readonly frame: ReturnType<typeof frameAt>, readonly check: () => void) {
    this.n = channels.reduce((s, c) => s + c.n, 0);
  }

  /** The four controls and their weights on this span, using the same stored time and clamp as the curve. */
  private place(c: Channel, t: number) {
    const raw = (Math.fround(t) - this.t0) / c.h, tau = Math.min(Math.max(raw, 0), c.n - 3);
    const first = Math.max(0, Math.min(c.n - 4, Math.floor(tau)));
    const weight = (q: number, d: number) => {
      if (d > 0 && raw !== tau) return 0;
      // At a knot the piecewise cubic takes the right span (the last span at the upper endpoint).
      return d === 3 ? [-1, 3, -3, 1][q]! : kernel(tau - (first + q - 1), d);
    };
    return { first, weight };
  }

  /** The jets of every unknown at t (value and three time derivatives), as the variables the compiled holds read. */
  env(x: Float64Array, t: number): Env {
    const vars: Record<string, number> = {};
    this.channels.forEach((c, k) => {
      const { first, weight } = this.place(c, t);
      for (let d = 0; d < 4; d++) {
        let v = 0;
        for (let q = 0; q < 4; q++) v += x[c.offset + first + q]! * weight(q, d);
        vars[this.jetNames[k]![d]!] = v / c.h ** d;
      }
    });
    return this.frame(Math.fround(t), vars);
  }

  /**
   * Every row of `lines` at x, with gradients when asked. A row's gradient is the chain rule through the jets: the
   * margin's derivative in unknown k's d-th derivative, times that derivative's weight on each of k's numbers (the
   * B-spline kernel's d-th derivative), so the symbolic work is per jet (4 per unknown), not per number.
   */
  rows(x: Float64Array, lines: readonly Line[], grad: boolean): Rows {
    const eq: number[] = [], ge: number[] = [], eqGrad: Float64Array[] = [], geGrad: Float64Array[] = [], eqHess: Curvature[] = [], geHess: Curvature[] = [];
    const envs = new Map<number, Env>();
    for (const line of lines) for (const t of line.samples) {
      this.check();
      let env = envs.get(t);
      if (env === undefined) { env = this.env(x, t); envs.set(t, env); }
      const out = (grad ? line.withGrad : line.value)(env);
      const m = comps(out[0]!);
      // Each jet read weighs its unknown's four numbers that reach t; every jet of one unknown reaches the same four,
      // so the row touches 4 numbers per unknown read: local index 4 * (that unknown's place among them) + q.
      const chans: number[] = [], firsts: number[] = [];
      const slot = grad ? line.reads.map((jet) => {
        let p = chans.indexOf(jet.k);
        if (p < 0) { p = chans.push(jet.k) - 1; firsts.push(this.place(this.channels[jet.k]!, t).first); }
        return p;
      }) : [];
      const w = grad ? line.reads.map(jet => {
        const ch = this.channels[jet.k]!, { weight } = this.place(ch, t), scale = ch.h ** jet.d;
        return [0, 1, 2, 3].map(q => weight(q, jet.d) / scale);
      }) : [];
      const idx: number[] = [];
      chans.forEach((k, p) => { for (let q = 0; q < 4; q++) idx.push(this.channels[k]!.offset + firsts[p]! + q); });
      const L = idx.length;
      for (let c = 0; c < line.size; c++) {
        const v = m[c] ?? m[0]!;
        const equal = line.hold.kind === "equal";
        if (equal) eq.push(v); else ge.push(v);
        if (!grad) continue;
        const pick = (r: number): number => { const d = out[r]!; return typeof d === "number" ? d : (d as number[])[c]!; };
        const g = new Float64Array(this.n);
        for (let r = 0; r < line.reads.length; r++) {
          const D = pick(1 + r);
          if (D !== 0) for (let q = 0; q < 4; q++) g[idx[slot[r]! * 4 + q]!]! += D * w[r]![q]!;
        }
        const M = new Float64Array(L * L);
        for (let r = 0; r < line.pairs.length; r++) {
          if ((r & 255) === 0) this.check();
          const D = pick(1 + line.reads.length + r);
          if (D === 0) continue;
          const [p, q] = line.pairs[r]!, wp = w[p]!, wq = w[q]!, P = slot[p]! * 4, Qs = slot[q]! * 4;
          for (let i = 0; i < 4; i++) for (let j = 0; j < 4; j++) {
            const v2 = D * wp[i]! * wq[j]!;
            M[(P + i) * L + Qs + j]! += v2;
            if (p !== q) M[(Qs + j) * L + P + i]! += v2;
          }
        }
        if (!g.every(Number.isFinite) || !M.every(Number.isFinite))
          throw new SolveError({ kind: "unreadable", hold: line.hold, at: t, detail: "non-finite spline derivative" });
        (equal ? eqGrad : geGrad).push(g);
        (equal ? eqHess : geHess).push({ idx, m: M });
      }
    }
    return { eq: Float64Array.from(eq), ge: Float64Array.from(ge), eqGrad, geGrad, eqHess, geHess };
  }

  objective(x: Float64Array): number {
    let f = 0;
    for (let i = 0; i < this.n; i++) { this.check(); let s = 0; for (let j = 0; j < this.n; j++) s += this.Q[i * this.n + j]! * x[j]!; f += 0.5 * x[i]! * s - this.b[i]! * x[i]!; }
    return f;
  }

  /** The worst violation of this hold at its samples. */
  worst(x: Float64Array, line: Line): number {
    const r = this.rows(x, [line], false);
    let w = 0;
    if (line.hold.kind === "equal") for (const v of r.eq) w = Math.max(w, Math.abs(v) - line.hold.within);
    for (const v of r.ge) w = Math.max(w, -v);
    return w;
  }

  /**
   * Minimises a function by Newton steps with Levenberg-Marquardt damping (the damping grows until the damped Hessian is
   * positive definite and the step lowers the function), until a step moves no number by more than `tol` relative or
   * no step lowers it. `terms(x, grad)` gives the value, and with grad the gradient and Hessian. The test is on the
   * step, not the gradient: a preference like jerk is badly conditioned (its least eigenvalues are tiny), so a small
   * gradient can sit far from the least.
   */
  descend(x: Float64Array, terms: (x: Float64Array, grad: boolean) => { f: number; g?: Float64Array; H?: Float64Array }, budget: number, tol = 1e-12): void {
    const n = this.n;
    let damp = 1e-9;
    for (let it = 0; it < budget; it++) {
      const { f, g, H } = terms(x, true);
      let moved = false;
      for (let tries = 0; tries < 40; tries++) {
        const d = Float64Array.from(g!, (v) => -v);
        if (choleskySolve(H!, n, damp, d, this.check)) {
          let step = 0, size = 0;
          for (let i = 0; i < n; i++) { step = Math.max(step, Math.abs(d[i]!)); size = Math.max(size, Math.abs(x[i]!)); }
          if (step <= tol * (1 + size)) return;
          const y = Float64Array.from(x, (v, i) => v + d[i]!);
          this.steps++;
          // Near the least, a step's gain falls below the rounding of f (which carries the penalty terms): accept a
          // step that does not raise f past that rounding, or the badly conditioned directions are never finished.
          if (terms(y, false).f <= f + 1e-13 * Math.abs(f)) { x.set(y); damp = Math.max(damp / 8, 1e-12); moved = true; break; }
        }
        damp = Math.max(damp * 8, 1e-9);
        if (damp > 1e14) break;
      }
      if (!moved) return;
    }
  }

  /**
   * The augmented Lagrangian: least objective subject to every line's samples. Returns the worst violation at the
   * samples (at most AIM when it converged). Multipliers are kept across calls by sample count (an exchange round adds
   * rows, which start at 0).
   */
  augmented(x: Float64Array): number {
    const n = this.n, lines = this.lines;
    const keys = this.keys(lines);
    const bands = lines.flatMap(({ hold, size, samples }) => hold.kind === "equal" ? Array<number>(size * samples.length).fill(innerBand(hold.within)) : []);
    const lam = Float64Array.from(keys.eq, (k) => this.mult.get(k) ?? 0), mu = Float64Array.from(keys.ge, (k) => this.mult.get(k) ?? 0);
    let last = Infinity, v = Infinity;
    for (let outer = 0; outer < 60; outer++) {
      const L = lam, M = mu, R = this.rho, before = Float64Array.from(x);
      // Split each equality band into a target inside the band and a smooth equality subproblem.
      // Holding the target fixed during Newton steps keeps the spline Hessian conditioned at contact.
      const slack = this.rows(x, lines, false).eq.map((e, k) => Math.max(-bands[k]!, Math.min(bands[k]!, e + L[k]! / R)));
      this.descend(x, (y, grad) => {
        const r = this.rows(y, lines, grad);
        let f = this.objective(y);
        const g = grad ? new Float64Array(n) : undefined, H = grad ? Float64Array.from(this.Q) : undefined;
        if (grad) for (let i = 0; i < n; i++) { this.check(); let s = -this.b[i]!; for (let j = 0; j < n; j++) s += this.Q[i * n + j]! * y[j]!; g![i] = s; }
        r.eq.forEach((e, k) => {
          const residual = e - slack[k]!, z = L[k]! + R * residual;
          f += L[k]! * residual + 0.5 * R * residual * residual;
          if (grad) { addRow(g!, H!, n, r.eqGrad[k]!, r.eqHess[k]!.idx, z, R, this.check); addCurvature(H!, n, r.eqHess[k]!, z, this.check); }
        });
        r.ge.forEach((margin, k) => {
          const c = margin - this.delta;
          const s = Math.max(0, M[k]! - R * c);
          f += (s * s - M[k]! * M[k]!) / (2 * R);
          if (grad && s > 0) { addRow(g!, H!, n, r.geGrad[k]!, r.geHess[k]!.idx, -s, R, this.check); addCurvature(H!, n, r.geHess[k]!, -s, this.check); }
        });
        return { f, g, H };
      }, 100, 1e-9);
      const r = this.rows(x, lines, false);
      v = 0;
      r.eq.forEach((e, k) => {
        v = Math.max(v, Math.abs(e) - bands[k]!);
        const u = lam[k]! + R * e;
        lam[k] = u - Math.min(Math.max(u, -R * bands[k]!), R * bands[k]!);
      });
      r.ge.forEach((margin, k) => {
        const c = margin - this.delta;
        v = Math.max(v, -c);
        mu[k] = Math.max(0, mu[k]! - R * c);
      });
      let moved = 0, size = 0;
      for (let i = 0; i < n; i++) { moved = Math.max(moved, Math.abs(x[i]! - before[i]!)); size = Math.max(size, Math.abs(x[i]!)); }
      // Settled: every row within AIM, and the numbers no longer moving to a part in a million (the preference is a matter
      // of taste; the holds are what must be exact).
      if (v <= AIM && moved <= 1e-6 * (1 + size)) break;
      if (v > 0.25 * last) this.rho = Math.min(this.rho * 10, 1e12);
      last = v;
    }
    // The multipliers and the penalty carry into the next exchange round, which adds rows but keeps these.
    keys.eq.forEach((k, i) => this.mult.set(k, lam[i]!));
    keys.ge.forEach((k, i) => this.mult.set(k, mu[i]!));
    return v;
  }

  /** A name for every row, in `rows` order: line, time, component. */
  private keys(lines: readonly Line[]): { eq: string[]; ge: string[] } {
    const eq: string[] = [], ge: string[] = [];
    lines.forEach((line, i) => { for (const t of line.samples) for (let c = 0; c < line.size; c++) (line.hold.kind === "equal" ? eq : ge).push(`${i}:${t}:${c}`); });
    return { eq, ge };
  }
}

// ---- solve ---------------------------------------------------------------------------------------------------------

// The graph interns a node by its operation and children. Stable jet leaves make the root IDs structural keys
// even when two builds allocate fresh hold callbacks. Only successful, verified solutions are cached.
const CACHE_ENTRIES = 128;
const cache = new Map<string, { curves: Readonly<Record<string, F>>; reports: readonly HoldReport[]; margins: readonly number[] }>();

function checkedEvaluator(hold: HoldLine, roots: readonly Expr[], first: number, check: () => void): Line["value"] {
  let fn: Line["value"];
  try { fn = compileJs(roots); }
  catch (e) { throw new SolveError({ kind: "unreadable", hold, at: first, detail: String(e) }); }
  return (env) => {
    check();
    const at = env.inputs.time as number;
    let values;
    try { values = fn(env); }
    catch (e) { throw new SolveError({ kind: "unreadable", hold, at, detail: String(e) }); }
    // A NaN cannot take part in a max or a peak comparison. Check every root before either can discard it.
    for (let i = 0; i < values.length; i++) {
      const v = values[i]!;
      if (!(typeof v === "number" ? Number.isFinite(v) : Array.isArray(v) && v.every(Number.isFinite)))
        throw new SolveError({ kind: "unreadable", hold, at, detail: `non-finite ${i === 0 ? "margin" : "derivative"}` });
    }
    return values;
  };
}

function rangedHold(hold: HoldLine, over: readonly [number, number]): HoldLine & Ranged {
  const range = [time, ...spanOf(hold.times, ...over)] as const;
  const settings = { label: holdText(hold) };
  switch (hold.kind) {
    case "atLeast": return { ...hold, ...always(range, ge(hold.margin, 0), settings) };
    case "equal": {
      const m = abs(hold.margin);
      let largest: F;
      switch (m.ty) {
        case "f32": largest = m as F; break;
        case "vec2": largest = max(m.x, m.y); break;
        case "vec3": largest = max(m.x, m.y, m.z); break;
        case "vec4": largest = max(m.x, m.y, m.z, m.w); break;
      }
      return { ...hold, ...always(range, le(largest, hold.within), settings) };
    }
  }
}

/**
 * Unknown curves chosen to meet stated holds, with the preferred one among those that do: motion from constraints,
 * returned as ordinary expressions of time (`bspline(time, over, [numbers])`) to read, edit and compose.
 *
 * ```ts
 * const leap = solve({
 *   over: [0, 1.8],
 *   vary: { x: spline(10), y: spline(10) },
 *   hold: ({ x, y }) => [
 *     equal(vec2(x, y), [0, 1.9], { at: 0 }),                         // starts here
 *     equal(vec2(x, y), [3, 2.7], { at: 1.2 }),                       // lands
 *     atLeast(distance(vec2(x, y), RAIL), 0.4),                       // clear, always
 *   ],
 *   prefer: { jerk: 1 },
 * });
 * const body = vec3(leap.curves.x, leap.curves.y, 0);
 * ```
 *
 * The law. Each unknown is a uniform cubic B-spline (`spline(n)`: n numbers) over `over`. The numbers minimise the
 * preference (an exact quadratic in them) subject to every hold, by an augmented Lagrangian with Newton steps and
 * exact symbolic derivatives. Each `equal` allows ±`within` (default 1e-5) per component. Each `atLeast` aims for a positive
 * margin (`delta`, default 4e-6) and checks the authored lower bound. Intervals are held once per knot gap on a grid 64
 * times coarser than the verification grid. The search adds samples around breaks and repeats up to 24 times.
 * Final verification reads the returned f32 controls at stored frame times, with the frame's f32 input and param defaults.
 * Pass `checks: leap.holds` to check the entire windows. The report gives the sampled worst misses and `delta`.
 * Identical problems reuse verified curves within a process. The cache keeps the 128 most recently used solutions.
 * A cache hit reports the time of this call and zero steps.
 *
 * Parameters: `hold` is a function of the unknown curves, so any expression of them is a hold (positions through a
 * rig, distances, `derivative(c.y, time)` for a speed or an acceleration); `over`, the families and the preference's
 * weights are numbers.
 *
 * What it cannot do, because of the law:
 * - guarantee between fine-grid times: an interval hold is checked at h/64 spacing (h the knot gap), not proven.
 *   Ranged checks prove it.
 * - find a solution far from its start: the search is local, from each unknown's `spline(n, from)` (default 0). A
 *   hold says nothing of which side of an obstacle to pass ("clear of the rail" is met under it as well as over it):
 *   the start picks the side. A failed local search returns `unsolved` with a suggested starting curve.
 * - read an unknown at another time than the hold's own (a delay): refused, naming the hold.
 * - hold for every value of a param or input: they take their defaults while solving.
 * - read raw WGSL or per-frame tables in a hold (refused, naming the hold).
 * - change after the build: the result is numbers. A hold that should follow a host input at run time is a closed
 *   form, not a solve.
 *
 * Failure is a `SolveError` (exit 4). `infeasible` names certified contradictory bounds. `unreadable` identifies a
 * non-finite margin or derivative at its time. `unsolved` names a local search failure, a missed hold on the
 * returned curve (`verification`), or expiry of `budget: { ms }`.
 *
 * Relatives: `keys` and `through` (curves from given values); equations of time written directly; `bspline` (the
 * family, by hand).
 */
export function solve<K extends string>(spec: SolveSpec<K>): Solved<K> {
  const start = performance.now();
  const ms = spec.budget?.ms, delta = spec.delta ?? DEFAULT_DELTA;
  if (ms !== undefined && !(Number.isFinite(ms) && ms >= 0))
    throw new SceneError("solve", `budget.ms ${ms}`, "a finite nonnegative time budget", "give milliseconds, for example budget: { ms: 2000 }");
  if (!(Number.isFinite(delta) && delta >= 0))
    throw new SceneError("solve", `delta ${delta}`, "a finite nonnegative clearance target", "give delta in the units of the margin");
  const check = () => {
    if (ms !== undefined && performance.now() - start >= ms) throw new SolveError({ kind: "unsolved", reason: "budget", ms });
  };
  check();
  const [t0, t1] = spec.over;
  if (!(Number.isFinite(t0) && Number.isFinite(t1) && t1 > t0)) throw new SceneError("solve", `over [${t0}, ${t1}]`, "a span [t0, t1] with t1 > t0", "the seconds the motion takes, in increasing order");
  const keys = Object.keys(spec.vary).sort() as K[];
  if (keys.length === 0) throw new SceneError("solve", "vary with no unknowns", "at least one unknown curve", "vary: { y: spline(8) }");

  // While solving, each unknown is written as its jet at the time being evaluated: `JET(time, v0, v1, v2, v3)`, whose
  // value is v0 and whose time derivative is JET(time, v1, v2, v3, 0). The four are variables set, at each sample
  // time, to the B-spline's value and time derivatives there; a hold reads the unknowns at its own time, so this is
  // the B-spline exactly where it is evaluated, and the hold's derivatives in the jet variables give, by the chain
  // rule, its derivatives in every number (Problem.rows). Only the jets a hold reads appear in it.
  const offset = new Map<K, number>(), channels: Channel[] = [], jetNames: string[][] = [], jetCurves = {} as Record<K, F>;
  let n = 0;
  for (const k of keys) {
    check();
    const fam = spec.vary[k];
    offset.set(k, n);
    channels.push({ offset: n, n: fam.n, h: (t1 - t0) / (fam.n - 3) });
    n += fam.n;
    const names = [0, 1, 2, 3].map((d) => `loft.solve.${k}.d${d}`);
    jetNames.push(names);
    jetCurves[k] = lib<"f32">(JET, time, ...names.map((nm) => variable(nm, "f32")));
  }
  const jetOf = new Map(jetNames.flatMap((names, k) => names.map((nm, d) => [nm, { k, d }] as const)));

  const holds = spec.hold(jetCurves);
  check();
  if (!Array.isArray(holds) || holds.length === 0) throw new SceneError("solve", "no holds", "at least one hold", "hold: (c) => [equal(c.y, 0, { at: 0 }), ...]");

  const pre = spec.prefer, near = pre.near;
  const weights = [pre.jerk ?? 0, pre.acceleration ?? 0, near?.weight ?? 0];
  if (!weights.every(w => Number.isFinite(w) && w >= 0))
    throw new SceneError("solve", `preference weights ${weights}`, "finite nonnegative weights", "use a positive weight for each preference you want");
  const nearKeys = near ? Object.keys(near.to).sort() as K[] : [];
  const targets = nearKeys.map(k => lift<"f32">(near!.to[k]!));
  // A row's interned ID describes its slot. Its mutable samples also enter a CPU solve.
  const dependencies = leaves([...holds.map(h => h.margin), ...keys.map(k => spec.vary[k].from), ...targets]);
  const frame = frameAt(dependencies);
  const rowContent = dependencies.rows.map(row => {
    check();
    return Array.from(new Uint32Array(row.samples.buffer, row.samples.byteOffset, row.samples.length));
  });
  const key = JSON.stringify([rowContent, spec.over, delta, keys.map(k => [k, spec.vary[k].kind, spec.vary[k].n, spec.vary[k].from.id]),
    weights, nearKeys.map((k, i) => [k, targets[i]!.id]),
    holds.map(h => [h.kind, h.margin.id, h.times, h.kind === "equal" && h.within])]);
  const hit = cache.get(key);
  if (hit) {
    const curves = hit.curves as Readonly<Record<K, F>>, current = spec.hold(curves);
    if (current.length === hit.margins.length && current.every((h, i) =>
      h.margin.id === hit.margins[i] && sameWindow(h, holds[i]!))) {
      const checks = current.map(h => { check(); return rangedHold(h, spec.over); });
      const reports = hit.reports.map((r, i) => ({ ...r, site: current[i]!.site }));
      check();
      // Map order is insertion order. Only a verified hit makes the entry newest.
      cache.delete(key);
      cache.set(key, hit);
      return { curves, holds: checks, report: { ms: performance.now() - start, delta, steps: 0, samples: 0, rounds: 0, holds: reports } };
    }
  }

  // Knot gap of the finest unknown: it sets the sample spacing.
  const h = Math.min(...channels.map((c) => c.h));
  const grid = (a: number, b: number, step: number): number[] => {
    const m = Math.max(1, Math.ceil((b - a) / step - 1e-9));
    return Array.from({ length: m + 1 }, (_, i) => i === m ? b : a + ((b - a) * i) / m);
  };
  const lines: Line[] = holds.map((hold) => {
    check();
    const [a, b] = spanOf(hold.times, t0, t1);
    if (!(a >= t0 - 1e-12 && b <= t1 + 1e-12)) throw new SceneError("solve", `${holdText(hold)}, outside the span [${t0}, ${t1}]`, "holds within the span", "widen `over`, or move the hold");
    // Time differentiation is finished. JET now reads just its value argument. Keeping its three
    // unused derivatives would grow a five-channel position hold's Hessian from 15 entries to 210.
    const values = new Map<number, Expr>();
    for (const { id, node } of everything([hold.margin])) {
      if (node.op !== "lib" || node.fn !== JET) continue;
      if (node.args[0] !== time) throw new SolveError({ kind: "unreadable", hold, detail: "it reads an unknown at a time other than its own; write the hold at the time it reads" });
      values.set(id, node.args[1]!);
    }
    const margin = replace([hold.margin], values)[0] as Expr<NumTy>;
    const readNames = [...leaves([margin]).vars.keys()].filter((nm) => jetOf.has(nm));
    // The margin, its gradient in the jets it reads, and its Hessian in them (upper triangle, row by row): with the
    // Hessian the inner steps are Newton steps on the Lagrangian, not Gauss-Newton ones, which converge only linearly
    // where a curved hold (a reach, a clearance) carries a large multiplier.
    const pairs: [number, number][] = [];
    for (let i = 0; i < readNames.length; i++) {
      check();
      for (let j = i; j < readNames.length; j++) pairs.push([i, j]);
    }
    let roots: Expr[];
    try {
      const vars = readNames.map((nm) => variable(nm, "f32"));
      const grads = vars.map((v) => { check(); return derivative(margin, v); });
      roots = [margin, ...grads, ...pairs.map(([i, j]) => { check(); return derivative(grads[i]!, vars[j]!); })];
    } catch (e) {
      if (e instanceof SolveError) throw e;
      if (e instanceof DerivativeError || e instanceof SceneError) throw new SolveError({ kind: "unreadable", hold, at: a, detail: e.message });
      throw e;
    }
    const samples = a === b ? [a] : grid(a, b, h);
    const dense = a === b ? [a] : grid(a, b, h / 64);
    return { hold, size: SIZE[hold.margin.ty as NumTy], reads: readNames.map((nm) => jetOf.get(nm)!), pairs, withGrad: checkedEvaluator(hold, roots, a, check), value: checkedEvaluator(hold, [margin], a, check), samples, dense };
  });

  // The preference as an exact quadratic 0.5 x'Qx - b'x, by Gauss-Legendre over each knot gap.
  const Q = new Float64Array(n * n), b = new Float64Array(n);
  const nearFns = new Map<K, (env: Env) => (V | boolean)[]>();
  if (near !== undefined) for (const k of Object.keys(near.to) as K[]) {
    if (!keys.includes(k)) throw new SceneError("solve", `prefer.near for "${k}"`, `one of the unknowns (${keys.join(", ")})`, "name an unknown in vary");
    nearFns.set(k, compileJs([lift<"f32">(near.to[k]!)]));
  }
  const terms: [number, number][] = [[pre.jerk ?? 0, 3], [pre.acceleration ?? 0, 2]];
  if (!(terms.some(([w]) => w > 0) || (near !== undefined && near.weight > 0 && nearFns.size > 0))) {
    throw new SceneError("solve", "a preference with no positive weight", "jerk, acceleration or near with a weight above 0", "prefer: { jerk: 1 }");
  }
  for (const k of keys) {
    const N = spec.vary[k].n, hk = (t1 - t0) / (N - 3), o = offset.get(k)!;
    for (let s = 0; s < N - 3; s++) for (const [xi, wq] of GL) {
      check();
      const tau = s + (1 + xi) / 2, w = (wq * hk) / 2, t = t0 + tau * hk;
      const js = [s, s + 1, s + 2, s + 3];
      for (const [weight, d] of terms) {
        if (weight <= 0) continue;
        const bd = js.map((j) => kernel(tau - (j - 1), d) / hk ** d);
        js.forEach((i, a) => js.forEach((j, c) => { Q[(o + i) * n + o + j]! += weight * w * bd[a]! * bd[c]!; }));
      }
      const nf = nearFns.get(k);
      if (nf !== undefined && near !== undefined) {
        const r = comps(nf(frame(t))[0]!)[0]!;
        // A NaN here would poison Q and b, and the Newton steps would then silently refuse to move.
        if (!Number.isFinite(r)) throw new SceneError("solve", `prefer.near for "${k}" = ${r} at t = ${t}`, "a finite curve to stay near", "check the domain of the near curve over the span");
        const b0 = js.map((j) => kernel(tau - (j - 1), 0));
        js.forEach((i, a) => { b[o + i]! += near.weight * w * r * b0[a]!; js.forEach((j, c) => { Q[(o + i) * n + o + j]! += near.weight * w * b0[a]! * b0[c]!; }); });
      }
    }
  }
  // Scale the preference to order 1 (its largest diagonal), and add a whisper of distance from 0 so the least is unique
  // where the preference is flat (jerk does not see a quadratic).
  let scale = 0;
  for (let i = 0; i < n; i++) scale = Math.max(scale, Q[i * n + i]!);
  for (let i = 0; i < n; i++) {
    check();
    for (let j = i * n; j < (i + 1) * n; j++) Q[j]! /= scale;
  }
  for (let i = 0; i < n; i++) { b[i]! /= scale; Q[i * n + i]! += 1e-12; }

  const problem = new Problem(lines, channels, t0, jetNames, Q, b, delta, frame, check);
  // The start: each unknown's B-spline nearest its `from`, by least squares over the span (Gauss-Legendre again).
  const x = new Float64Array(n);
  for (const k of keys) {
    const ch = channels[keys.indexOf(k)]!, fam = spec.vary[k], from = compileJs([fam.from]);
    const M = new Float64Array(ch.n * ch.n), r = new Float64Array(ch.n);
    for (let s = 0; s < ch.n - 3; s++) for (const [xi, wq] of GL) {
      check();
      const tau = s + (1 + xi) / 2, w = (wq * ch.h) / 2, v = comps(from(frame(t0 + tau * ch.h))[0]!)[0]!;
      // A NaN start would surface later as an unreadable hold, blamed on the wrong line.
      if (!Number.isFinite(v)) throw new SceneError("spline", `from = ${v} for "${k}" at t = ${t0 + tau * ch.h}`, "a finite starting curve", "check the domain of the starting curve over the span");
      for (let i = s; i < s + 4; i++) {
        const bi = kernel(tau - (i - 1), 0);
        r[i]! += w * v * bi;
        for (let j = s; j < s + 4; j++) M[i * ch.n + j]! += w * bi * kernel(tau - (j - 1), 0);
      }
    }
    choleskySolve(M, ch.n, 1e-12, r, check);
    x.set(r, ch.offset);
  }
  let rounds = 0;
  for (; ;) {
    rounds++;
    const v = problem.augmented(x);
    if (v > MET) diagnose(problem, lines, x);
    // The exchange: check every interval hold on its fine grid and add the times where it is broken.
    let added = 0;
    for (const line of lines) {
      if (line.dense.length === 1) continue;
      const violation = (t: number): number => {
        const m = comps(line.value(problem.env(x, t))[0]!);
        return line.hold.kind === "equal" ? Math.max(...m.map(Math.abs)) - innerBand(line.hold.within) : delta - m[0]!;
      };
      // The worst time of each stretch that is broken, or that an atLeast hold nearly touches (within NEAR), joins the
      // samples: a local peak of the violation, refined between grid points. Taking the near ones too saves rounds,
      // since the next solve moves a little and a dip that was just clear is often the next one broken.
      const floor = line.hold.kind === "equal" ? MET : -NEAR;
      const bad = peaks(violation, line.dense).filter((p) => p.v > floor);
      // A broken time always joins; a nearly touched one only away from the samples it has (a hold resting on its
      // bound, as a foot on the ground while planted, touches everywhere and is already held there). Round a broken
      // time the whole neighbourhood (h/4 either side) is sampled every h/16: the motion there is faster than the
      // samples (a leg sweeping past the rail), and one time per round only moves the break along.
      const [lo, hi] = [line.dense[0]!, line.dense.at(-1)!];
      const apart = (t: number) => { check(); return line.samples.every((u) => Math.abs(u - t) > h / 16); };
      const fresh = [...new Set(bad.flatMap((p) => p.v > MET
        ? [p.t, ...[-4, -3, -2, -1, 1, 2, 3, 4].map((q) => Math.min(Math.max(p.t + (q * h) / 16, lo), hi)).filter(apart)]
        : apart(p.t) ? [p.t] : []))].filter((t) => !line.samples.includes(t));
      line.samples = [...line.samples, ...fresh].sort((p, q) => p - q);
      added += fresh.length;
    }
    if (added === 0) break;
    if (rounds >= 24) break;
  }

  // The answer as numbers, and the holds measured on the expressions actually returned (f32 constants).
  const curves = bsplines(spec, keys, offset, x, check);
  const solvedHolds = spec.hold(curves);
  if (solvedHolds.length !== lines.length || solvedHolds.some((h, i) => !sameWindow(h, holds[i]!)))
    throw new SceneError("solve", "a hold function that changed its list or windows", "the same holds every call", "build the list of holds from the curves alone");
  const reports: HoldReport[] = solvedHolds.map((hold, i) => {
    const fn = checkedEvaluator(hold, [hold.margin], lines[i]!.dense[0]!, check);
    const frame = frameAt(leaves([hold.margin]));
    const violation = (t: number): number => {
      const m = comps(fn(frame(Math.fround(t)))[0]!);
      return hold.kind === "equal" ? Math.max(...m.map(Math.abs)) : -m[0]!;
    };
    const worst = peaks(violation, lines[i]!.dense).reduce((w, p) => (p.v > w.v ? p : w), { t: lines[i]!.dense[0]!, v: -Infinity });
    const miss = worst.v - (hold.kind === "equal" ? hold.within : 0);
    if (miss > 0) throw new SolveError({ kind: "unsolved", reason: "verification", worst: miss, at: worst.t, hold });
    return { kind: hold.kind, site: hold.site, when: whenText(hold.times), worst: Math.max(worst.v, 0), at: worst.t };
  });
  check();
  const checks = solvedHolds.map(h => { check(); return rangedHold(h, spec.over); });
  check();
  cache.delete(key);
  cache.set(key, { curves, reports, margins: solvedHolds.map(h => h.margin.id) });
  if (cache.size > CACHE_ENTRIES) cache.delete(cache.keys().next().value!);
  return {
    curves, holds: checks,
    report: { delta, ms: performance.now() - start, steps: problem.steps, rounds, samples: lines.reduce((s, l) => s + l.samples.length, 0), holds: reports },
  };
}

/**
 * The local peaks of f over a grid (the ends included), each refined by golden-section search between its grid
 * neighbours: where a hold is worst, to far better than the grid spacing. A peak of a smooth f between grid points is
 * otherwise missed by up to f'' spacing^2 / 8 (a dip under a bound measured 1e-4 at h/64, tests/solve.test.ts).
 */
function peaks(f: (t: number) => number, grid: readonly number[]): { t: number; v: number }[] {
  const v = grid.map(f), out: { t: number; v: number }[] = [];
  const G = (Math.sqrt(5) - 1) / 2;
  grid.forEach((t, i) => {
    if (!(v[i]! >= (v[i - 1] ?? -Infinity) && v[i]! >= (v[i + 1] ?? -Infinity))) return;
    let a = grid[Math.max(0, i - 1)]!, b = grid[Math.min(grid.length - 1, i + 1)]!, best = { t, v: v[i]! };
    let c = b - G * (b - a), d = a + G * (b - a), fc = f(c), fd = f(d);
    for (let k = 0; k < 30 && b - a > 1e-12; k++) {
      if (fc >= fd) { b = d; d = c; fd = fc; c = b - G * (b - a); fc = f(c); } else { a = c; c = d; fc = fd; d = a + G * (b - a); fd = f(d); }
    }
    for (const p of [{ t: c, v: fc }, { t: d, v: fd }]) if (p.v > best.v) best = p;
    out.push(best);
  });
  return out;
}

/** Every unknown as the B-spline of its numbers in x: what solve returns. */
function bsplines<K extends string>(spec: SolveSpec<K>, keys: readonly K[], offset: ReadonlyMap<K, number>, x: Float64Array, check: () => void): Record<K, F> {
  const out = {} as Record<K, F>;
  for (const k of keys) {
    check();
    const begin = offset.get(k)!;
    // Materialise the same f32 controls and constant polynomial coefficients that the shader stores.
    const curve = bspline(time, spec.over, Array.from(x.subarray(begin, begin + spec.vary[k].n), Math.fround));
    out[k] = fold([curve])[0] as F;
  }
  return out;
}

/** Constant limits on a spline jet can contradict one another only where their time windows overlap. */
function diagnose(problem: Problem, lines: readonly Line[], x: Float64Array): void {
  type Bound = { line: Line; low: number; high: number };
  const bounds = new Map<number, Bound[]>();
  let worst = -Infinity, which = lines[0]!;
  for (const line of lines) {
    problem.check();
    const miss = problem.worst(x, line);
    problem.check();
    if (miss > worst) { worst = miss; which = line; }
    if (line.reads.length === 0 && miss > 0) throw new SolveError({ kind: "infeasible", conflict: [line.hold], worst: miss });
    // Certify using the authored bounds (zero clearance, full equality band), independent of the interior targets.
    const node = line.hold.margin.node;
    if (node.op !== "call" || node.fn !== "sub") continue;
    for (const side of [0, 1]) {
      const term = node.args[side]!, limit = node.args[1 - side]!.node;
      if (limit.op !== "const") continue;
      const components = term.node.op === "vec" ? term.node.args : [term];
      if (!components.every(e => e.ty === "f32")) continue;
      for (let i = 0; i < line.size; i++) {
        const jet = components[components.length === 1 ? 0 : i]!;
        if (jet.node.op !== "lib" || jet.node.fn !== JET) continue;
        const value = limit.v[limit.v.length === 1 ? 0 : i]!;
        const width = line.hold.kind === "equal" ? line.hold.within : 0;
        // Widen certificate bounds for rounding in both the margin subtraction and the two endpoints.
        const radius = width + 4 * Number.EPSILON * (Math.abs(value) + width);
        const low = line.hold.kind === "equal" ? value - radius : side === 0 ? value : -Infinity;
        const high = line.hold.kind === "equal" ? value + radius : side === 0 ? Infinity : value;
        const prior = bounds.get(jet.id) ?? [];
        for (const other of prior) {
          problem.check();
          if (other.line.dense[0]! > line.dense.at(-1)! || line.dense[0]! > other.line.dense.at(-1)!) continue;
          if (low > other.high || high < other.low)
            throw new SolveError({ kind: "infeasible", conflict: [...new Set([other.line.hold, line.hold])], worst });
        }
        prior.push({ line, low, high });
        bounds.set(jet.id, prior);
      }
    }
  }
  // An unreachable interior target can still meet the authored bound. Verify the returned curve in that case.
  problem.check();
  if (worst > 0) throw new SolveError({ kind: "unsolved", reason: "search", worst, hold: which.hold });
}
