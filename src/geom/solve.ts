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
// - Failure: when the holds cannot be met, a least-squares search for any curve meeting them decides between
//   "infeasible" and "unsolved"; for infeasible, a deletion filter over hold lines leaves a minimal conflicting set,
//   each line named by the file and line it was written on. The searches are local (from the start curve), so
//   "infeasible" means "none found near the start", which for holds linear in the unknowns is exact.

import { SceneError, siteOf } from "../errors.ts";
import { add, compileJs, leaves, lib, lift, mul, sub, time, variable, type LibFn, type Env, type Expr, type In, type NumTy, type V } from "../expr.ts";
import { DerivativeError, defineDerivative, derivative } from "../grad.ts";
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
 * A stated constraint: `margin` is zero (`equal`, every component) or at least zero (`atLeast`) at the `times`.
 * `site` is the line it was written on. The margin is an ordinary expression of the unknowns and `time` (of `time`
 * alone once solved), so a ranged check can read the same hold.
 */
export type HoldLine = {
  readonly kind: "equal" | "atLeast";
  readonly margin: Expr<NumTy>;
  readonly times: Times;
  readonly site: string | undefined;
};

function timesOf(where: string, when: When | undefined): Times {
  if (when === undefined) return { kind: "span" };
  if ("at" in when) {
    if (!Number.isFinite(when.at)) throw new SceneError(where, `at ${when.at}`, "a finite time", "a time in seconds");
    return { kind: "at", t: when.at };
  }
  if (!(Number.isFinite(when.from) && Number.isFinite(when.to) && when.to > when.from)) throw new SceneError(where, `from ${when.from} to ${when.to}`, "from < to", "give the interval in increasing order, or { at } for one instant");
  return { kind: "during", from: when.from, to: when.to };
}

const SIZE: Record<NumTy, number> = { f32: 1, vec2: 2, vec3: 3, vec4: 4 };

/**
 * `a` equals `b` (numbers or vectors of one size, every component) at `when`: an instant `{ at }` (lands, reaches),
 * an interval `{ from, to }` (planted), or the whole span when left out. For `solve`'s `hold`.
 */
export function equal(a: In<NumTy>, b: In<NumTy>, when?: When): HoldLine {
  const site = siteOf(new Error().stack);
  const A = lift<NumTy>(a), B = lift<NumTy>(b);
  if (A.ty !== B.ty && B.ty !== "f32" && A.ty !== "f32") throw new SceneError("equal", `a ${A.ty} and a ${B.ty}`, "two values of one size", "compare like with like: `.xy`, or vec3(...)");
  return { kind: "equal", margin: sub(A, B) as Expr<NumTy>, times: timesOf("equal", when), site };
}

/**
 * `a` is at least `b` (numbers) at `when` (as for `equal`): clear by a distance, above a floor, within a reach
 * (`atLeast(reach, distance)`). For `solve`'s `hold`.
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
};

/** How well one hold is met by the returned curves, measured on the fine grid (and its instant). */
export type HoldReport = { readonly kind: HoldLine["kind"]; readonly site: string | undefined; readonly when: string; readonly worst: number; readonly at: number };

export type SolveReport = {
  /** Wall time of the whole solve, ms. */
  readonly ms: number;
  /** Newton steps tried (each one evaluation of every sampled hold), over every round. */
  readonly steps: number;
  /** Rounds of the exchange (1 when the first samples were enough). */
  readonly rounds: number;
  /** Sample times the interval holds were held at, in all. */
  readonly samples: number;
  /** Per hold: its worst violation (|margin| for equal, -margin for atLeast; at most `TOLERANCE`) and when. */
  readonly holds: readonly HoldReport[];
};

export type Solved<K extends string> = {
  /** The solved curves: `bspline(time, over, [numbers])`, ordinary expressions of time. */
  readonly curves: Readonly<Record<K, F>>;
  /** The holds again, written over the solved curves: expressions of `time` alone, for checks. */
  readonly holds: readonly HoldLine[];
  readonly report: SolveReport;
};

