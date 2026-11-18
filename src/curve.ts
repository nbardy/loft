// Curves are the atoms: any function of one variable. Curves built from keys live in table rows (editing one is a
// buffer write, not a recompile); `through` is closed form so its control points may move; the motion operators are
// closed-form functions of time with no state (DESIGN.md 2.1).

import {
  abs, add, clamp, cos, CURVE_N, div, exp, lift, lt, max, mul, neg, row, Row, select, sin, smoothstep, step, sub, TAU, time,
  type Expr, type In,
} from "./expr.ts";
import { SceneError } from "./errors.ts";

export type Curve<T extends "f32" | "vec2" | "vec3" | "vec4" = "vec4"> = (x: In<"f32">) => Expr<T>;

/** A key: position in [0, 1] and a scalar or a vector of up to four components. */
export type Key = readonly [number, number | readonly number[]];

// ---- table-backed curves ------------------------------------------------------------------------------------------

/**
 * C1 interpolation through (x, y) keys, monotone between keys (Fritsch-Butland tangents, so a taper never bulges
 * past its keys), clamped outside them. Ported from `strand_kit::key`.
 */
function monotone(xs: readonly number[], ys: readonly number[], u: number): number {
  const n = xs.length;
  if (u <= xs[0]) return ys[0];
  if (u >= xs[n - 1]) return ys[n - 1];
  let k = 0;
  while (u > xs[k + 1]) k++;
  const secant = (i: number): number => (ys[i + 1] - ys[i]) / (xs[i + 1] - xs[i]);
  const tangent = (i: number): number => {
    if (i === 0) return secant(0);
    if (i === n - 1) return secant(n - 2);
    const a = secant(i - 1), b = secant(i);
    return a * b <= 0 ? 0 : (2 * a * b) / (a + b);
  };
  const h = xs[k + 1] - xs[k];
  const t = (u - xs[k]) / h;
  const t2 = t * t, t3 = t2 * t;
  return (2 * t3 - 3 * t2 + 1) * ys[k] + (t3 - 2 * t2 + t) * tangent(k) * h + (-2 * t3 + 3 * t2) * ys[k + 1] + (t3 - t2) * tangent(k + 1) * h;
}

function linear(xs: readonly number[], ys: readonly number[], u: number): number {
  const n = xs.length;
  if (u <= xs[0]) return ys[0];
  if (u >= xs[n - 1]) return ys[n - 1];
  let k = 0;
  while (u > xs[k + 1]) k++;
  return ys[k] + ((ys[k + 1] - ys[k]) * (u - xs[k])) / (xs[k + 1] - xs[k]);
}

const INTERP = { smooth: monotone, linear } as const;
export type Interp = keyof typeof INTERP;

/**
 * A keyed curve, sampled into one table row of CURVE_N vec4s. Scalar keys fill `.x`; vector keys fill as many
 * components as they have (each interpolated on its own). Returns a `Curve<"vec4">`: take `.x` or `.xyz`.
 */
export function keys(points: readonly Key[], options: { interp?: Interp; label?: string } = {}): Curve<"vec4"> {
  if (points.length < 1) throw new Error("keys: need at least one key");
  const width = typeof points[0][1] === "number" ? 1 : (points[0][1] as readonly number[]).length;
  if (width < 1 || width > 4) throw new Error(`keys: values must have 1 to 4 components, got ${width}`);
  const xs = points.map((p) => p[0]);
  for (let i = 1; i < xs.length; i++) if (!(xs[i] > xs[i - 1])) throw new Error(`keys: positions must increase, got ${xs[i - 1]} then ${xs[i]}`);
  const comps = (p: Key): readonly number[] => {
    const v = typeof p[1] === "number" ? [p[1]] : p[1];
    if (v.length !== width) throw new Error(`keys: every key needs ${width} components, key at ${p[0]} has ${v.length}`);
    return v;
  };
  const values = points.map(comps);
  const interp = INTERP[options.interp ?? "smooth"];
  const samples = new Float32Array(CURVE_N * 4);
  for (let c = 0; c < width; c++) {
    const ys = values.map((v) => v[c]);
    // A single key is a constant: the interpolants need two.
    for (let s = 0; s < CURVE_N; s++) samples[s * 4 + c] = xs.length === 1 ? ys[0] : interp(xs, ys, s / (CURVE_N - 1));
  }
  const r = new Row(samples, options.label ?? `keys(${points.map((p) => p[0]).join(",")})`);
  return (x) => row(r, x);
}

/** Evenly spaced colour stops, table-backed, interpolated linearly (no overshoot between colours). */
export function ramp(colours: readonly (readonly [number, number, number])[], label = "ramp"): Curve<"vec4"> {
  if (colours.length < 2) throw new Error("ramp: need at least two colours");
  return keys(colours.map((c, i) => [i / (colours.length - 1), c] as const), { interp: "linear", label });
}

// ---- closed-form splines ------------------------------------------------------------------------------------------