/** Why `solve` returned no curves. */
export type SolveFailure =
  /** No curve of the family meets these holds together (a minimal set: drop any one and the rest can be met). */
  | { readonly kind: "infeasible"; readonly conflict: readonly HoldLine[]; readonly worst: number }
  /** A curve meeting every hold exists, but the search did not settle on the least preferred one within its budget. */
  | { readonly kind: "unsolved"; readonly worst: number; readonly hold: HoldLine }
  /** A hold cannot be evaluated or differentiated on the CPU (raw WGSL, a per-frame table, a function with no derivative). */
  | { readonly kind: "unreadable"; readonly hold: HoldLine; readonly detail: string };

/** HoldLine violations at most this are met (absolute, in each margin's own units), measured on the returned expressions. */
export const TOLERANCE = 1e-5;
/** What the solver aims for in double precision, so the returned f32 numbers still meet `TOLERANCE`. */
const AIM = 1e-8;
/**
 * A violation the solver counts as met, at its samples, in double precision: a tenth of `TOLERANCE`, leaving the rest
 * for the f32 rounding of the returned numbers. A solve that stops above this is unsolved or infeasible.
 */
const MET = TOLERANCE / 10;
/** An atLeast hold whose margin dips below this between samples gets a sample at the dip (in the hold's own units). */
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

function failureParts(f: SolveFailure): [string, string, string] {
  switch (f.kind) {
    case "infeasible": return [
      `holds that cannot all be met: ${f.conflict.map(holdText).join("; ")} (the nearest found misses by ${f.worst.toPrecision(3)})`,
      "holds some curve of the family meets together",
      `drop or loosen one of these ${f.conflict.length}: without any one of them the others can be met`,
    ];
    case "unsolved": return [
      `holds that some curve meets, but the search stopped with ${holdText(f.hold)} off by ${f.worst.toPrecision(3)}`,
      "a solve that settles",
      "give the unknowns more numbers (spline(n)), or a nearer start (spline(n, from) with a rough curve on the right side of each obstacle)",
    ];
    case "unreadable": return [`a hold the CPU cannot read: ${holdText(f.hold)}: ${f.detail}`, "holds made of expressions the JS twin evaluates", "write that hold without raw WGSL or per-frame tables"];
  }
}

/** `solve` could not return curves: the holds conflict (exit 4, with the conflicting lines), or the search did not settle. */
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
function addRow(g: Float64Array, H: Float64Array, n: number, grad: Float64Array, idx: readonly number[], c: number, w: number): void {
  for (const i of idx) {
    const gi = grad[i]!;
    if (gi === 0) continue;
    g[i]! += c * gi;
    for (const j of idx) H[i * n + j]! += w * gi * grad[j]!;
  }
}