/**
 * Weight of the Catmull-Rom kernel (the Keys cubic with a = -1/2) at offset x. A uniform Catmull-Rom spline is the
 * sum of its control points weighted by this kernel at (tau - j), which is what lets `through` be one closed-form
 * expression even when the control points move.
 */
function kernel(x: Expr<"f32">): Expr<"f32"> {
  const a = abs(x), a2 = mul(a, a), a3 = mul(a2, a);
  const inner = add(sub(mul(a3, 1.5), mul(a2, 2.5)), 1);
  const outer = add(sub(add(mul(a3, -0.5), mul(a2, 2.5)), mul(a, 4)), 2);
  return select(lt(a, 1), inner, select(lt(a, 2), outer, 0));
}

/**
 * An interpolating cubic spline (uniform Catmull-Rom) through control points that may be expressions, at
 * parameter m in [0, 1] (clamped). The ends are mirrored (phantom points 2 p0 - p1 and 2 pn - pn-1, as in
 * `strand_kit::catmull_rom`), so the curve leaves its end points heading at the neighbouring point.
 */
export function through(m: In<"f32">, points: readonly In<"vec3">[]): Expr<"vec3"> {
  const n = points.length;
  if (n < 2) throw new Error(`through: need at least two control points, got ${n}`);
  const p = points.map((q) => lift<"vec3">(q));
  const me = lift<"f32">(m);
  const tau = mul(clamp(me, 0, 1), n - 1);
  const w = p.map((_, j) => kernel(sub(tau, j)) as Expr<"f32">);
  // Phantom points are linear in the real ones: fold their weights onto them.
  const before = kernel(add(tau, 1)), after = kernel(sub(tau, n));
  w[0] = add(w[0], mul(before, 2));
  w[1] = sub(w[1], before);
  w[n - 1] = add(w[n - 1], mul(after, 2));
  w[n - 2] = sub(w[n - 2], after);
  return p.map((q, j) => mul(q, w[j])).reduce((s, x) => add(s, x)) as Expr<"vec3">;
}

/**
 * Weight of the uniform cubic B-spline kernel at offset x: (4 - 6x^2 + 3|x|^3) / 6 within 1 of the centre,
 * (2 - |x|)^3 / 6 within 2, 0 beyond. Twice continuously differentiable, and the weights of the four kernels that
 * reach any point sum to 1.
 */
function bsplineKernel(x: Expr<"f32">): Expr<"f32"> {
  // |x| as a select, not `abs`: the derivative of abs at 0 is sign(0) = 0, so the second time derivative of the
  // curve read exactly at a knot came out 0 times the kernel's curvature there, not the curvature (found by solve's
  // probe, 2026-11-18). A select takes one branch's derivative at the tie, which is the right one-sided value.
  const a = select(lt(x, 0), neg(x), x), a2 = mul(a, a), a3 = mul(a2, a), out = sub(2, a);
  const inner = div(add(sub(4, mul(a2, 6)), mul(a3, 3)), 6);
  return select(lt(a, 1), inner, select(lt(a, 2), div(mul(mul(out, out), out), 6), 0));
}

/**
 * A curve of time through `values` as a uniform cubic B-spline over `over` = [t0, t1]: value j weights the curve
 * most at t0 + (j - 1) h, h = (t1 - t0) / (n - 3), and reaches 2 h either side of it. It is what `solve` returns, and
 * the same thing written by hand: edit a value and the curve moves near that time only, by at most 2/3 of the edit.
 *
 * The law. With tau = clamp((t - t0) / h, 0, n - 3), the curve is sum_j values[j] B(tau - j + 1), B the cubic B-spline
 * kernel. It is a cubic between neighbouring knots with continuous speed and acceleration (its jerk steps at knots),
 * linear in the values, and lies within the least and greatest of the four values that reach any time (the convex
 * hull: a bound on the whole curve read off its numbers). Outside [t0, t1] it holds its end value.
 *
 * Parameters: `t` (the clock; pass `time`) and every value take expressions; `over` and the count are fixed.
 *
 * What it cannot do, because of the law:
 * - pass through its values: it is near them, not on them (at t0 it is (v0 + 4 v1 + v2) / 6). `through` interpolates.
 * - corner or jump: a corner needs a knot repeated, which a uniform spline has no way to say.
 * - change speed at its ends: outside [t0, t1] it is constant, so a curve still moving at t1 stops there in one frame
 *   (hold its speed to 0 at the end, or end the span after the motion).
 * - fewer than four values.
 *
 * Relatives: `keys` (a table row, monotone between keys: edits are buffer writes); `through` (Catmull-Rom through
 * points, C1); `solve` (values chosen to meet stated holds).
 */