/** H += c * the row's Hessian (H is n x n). */
function addCurvature(H: Float64Array, n: number, k: Curvature, c: number): void {
  const L = k.idx.length;
  for (let a = 0; a < L; a++) for (let b = 0; b < L; b++) H[k.idx[a]! * n + k.idx[b]!]! += c * k.m[a * L + b]!;
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
function choleskySolve(A: Float64Array, n: number, damp: number, r: Float64Array): boolean {
  const L = new Float64Array(n * n);
  for (let i = 0; i < n; i++) {
    for (let j = 0; j <= i; j++) {
      let s = A[i * n + j]! + (i === j ? damp : 0);
      for (let k = 0; k < j; k++) s -= L[i * n + k]! * L[j * n + k]!;
      if (i === j) { if (!(s > 0)) return false; L[i * n + i] = Math.sqrt(s); } else L[i * n + j] = s / L[j * n + j]!;
    }
  }
  for (let i = 0; i < n; i++) { let s = r[i]!; for (let k = 0; k < i; k++) s -= L[i * n + k]! * r[k]!; r[i] = s / L[i * n + i]!; }
  for (let i = n - 1; i >= 0; i--) { let s = r[i]!; for (let k = i + 1; k < n; k++) s -= L[k * n + i]! * r[k]!; r[i] = s / L[i * n + i]!; }
  return true;
}

class Problem {
  readonly n: number;
  steps = 0;
  /** The augmented Lagrangian's multipliers by row (`keys`), and its penalty weight. */
  private readonly mult = new Map<string, number>();
  private rho = 10;
  constructor(readonly lines: Line[], readonly channels: readonly Channel[], readonly t0: number, readonly jetNames: readonly (readonly string[])[], readonly Q: Float64Array, readonly b: Float64Array) {
    this.n = channels.reduce((s, c) => s + c.n, 0);
  }

  /** Where unknown k's numbers weigh at t: its position tau in knot gaps and the first number that reaches t. */
  private place(c: Channel, t: number): { tau: number; first: number } {
    const tau = Math.min(Math.max((t - this.t0) / c.h, 0), c.n - 3);
    return { tau, first: Math.max(0, Math.min(c.n - 4, Math.floor(tau))) };
  }

  /** The jets of every unknown at t (value and three time derivatives), as the variables the compiled holds read. */
  env(x: Float64Array, t: number): Env {
    const vars: Record<string, number> = {};
    this.channels.forEach((c, k) => {
      const { tau, first } = this.place(c, t);
      for (let d = 0; d < 4; d++) {
        let v = 0;
        for (let j = first; j < first + 4; j++) v += x[c.offset + j]! * kernel(tau - (j - 1), d);
        vars[this.jetNames[k]![d]!] = v / c.h ** d;
      }
    });
    return { vars, inputs: { time: t }, params: {} };
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
      const w = grad ? line.reads.map((jet) => {
        const ch = this.channels[jet.k]!, { tau, first } = this.place(ch, t), scale = ch.h ** jet.d;
        return [0, 1, 2, 3].map((q) => kernel(tau - (first + q - 1), jet.d) / scale);
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
          const D = pick(1 + line.reads.length + r);
          if (D === 0) continue;
          const [p, q] = line.pairs[r]!, wp = w[p]!, wq = w[q]!, P = slot[p]! * 4, Qs = slot[q]! * 4;
          for (let i = 0; i < 4; i++) for (let j = 0; j < 4; j++) {
            const v2 = D * wp[i]! * wq[j]!;
            M[(P + i) * L + Qs + j]! += v2;
            if (p !== q) M[(Qs + j) * L + P + i]! += v2;
          }
        }
        (equal ? eqGrad : geGrad).push(g);
        (equal ? eqHess : geHess).push({ idx, m: M });
      }
    }
    return { eq: Float64Array.from(eq), ge: Float64Array.from(ge), eqGrad, geGrad, eqHess, geHess };
  }

  objective(x: Float64Array): number {
    let f = 0;
    for (let i = 0; i < this.n; i++) { let s = 0; for (let j = 0; j < this.n; j++) s += this.Q[i * this.n + j]! * x[j]!; f += 0.5 * x[i]! * s - this.b[i]! * x[i]!; }
    return f;
  }

  /** The worst violation of `lines` at their samples. */
  worst(x: Float64Array, lines: readonly Line[]): number {
    const r = this.rows(x, lines, false);
    let w = 0;
    for (const v of r.eq) w = Math.max(w, Math.abs(v));
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
        if (choleskySolve(H!, n, damp, d)) {
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

  /** Least squares of the violations of `lines` from x (in place); returns the worst violation left. */
  feasibility(x: Float64Array, lines: readonly Line[]): number {
    const n = this.n;
    this.descend(x, (y, grad) => {
      const r = this.rows(y, lines, grad);
      let f = 0;
      const g = grad ? new Float64Array(n) : undefined, H = grad ? new Float64Array(n * n) : undefined;
      const add = (v: number, gr: Float64Array | undefined, k: Curvature | undefined): void => {
        f += 0.5 * v * v;
        if (!gr || !k) return;
        addRow(g!, H!, n, gr, k.idx, v, 1);
      };
      r.eq.forEach((v, k) => { add(v, r.eqGrad[k], r.eqHess[k]); if (grad) addCurvature(H!, n, r.eqHess[k]!, v); });
      r.ge.forEach((v, k) => { if (v < 0) { add(v, r.geGrad[k], r.geHess[k]); if (grad) addCurvature(H!, n, r.geHess[k]!, v); } });
      return { f, g, H };
    }, 200);
    return this.worst(x, lines);
  }

  /**
   * The augmented Lagrangian: least objective subject to every line's samples. Returns the worst violation at the
   * samples (at most AIM when it converged). Multipliers are kept across calls by sample count (an exchange round adds
   * rows, which start at 0).
   */
  augmented(x: Float64Array): number {
    const n = this.n, lines = this.lines;
    const keys = this.keys(lines);
    const lam = Float64Array.from(keys.eq, (k) => this.mult.get(k) ?? 0), mu = Float64Array.from(keys.ge, (k) => this.mult.get(k) ?? 0);
    let last = Infinity, v = Infinity;
    for (let outer = 0; outer < 60; outer++) {
      const L = lam, M = mu, R = this.rho, before = Float64Array.from(x);
      this.descend(x, (y, grad) => {
        const r = this.rows(y, lines, grad);
        let f = this.objective(y);
        const g = grad ? new Float64Array(n) : undefined, H = grad ? Float64Array.from(this.Q) : undefined;
        if (grad) for (let i = 0; i < n; i++) { let s = -this.b[i]!; for (let j = 0; j < n; j++) s += this.Q[i * n + j]! * y[j]!; g![i] = s; }
        r.eq.forEach((e, k) => {
          f += L[k]! * e + 0.5 * R * e * e;
          if (grad) { addRow(g!, H!, n, r.eqGrad[k]!, r.eqHess[k]!.idx, L[k]! + R * e, R); addCurvature(H!, n, r.eqHess[k]!, L[k]! + R * e); }
        });
        r.ge.forEach((c, k) => {
          const s = Math.max(0, M[k]! - R * c);
          f += (s * s - M[k]! * M[k]!) / (2 * R);
          if (grad && s > 0) { addRow(g!, H!, n, r.geGrad[k]!, r.geHess[k]!.idx, -s, R); addCurvature(H!, n, r.geHess[k]!, -s); }
        });
        return { f, g, H };
      }, 100, 1e-9);
      const r = this.rows(x, lines, false);
      v = 0;
      r.eq.forEach((e, k) => { v = Math.max(v, Math.abs(e)); lam[k]! += R * e; });
      r.ge.forEach((c, k) => { v = Math.max(v, -c); mu[k] = Math.max(0, mu[k]! - R * c); });
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

let solves = 0;

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
 * exact symbolic first and second derivatives of each hold. Instants are held exactly (to `TOLERANCE`, measured on
 * the returned expressions); intervals and the whole span are held at one sample per knot gap, checked on a grid 64
 * times finer, sampled finely round any break found there and solved again (up to 24 rounds). It runs once, when
 * the scene is built, and is deterministic. `report` gives the time, the work and each hold's worst violation.
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
 *   the start picks the side. Holds not linear in the unknowns may have solutions it does not find, and "infeasible"
 *   means none was found near where the search stopped.
 * - read an unknown at another time than the hold's own (a delay): refused, naming the hold.
 * - hold for every value of a param or input: they take their defaults while solving.
 * - read raw WGSL or per-frame tables in a hold (refused, naming the hold).
 * - change after the build: the result is numbers. A hold that should follow a host input at run time is a closed
 *   form, not a solve.
 *
 * Failure is a `SolveError` (a scene refusal, exit 4): `infeasible` names a minimal set of conflicting holds by file
 * and line; `unsolved` names the hold left furthest off.
 *
 * Relatives: `keys` and `through` (curves from given values); equations of time written directly; `bspline` (the
 * family, by hand).
 */
export function solve<K extends string>(spec: SolveSpec<K>): Solved<K> {
  const start = performance.now();
  const id = solves++;
  const [t0, t1] = spec.over;
  if (!(Number.isFinite(t0) && Number.isFinite(t1) && t1 > t0)) throw new SceneError("solve", `over [${t0}, ${t1}]`, "a span [t0, t1] with t1 > t0", "the seconds the motion takes, in increasing order");
  const keys = Object.keys(spec.vary) as K[];
  if (keys.length === 0) throw new SceneError("solve", "vary with no unknowns", "at least one unknown curve", "vary: { y: spline(8) }");

  // While solving, each unknown is written as its jet at the time being evaluated: `JET(time, v0, v1, v2, v3)`, whose
  // value is v0 and whose time derivative is JET(time, v1, v2, v3, 0). The four are variables set, at each sample
  // time, to the B-spline's value and time derivatives there; a hold reads the unknowns at its own time, so this is
  // the B-spline exactly where it is evaluated, and the hold's derivatives in the jet variables give, by the chain
  // rule, its derivatives in every number (Problem.rows). Only the jets a hold reads appear in it.
  const offset = new Map<K, number>(), channels: Channel[] = [], jetNames: string[][] = [], jetCurves = {} as Record<K, F>;
  let n = 0;
  for (const k of keys) {
    const fam = spec.vary[k];
    offset.set(k, n);
    channels.push({ offset: n, n: fam.n, h: (t1 - t0) / (fam.n - 3) });
    n += fam.n;
    const names = [0, 1, 2, 3].map((d) => `solve${id}_${k}_d${d}`);
    jetNames.push(names);
    jetCurves[k] = lib<"f32">(JET, time, ...names.map((nm) => variable(nm, "f32")));
  }
  const jetOf = new Map(jetNames.flatMap((names, k) => names.map((nm, d) => [nm, { k, d }] as const)));

  const holds = spec.hold(jetCurves);
  if (!Array.isArray(holds) || holds.length === 0) throw new SceneError("solve", "no holds", "at least one hold", "hold: (c) => [equal(c.y, 0, { at: 0 }), ...]");

  // Knot gap of the finest unknown: it sets the sample spacing.
  const h = Math.min(...channels.map((c) => c.h));
  const grid = (a: number, b: number, step: number): number[] => {
    const m = Math.max(1, Math.ceil((b - a) / step - 1e-9));
    return Array.from({ length: m + 1 }, (_, i) => a + ((b - a) * i) / m);
  };
  const lines: Line[] = holds.map((hold) => {
    const [a, b] = spanOf(hold.times, t0, t1);
    if (!(a >= t0 - 1e-12 && b <= t1 + 1e-12)) throw new SceneError("solve", `${holdText(hold)}, outside the span [${t0}, ${t1}]`, "holds within the span", "widen `over`, or move the hold");
    const readNames = [...leaves([hold.margin]).vars.keys()].filter((nm) => jetOf.has(nm));
    // The margin, its gradient in the jets it reads, and its Hessian in them (upper triangle, row by row): with the
    // Hessian the inner steps are Newton steps on the Lagrangian, not Gauss-Newton ones, which converge only linearly
    // where a curved hold (a reach, a clearance) carries a large multiplier.
    const pairs: [number, number][] = [];
    readNames.forEach((_, i) => readNames.forEach((__, j) => { if (j >= i) pairs.push([i, j]); }));
    let roots: Expr[];
    try {
      const vars = readNames.map((nm) => variable(nm, "f32"));
      const grads = vars.map((v) => derivative(hold.margin, v));
      roots = [hold.margin, ...grads, ...pairs.map(([i, j]) => derivative(grads[i]!, vars[j]!))];
    } catch (e) {
      if (e instanceof DerivativeError) throw new SolveError({ kind: "unreadable", hold, detail: e.message });
      throw e;
    }
    const samples = a === b ? [a] : grid(a, b, h);
    const dense = a === b ? [a] : grid(a, b, h / 64);
    return { hold, size: SIZE[hold.margin.ty as NumTy], reads: readNames.map((nm) => jetOf.get(nm)!), pairs, withGrad: compileJs(roots), value: compileJs([hold.margin]), samples, dense };
  });

  // The preference as an exact quadratic 0.5 x'Qx - b'x, by Gauss-Legendre over each knot gap.
  const Q = new Float64Array(n * n), b = new Float64Array(n);
  const pre = spec.prefer;
  const near = pre.near;
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
      const tau = s + (1 + xi) / 2, w = (wq * hk) / 2, t = t0 + tau * hk;
      const js = [s, s + 1, s + 2, s + 3];
      for (const [weight, d] of terms) {
        if (weight <= 0) continue;
        const bd = js.map((j) => kernel(tau - (j - 1), d) / hk ** d);
        js.forEach((i, a) => js.forEach((j, c) => { Q[(o + i) * n + o + j]! += weight * w * bd[a]! * bd[c]!; }));
      }
      const nf = nearFns.get(k);
      if (nf !== undefined && near !== undefined) {
        const r = comps(nf({ vars: {}, inputs: { time: t }, params: {} })[0]!)[0]!;
        const b0 = js.map((j) => kernel(tau - (j - 1), 0));
        js.forEach((i, a) => { b[o + i]! += near.weight * w * r * b0[a]!; js.forEach((j, c) => { Q[(o + i) * n + o + j]! += near.weight * w * b0[a]! * b0[c]!; }); });
      }
    }
  }
  // Scale the preference to order 1 (its largest diagonal), and add a whisper of distance from 0 so the least is unique
  // where the preference is flat (jerk does not see a quadratic).
  let scale = 0;
  for (let i = 0; i < n; i++) scale = Math.max(scale, Q[i * n + i]!);
  for (let i = 0; i < n * n; i++) Q[i]! /= scale;
  for (let i = 0; i < n; i++) { b[i]! /= scale; Q[i * n + i]! += 1e-12; }

  const problem = new Problem(lines, channels, t0, jetNames, Q, b);
  // The start: each unknown's B-spline nearest its `from`, by least squares over the span (Gauss-Legendre again).
  const x = new Float64Array(n);
  for (const k of keys) {
    const ch = channels[keys.indexOf(k)]!, fam = spec.vary[k], from = compileJs([fam.from]);
    const M = new Float64Array(ch.n * ch.n), r = new Float64Array(ch.n);
    for (let s = 0; s < ch.n - 3; s++) for (const [xi, wq] of GL) {
      const tau = s + (1 + xi) / 2, w = (wq * ch.h) / 2, v = comps(from({ vars: {}, inputs: { time: t0 + tau * ch.h }, params: {} })[0]!)[0]!;
      for (let i = s; i < s + 4; i++) {
        const bi = kernel(tau - (i - 1), 0);
        r[i]! += w * v * bi;
        for (let j = s; j < s + 4; j++) M[i * ch.n + j]! += w * bi * kernel(tau - (j - 1), 0);
      }
    }
    choleskySolve(M, ch.n, 1e-12, r);
    x.set(r, ch.offset);
  }
  // Before solving, each hold is read once at made-up numbers: through the jets with gradients (refusing what the CPU
  // cannot evaluate), and again written over the B-splines themselves, which must agree. They differ only for a hold
  // that reads an unknown at a time other than its own (a delay, a substitution of time), which the jets cannot say.
  const probeX = Float64Array.from({ length: n }, (_, i) => 0.1 * Math.sin(1.7 * i + 0.3));
  const probeHolds = spec.hold(bsplines(spec, keys, offset, probeX));
  lines.forEach((line, i) => {
    let jet: number[];
    try {
      problem.rows(probeX, [line], true);
      jet = comps(line.value(problem.env(probeX, line.samples[0]!))[0]!);
    } catch (e) {
      if (e instanceof SceneError) throw e;
      throw new SolveError({ kind: "unreadable", hold: line.hold, detail: e instanceof Error ? e.message : String(e) });
    }
    const real = comps(compileJs([probeHolds[i]!.margin])({ vars: {}, inputs: { time: line.samples[0]! }, params: {} })[0]!);
    if (real.some((r, c) => !(Math.abs(r - jet[c]!) <= 1e-6 * (1 + Math.abs(r))))) {
      throw new SolveError({ kind: "unreadable", hold: line.hold, detail: "it reads an unknown at a time other than its own (a delay or a substitution of time); write the hold at the time it reads" });
    }
  });

  let rounds = 0;
  for (; ;) {
    rounds++;
    const v = problem.augmented(x);
    if (v > MET) fail(problem, lines, x);
    // The exchange: check every interval hold on its fine grid and add the times where it is broken.
    let added = 0;
    for (const line of lines) {
      if (line.dense.length === 1) continue;
      const violation = (t: number): number => {
        const m = comps(line.value(problem.env(x, t))[0]!);
        return line.hold.kind === "equal" ? Math.max(...m.map(Math.abs)) : -m[0]!;
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
      const apart = (t: number) => line.samples.every((u) => Math.abs(u - t) > h / 16);
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
  const curves = bsplines(spec, keys, offset, x);
  const solvedHolds = spec.hold(curves);
  if (solvedHolds.length !== lines.length) throw new SceneError("solve", `a hold function that gave ${lines.length} holds, then ${solvedHolds.length}`, "the same holds every call", "build the list of holds from the curves alone");
  const reports: HoldReport[] = solvedHolds.map((hold, i) => {
    const fn = compileJs([hold.margin]);
    const violation = (t: number): number => {
      const m = comps(fn({ vars: {}, inputs: { time: t }, params: {} })[0]!);
      return hold.kind === "equal" ? Math.max(...m.map(Math.abs)) : -m[0]!;
    };
    const worst = peaks(violation, lines[i]!.dense).reduce((w, p) => (p.v > w.v ? p : w), { t: lines[i]!.dense[0]!, v: -Infinity });
    return { kind: hold.kind, site: hold.site, when: whenText(hold.times), worst: Math.max(worst.v, 0), at: worst.t };
  });
  const off = reports.findIndex((r) => r.worst > TOLERANCE);
  if (off >= 0) throw new SolveError({ kind: "unsolved", worst: reports[off]!.worst, hold: solvedHolds[off]! });
  return {
    curves, holds: solvedHolds,
    report: { ms: performance.now() - start, steps: problem.steps, rounds, samples: lines.reduce((s, l) => s + l.samples.length, 0), holds: reports },
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
function bsplines<K extends string>(spec: SolveSpec<K>, keys: readonly K[], offset: ReadonlyMap<K, number>, x: Float64Array): Record<K, F> {
  const out = {} as Record<K, F>;
  for (const k of keys) out[k] = bspline(time, spec.over, Array.from(x.subarray(offset.get(k)!, offset.get(k)! + spec.vary[k].n)));
  return out;
}

/** The holds were not met: infeasible (with a minimal conflicting set of lines) or unsolved. Always throws. */
function fail(problem: Problem, lines: readonly Line[], x: Float64Array): never {
  // Searches start where the solve stopped: the nearest it got, so "infeasible" is not a start that was merely far.
  const fresh = (): Float64Array => Float64Array.from(x);
  const y = fresh();
  const least = problem.feasibility(y, lines);
  if (least <= MET) {
    let worst = 0, which = lines[0]!;
    for (const line of lines) { const w = problem.worst(x, [line]); if (w > worst) { worst = w; which = line; } }
    throw new SolveError({ kind: "unsolved", worst, hold: which.hold });
  }
  // Deletion filter: drop each line in turn; if the rest still cannot be met, it was not needed for the conflict.
  let set = [...lines];
  for (const line of lines) {
    const rest = set.filter((l) => l !== line);
    if (rest.length === 0) continue;
    if (problem.feasibility(fresh(), rest) > MET) set = rest;
  }
  throw new SolveError({ kind: "infeasible", conflict: set.map((l) => l.hold), worst: least });
}