export function bspline(t: In<"f32">, over: readonly [number, number], values: readonly In<"f32">[]): Expr<"f32"> {
  const n = values.length;
  if (n < 4) throw new SceneError("bspline", `${n} values`, "at least 4", "add values: a cubic B-spline needs four to reach any time");
  const [t0, t1] = over;
  if (!(Number.isFinite(t0) && Number.isFinite(t1) && t1 > t0)) throw new SceneError("bspline", `over [${t0}, ${t1}]`, "a span [t0, t1] with t1 > t0", "give the times in increasing order");
  const h = (t1 - t0) / (n - 3);
  const tau = clamp(div(sub(t, t0), h), 0, n - 3);
  return values.map((v, j) => mul(lift<"f32">(v), bsplineKernel(sub(tau, j - 1)))).reduce((s, x) => add(s, x));
}

// ---- stateless motion operators -----------------------------------------------------------------------------------

export type WaveSpec = {
  /** Direction of the displacement. */
  readonly axis: In<"vec3">;
  /** Wavelengths along m in [0, 1]. */
  readonly waves: In<"f32">;
  /** Waves per second passing a point (toward increasing m). */
  readonly rate: In<"f32">;
  /** Peak displacement. Default 1. */
  readonly amplitude?: In<"f32">;
  /** Amplitude envelope over m, as a curve (its `.x`) or a number. Default 1. */
  readonly grow?: Curve<"vec4"> | In<"f32">;
  /** The clock. Default `time`; pass another to take a slice at a different time. */
  readonly t?: In<"f32">;
};

/** A travelling lateral wave: `axis * amplitude * grow(m) * sin(TAU (m waves - t rate))` (anguilliform swimming). */
export function wave(m: In<"f32">, spec: WaveSpec): Expr<"vec3"> {
  const me = lift<"f32">(m);
  const envelope: Expr<"f32"> = typeof spec.grow === "function" ? spec.grow(me).x : lift<"f32">(spec.grow ?? 1);
  const phase = mul(sub(mul(me, spec.waves), mul(spec.t ?? time, spec.rate)), TAU);
  return mul(lift<"vec3">(spec.axis), mul(mul(envelope, spec.amplitude ?? 1), sin(phase)));
}

/** Retarded time: the clock a part at m reads when a signal starts at m = 0 and runs at `speed` (m per second). */
export function retarded(t: In<"f32">, m: In<"f32">, speed: In<"f32">): Expr<"f32"> {
  return sub(t, div(m, speed));
}

export type ResponseSpec = {
  /** Natural frequency, Hz. */
  readonly f: In<"f32">;
  /** Damping ratio: below 1 overshoots. A number, because it selects the closed form. */
  readonly zeta: number;
  /** Initial response: below 0 moves the wrong way first (anticipation). Default 0. */
  readonly r?: In<"f32">;
};

/**
 * Unit-step response of a second-order system at time t after the step (0 for t <= 0), ported from
 * `strand_kit::step_response`. Being a formula of t, it eases with anticipation and overshoot without state, and
 * steps at different times superpose.
 */
export function response(t: In<"f32">, spec: ResponseSpec): Expr<"f32"> {
  // Both closed forms are exactly 0 at t = 0, so clamping t (instead of selecting) gives 0 before the step and keeps
  // exp() of a large positive argument out of the graph.
  const tt = max(lift<"f32">(t), 0);
  const w0 = mul(spec.f, TAU);
  const r = lift<"f32">(spec.r ?? 0);
  if (spec.zeta < 0.999) {
    const z = spec.zeta, s = Math.sqrt(1 - z * z);
    const e = exp(mul(mul(w0, -z), tt));
    const arg = mul(mul(w0, s), tt);
    const sn = sin(arg);
    return add(sub(1, mul(e, add(cos(arg), mul(sn, z / s)))), mul(mul(r, z / s), mul(e, sn)));
  }
  const z = Math.max(spec.zeta, 1.001), root = Math.sqrt(z * z - 1);
  const p1 = mul(w0, -z + root), p2 = mul(w0, -z - root);
  const e1 = exp(mul(p1, tt)), e2 = exp(mul(p2, tt));
  const gap = sub(p1, p2);
  const stepUp = add(1, sub(mul(p2, e1), mul(p1, e2)).div(gap));
  const impulse = mul(mul(w0, w0), sub(e1, e2)).div(gap);
  return add(stepUp, mul(mul(r, z), impulse).div(w0));
}

/**
 * 1 while t0 <= t < t1, else 0. With `ease` (seconds), the edges are smoothsteps of that width inside the window
 * instead of hard steps.
 */
export function pulse(t: In<"f32">, t0: In<"f32">, t1: In<"f32">, ease?: In<"f32">): Expr<"f32"> {
  if (ease === undefined) return mul(step(t0, t), sub(1, step(t1, t)));
  return mul(smoothstep(t0, add(t0, ease), t), sub(1, smoothstep(sub(t1, ease), t1, t)));
}

/** Smooth 0..1 as x goes from a to b. */
export const ease = (x: In<"f32">, a: In<"f32">, b: In<"f32">): Expr<"f32"> => smoothstep(a, b, x);
