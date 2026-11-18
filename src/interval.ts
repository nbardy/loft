// Interval evaluation of the expression graph: for a box of leaf values (time in [a, b], a param in [lo, hi]), a sound
// enclosure of every node's value over the whole box. What ranged checks (src/ranged.ts) are built on.
//
// Each component is held two ways at once, and its enclosure is where they overlap:
// - an interval [L, H], each operator's interval rule applied to its operands' enclosures;
// - an affine form c + g . e + [-r, r], with e in [-1, 1]^d the box's leaves scaled to [-1, 1] (reduced affine
//   arithmetic: every non-linear operator's error is folded into the one radius r). Linear operators are exact in it;
//   a smooth one is its tangent at the middle of its operand's enclosure, with the Taylor remainder (half a bound on
//   |f''| times the radius squared) added to r.
// The interval alone loses every correlation: a body's 48-step frame transport over a hundredth of a second came out
// as "any rotation" (a quaternion's hemisphere test went both ways). The affine form keeps first-order dependence on
// the leaves through the chain, so its excess width shrinks as the square of the box's; the interval keeps what the
// affine form cannot (a square is never negative, a unit vector's components lie in [-1, 1]).
//
// Bounds cover the CPU twin: f64 operators with outward rounding and f32 frame slots and table stores.
// Compiled table fills include the compiler's folded constants and intermediate stores. GPU operations may round
// differently. A free variable takes the value supplied directly to the CPU function.
//
// - `RangeRefusal`: the graph or range is unsupported. The decision reports missing evaluator capabilities as
//   undecided. Malformed domains are scene errors.
// - `Unbounded` (thrown, caught by the solver per box): over this box the operator's value is not bounded (a divisor
//   whose interval contains 0, the log of a value that may be 0). A smaller box may bound it; the solver splits, and
//   reports the operator by name if its width budget runs out first.
//
// Per-frame tables are read by inlining their programs: entry k of a `map` table is its run's closed form at k; entry
// k of a `scan` is its step unrolled k times from the start, as expressions. The CPU fill computes the same thing.

import { children, rawComputeOf, rawReasonOf, replace, CURVE_N, Expr, f, variable, type Fn, type LibFn, type Loop, type Node, type NumTy, type TableProgram, type TableSource } from "./expr.ts";
import type { Compiled } from "./compile/index.ts";
import { SceneError } from "./errors.ts";
import { recurTable } from "./map/recur.ts";
import { noise2Expr, noise3Expr } from "./grad.ts";

// ---- intervals ------------------------------------------------------------------------------------------------------

/**
 * Outward rounding. A bound computed by one IEEE operation is within half an ulp of the real result; JavaScriptCore's
 * Math.sin, cos, exp, log, atan2 and pow are fdlibm-derived and within one or two ulps. Padding every rounded bound by
 * 2^-48 of its magnitude (16 ulps) plus the least subnormal covers both, with room; it costs nothing measurable in
 * width (3.6e-15 relative).
 */
const EPS = 2 ** -48;
/**
 * A sum is exact when it is 0 or subnormal, so padding is relative there and 0 stays 0 (an absolute pad turned the
 * length of a vector with a zero component into the root of a negative). A product or quotient that underflows is the
 * one place an absolute pad is needed: `tiny` below.
 */
const SUBNORMAL = 2 ** -1022;
const down = (x: number): number => x - (Math.abs(x) * EPS + (x !== 0 && Math.abs(x) < SUBNORMAL ? Number.MIN_VALUE : 0));
const up = (x: number): number => x + (Math.abs(x) * EPS + (x !== 0 && Math.abs(x) < SUBNORMAL ? Number.MIN_VALUE : 0));
/** A product's or quotient's bounds, widened by the least subnormal where a bound underflowed to 0 from nonzero operands. */
const tiny = (r: I, underflow: boolean): I => (underflow ? [Math.min(r[0], -Number.MIN_VALUE), Math.max(r[1], Number.MIN_VALUE)] : r);

/** A bool over a box: true everywhere, false everywhere, or either. */
export type Tri = "true" | "false" | "either";

/** Why an enclosure over this box is not bounded: the operator, and what about its operand made it so. */
export class Unbounded extends Error {
  constructor(readonly op: string, readonly why: string) { super(`${op}: ${why}`); }
}

/** Why an expression has no enclosure at all. */
export type Refusal =
  | { readonly kind: "recur"; readonly table: string }
  | { readonly kind: "raw-compute"; readonly resource: string }
  | { readonly kind: "skin" }
  | { readonly kind: "dependent-end"; readonly leaf: string }
  /** `fn`: the function's name, or for raw WGSL text (src/text) why it is raw: "raw function pcg at eel.ts:14: u32 integers". */
  | { readonly kind: "gpu-only"; readonly fn: string }
  | { readonly kind: "no-enclosure"; readonly fn: string }
  | { readonly kind: "cpu-table"; readonly table: string; readonly why: string }
  | { readonly kind: "unbound"; readonly leaf: string }
  | { readonly kind: "equality"; readonly fn: "eq" | "ne" };

const refusalText = (r: Refusal): [string, string, string] => {
  switch (r.kind) {
    case "raw-compute": return [`raw compute "${r.resource}"`, "an expression with a CPU twin", "measure the resource in a render or express the condition with Loft maths"];
    case "recur": return [`the baked recurrence "${r.table}"`, "a closed-form scene", "measure the baked trajectory with a sampled check"];
    case "skin": return ["a skin target", "a condition over expressions", "measure the skin with a sampled contact check"];
    case "dependent-end": return [`a range end that reads the bound leaf ${r.leaf}`, "ends fixed throughout the search", "use fixed ends or parameterise the dependent interval over [0, 1]"];
    case "gpu-only": return [`the GPU-only function "${r.fn}"`, "functions with a CPU twin", "a ranged check cannot read what only the GPU computes; measure it in a picture"];
    case "no-enclosure": return [`the library function "${r.fn}", which has no interval enclosure`, "operators with enclosures (every built-in operator, the hashes and gradient noise)", "write it with built-in operators, or measure it with a sampled check"];
    case "cpu-table": return [`a read of the table "${r.table}" (${r.why})`, "tables filled by a map or a scan program", "a ranged check reads a table by inlining its program; a hand-written or baked fill has none: use a sampled check"];
    case "unbound": return [`a read of ${r.leaf}, which no range binds`, "every free variable and the time bound by always or eventually", `add a range for it: always([${r.leaf === "time" ? "time" : "leaf"}, lo, hi], ...)`];
    case "equality": return [`a comparison by ${r.fn}`, "ge, gt, le or lt (a margin is how far apart the two sides are)", `compare with a tolerance: and(ge(a, b - tol), le(a, b + tol))`];
  }
};

/** An expression a ranged check cannot enclose, whatever the box (exit 4: the check as written must change). */
export class RangeRefusal extends SceneError {
  constructor(where: string, readonly reason: Refusal) {
    const [given, expected, fix] = refusalText(reason);
    super(where, given, expected, fix);
  }
}

// ---- scalar operations on [lo, hi] -----------------------------------------------------------------------------------

/** An interval [lo, hi]. */
export type I = readonly [number, number];
function finite(lo: number, hi: number): void {
  if (!Number.isFinite(lo) || !Number.isFinite(hi)) throw new Unbounded("overflow", `non-finite enclosure [${lo}, ${hi}]`);
}
const hull = (a: I, b: I): I => [Math.min(a[0], b[0]), Math.max(a[1], b[1])];
const rounded = (lo: number, hi: number): I => [down(lo), up(hi)];
const iadd = (a: I, b: I): I => rounded(a[0] + b[0], a[1] + b[1]);
const isub = (a: I, b: I): I => rounded(a[0] - b[1], a[1] - b[0]);
function imul(a: I, b: I): I {
  const p = [a[0] * b[0], a[0] * b[1], a[1] * b[0], a[1] * b[1]];
  const zero = a[0] === 0 || a[1] === 0 || b[0] === 0 || b[1] === 0;
  return tiny(rounded(Math.min(...p), Math.max(...p)), !zero && p.some((x) => x === 0));
}
/** x * x for one operand: never negative (the general product of [-1, 1] by itself gives [-1, 1]). */
function isqr(a: I): I {
  const lo = a[0] >= 0 ? a[0] : a[1] <= 0 ? -a[1] : 0, hi = Math.max(-a[0], a[1]);
  // Never negative: a square that underflows is at least 0, and 0 only where the operand may be 0.
  return [Math.max(0, down(lo * lo)), Math.max(up(hi * hi), hi !== 0 ? Number.MIN_VALUE : 0)];
}
function idiv(a: I, b: I): I {
  if (b[0] <= 0 && b[1] >= 0) throw new Unbounded("div", `the divisor may be 0 (it lies in [${b[0]}, ${b[1]}])`);
  const q = [a[0] / b[0], a[0] / b[1], a[1] / b[0], a[1] / b[1]];
  return tiny(rounded(Math.min(...q), Math.max(...q)), a[0] !== 0 && a[1] !== 0 && q.some((x) => x === 0));
}
const ineg = (a: I): I => [-a[1], -a[0]];
const iabs = (a: I): I => (a[0] >= 0 ? a : a[1] <= 0 ? ineg(a) : [0, Math.max(-a[0], a[1])]);
const imin = (a: I, b: I): I => [Math.min(a[0], b[0]), Math.min(a[1], b[1])];
const imax = (a: I, b: I): I => [Math.max(a[0], b[0]), Math.max(a[1], b[1])];
function isqrt(a: I): I {
  if (a[0] < 0) throw new Unbounded("sqrt", `its operand may be negative (it lies in [${a[0]}, ${a[1]}])`);
  return [Math.max(0, down(Math.sqrt(a[0]))), up(Math.sqrt(a[1]))];
}
function iexp(a: I): I {
  const hi = Math.exp(a[1]);
  if (!Number.isFinite(hi)) throw new Unbounded("overflow", `exp overflows (its operand reaches ${a[1]})`);
  return [Math.max(0, down(Math.exp(a[0]))), Math.max(up(hi), Number.MIN_VALUE)];
}
function ilog(a: I): I {
  if (a[0] <= 0) throw new Unbounded("log", `its operand may be 0 or less (it lies in [${a[0]}, ${a[1]}])`);
  return rounded(Math.log(a[0]), Math.log(a[1]));
}
/** Whether some x = phase + k * period lies in [lo, hi], tested on [lo, hi] widened so a rounded multiple is not missed. */
function hits(lo: number, hi: number, phase: number, period: number): boolean {
  const w = (Math.abs(lo) + Math.abs(hi) + 1) * 1e-15;
  return Math.ceil((lo - w - phase) / period) <= Math.floor((hi + w - phase) / period);
}
const TWO_PI = 2 * Math.PI;
/** cos over [lo, hi]: the ends, and 1 or -1 where a crest or trough lies inside. */
function icos(a: I): I {
  if (a[1] - a[0] >= TWO_PI) return [-1, 1];
  const c0 = Math.cos(a[0]), c1 = Math.cos(a[1]);
  const lo = hits(a[0], a[1], Math.PI, TWO_PI) ? -1 : Math.max(-1, down(Math.min(c0, c1)));
  const hi = hits(a[0], a[1], 0, TWO_PI) ? 1 : Math.min(1, up(Math.max(c0, c1)));
  return [lo, hi];
}
function isin(a: I): I {
  if (a[1] - a[0] >= TWO_PI) return [-1, 1];
  const s0 = Math.sin(a[0]), s1 = Math.sin(a[1]);
  const lo = hits(a[0], a[1], -Math.PI / 2, TWO_PI) ? -1 : Math.max(-1, down(Math.min(s0, s1)));
  const hi = hits(a[0], a[1], Math.PI / 2, TWO_PI) ? 1 : Math.min(1, up(Math.max(s0, s1)));
  return [lo, hi];
}
function itan(a: I): I {
  if (a[1] - a[0] >= Math.PI || hits(a[0], a[1], Math.PI / 2, Math.PI)) throw new Unbounded("tan", `its operand may reach a pole (it lies in [${a[0]}, ${a[1]}])`);
  return rounded(Math.tan(a[0]), Math.tan(a[1]));
}
/** atan2(y, x): the corners, unless the box holds the origin or crosses the cut along negative x (then all angles). */
function iatan2(y: I, x: I): I {
  const origin = y[0] <= 0 && y[1] >= 0 && x[0] <= 0 && x[1] >= 0;
  const cut = x[0] < 0 && y[0] < 0 && y[1] >= 0;
  if (origin || cut) return [down(-Math.PI), up(Math.PI)];
  const c = [Math.atan2(y[0], x[0]), Math.atan2(y[0], x[1]), Math.atan2(y[1], x[0]), Math.atan2(y[1], x[1])];
  return rounded(Math.min(...c), Math.max(...c));
}
/** x^n for a whole number n, any sign of x (Math.pow's CPU value; WGSL leaves a negative base undefined). */
function ipowInt(x: I, n: number): I {
  if (n === 0) return [1, 1];
  if (n < 0) return idiv([1, 1], ipowInt(x, -n));
  const p0 = Math.pow(x[0], n), p1 = Math.pow(x[1], n);
  if (n % 2 === 1 || x[0] >= 0) return rounded(Math.min(p0, p1), Math.max(p0, p1));
  if (x[1] <= 0) return rounded(p1, p0);
  return [0, up(Math.max(p0, p1))];
}
/** pow(x, y): x^y over the box. A whole-number constant exponent takes any base; otherwise the base must be >= 0. */
function ipow(x: I, y: I): I {
  if (y[0] === y[1] && Number.isInteger(y[0])) return ipowInt(x, y[0]);
  if (x[0] < 0) throw new Unbounded("pow", `the base may be negative with a fractional exponent (base in [${x[0]}, ${x[1]}])`);
  if (x[0] === 0 && y[0] <= 0) throw new Unbounded("pow", `0 raised to a power that may be 0 or less`);
  // y ln x is extreme at the box's corners (a product of two intervals), so x^y is too.
  const c = [Math.pow(x[0], y[0]), Math.pow(x[0], y[1]), Math.pow(x[1], y[0]), Math.pow(x[1], y[1])];
  if (!c.every(Number.isFinite)) throw new Unbounded("overflow", "pow overflows");
  return [Math.max(0, down(Math.min(...c))), up(Math.max(...c))];
}
function ifloor(a: I): I { return [Math.floor(a[0]), Math.floor(a[1])]; }
function ifract(a: I): I {
  const k = Math.floor(a[0]);
  if (Math.floor(a[1]) !== k) return [0, 1];
  return [Math.max(0, down(a[0] - k)), Math.min(1, up(a[1] - k))];
}
/** Floored modulo a - b floor(a / b) (the sign of the divisor). */
function imod(a: I, b: I): I {
  if (b[0] <= 0 && b[1] >= 0) throw new Unbounded("mod", `the divisor may be 0 (it lies in [${b[0]}, ${b[1]}])`);
  const q = ifloor(idiv(a, b));
  if (q[0] === q[1]) return isub(a, imul(b, [q[0], q[0]]));
  // The rounded quotient and product can lose an ulp of the dividend before subtraction.
  const error = EPS * (maxAbs(a) + maxAbs(b)) + Number.MIN_VALUE;
  return rounded(Math.min(0, b[0]) - error, Math.max(0, b[1]) + error);
}
const istep = (e: I, x: I): I => (x[0] >= e[1] ? [1, 1] : x[1] < e[0] ? [0, 0] : [0, 1]);
const isat = (a: I): I => [Math.min(1, Math.max(0, a[0])), Math.min(1, Math.max(0, a[1]))];
const iclamp = (x: I, lo: I, hi: I): I => imin(hi, imax(lo, x));
/** a + (b - a) t, as the CPU twin computes it; for t inside [0, 1] near the hull of a and b. */
function imix(a: I, b: I, t: I): I {
  const r = iadd(a, imul(isub(b, a), t));
  if (t[0] < 0 || t[1] > 1) return r;
  const h = hull(a, b);
  // Cancellation at t=1 can lose the entire smaller endpoint.
  const error = EPS * maxAbs(h) + Number.MIN_VALUE;
  return [Math.max(r[0], down(h[0] - error)), Math.min(r[1], up(h[1] + error))];
}
/** smoothstep(lo, hi, x): t = clamp((x - lo) / (hi - lo), 0, 1), then t^2 (3 - 2t), which rises on [0, 1]. */
function ismooth(lo: I, hi: I, x: I): I {
  const t = isat(idiv(isub(x, lo), isub(hi, lo)));
  const s = (u: number): number => u * u * (3 - 2 * u);
  return [Math.max(0, down(s(t[0]))), Math.min(1, up(s(t[1])))];
}
const triOf = (yes: boolean, no: boolean): Tri => (yes ? "true" : no ? "false" : "either");
function icompare(fn: "lt" | "gt" | "le" | "ge", a: I, b: I): Tri {
  // Each test is a local of its own: Bun 1.3.10 parses `f(x[1] < y[0], x[0] >= y[1])` as type arguments
  // (`x[1]<y[0], x[0]>`) and passed (0, undefined), which made every comparison "either" (found 2026-11-18).
  switch (fn) {
    case "lt": { const yes = a[1] < b[0]; const no = a[0] >= b[1]; return triOf(yes, no); }
    case "le": { const yes = a[1] <= b[0]; const no = a[0] > b[1]; return triOf(yes, no); }
    case "gt": { const yes = a[0] > b[1]; const no = a[1] <= b[0]; return triOf(yes, no); }
    case "ge": { const yes = a[0] >= b[1]; const no = a[1] < b[0]; return triOf(yes, no); }
  }
}

// ---- tables, inlined --------------------------------------------------------------------------------------------------

/**
 * A table's entries as expressions, made on first read. A table's `program` may build new objects each time it is read
 * (a body's station table makes its samples table there), so it is read once per table.
 */
type Entries = { readonly program: TableProgram; readonly made: Map<number, readonly Expr<"vec4">[]> };
const inlined = new WeakMap<object, Entries>();
const ZERO4: readonly Expr<"vec4">[] = [];

function entries(t: TableSource, where: string, fills: Work["tableFills"]): Entries {
  const placed = fills?.get(t);
  const key = placed ?? t;
  const cached = inlined.get(key);
  if (cached) return cached;
  let program: TableProgram;
  if (placed) {
    // Lowering lifts shared expressions into additional f32 tables. Inline that same program.
    const p = placed.canon;
    switch (p.kind) {
      case "kept": case "replay": throw new RangeRefusal(where, { kind: "recur", table: t.name });
      case "cpu": throw new RangeRefusal(where, { kind: "cpu-table", table: t.name, why: p.why });
      case "map":
        program = { kind: "map", runs: p.runs.map((r) => ({ count: r.count, lanes: (i) => replace(r.lanes, new Map([[r.i.id, i]])) as Expr<"vec4">[] })) };
        break;
      case "scan":
        program = { ...p, step(state, i) {
          const by = new Map<number, Expr>(p.state.map((v, j) => [v.id, state[j]!]));
          by.set(p.i.id, i);
          const out = replace([...p.next, ...p.lanes], by);
          return { next: out.slice(0, state.length) as Expr<NumTy>[], lanes: out.slice(state.length) as Expr<"vec4">[] };
        } };
        break;
    }
  } else {
    if (recurTable(t) !== undefined) throw new RangeRefusal(where, { kind: "recur", table: t.name });
    if (!("program" in t)) throw new RangeRefusal(where, { kind: "cpu-table", table: t.name, why: t.role === "static" ? "filled once, at build" : "a hand-written fill" });
    program = t.program;
  }
  const result: Entries = { program, made: new Map() };
  inlined.set(key, result);
  return result;
}

function entryOf(t: TableSource, k: number, where: string, work: Work): readonly Expr<"vec4">[] {
  const e = entries(t, where, work.tableFills);
  const hit = e.made.get(k);
  if (hit !== undefined) return hit;
  const p = e.program;
  switch (p.kind) {
    case "cpu": throw new RangeRefusal(where, { kind: "cpu-table", table: t.name, why: p.why });
    case "map": {
      let start = 0;
      for (const r of p.runs) {
        if (k < start + r.count) { const lanes = r.lanes(f(k - start)); e.made.set(k, lanes); return lanes; }
        start += r.count;
      }
      throw new Unbounded("fetch", `entry ${k} of "${t.name}" is past its ${t.entries} entries`);
    }
    case "scan": {
      // Unrolled from the start once, keeping every entry (a station table is read at many of its entries).
      let state = p.init;
      for (let j = 0; j < p.steps; j++) {
        work.checkTime?.();
        const s = p.step(state, f(j));
        e.made.set(p.first + j, s.lanes);
        state = s.next;
      }
      // Entries outside the scan are never written: zero, as the fill leaves them.
      const got = e.made.get(k);
      if (got !== undefined) return got;
      e.made.set(k, ZERO4);
      return ZERO4;
    }
  }
}


// ---- affine forms -----------------------------------------------------------------------------------------------------
//
// One component over a box of d leaves: [c, g_1 .. g_d, r, L, H]. Its value at the point e in [-1, 1]^d is within r of
// c + g . e, and always within [L, H]. Rounding of every coefficient is charged to r (EPS of the magnitudes it was
// computed from), so the form stays sound in floating point.

/** One component's form; `K = d + 4` numbers. */
type C = Float64Array;
const R = (x: C): number => x.length - 3;
const spread = (x: C): number => { let s = x[R(x)]!; for (let i = 1; i < R(x); i++) s += Math.abs(x[i]!); return s; };
/** Magnitude a coefficient computation started from: |c| + |g|_1 + r. */
const size = (x: C): number => Math.abs(x[0]!) + spread(x);
/** The component's enclosure: the affine form's range, cut to [L, H]. */
export function bounds(x: C): I {
  const s = spread(x), k = x.length;
  const lo = Math.max(x[k - 2]!, down(x[0]! - s)), hi = Math.min(x[k - 1]!, up(x[0]! + s));
  finite(lo, hi);
  if (!(lo <= hi)) throw new Error(`internal: interval: an empty enclosure [${lo}, ${hi}]`);
  return [lo, hi];
}
const pointC = (d: number, x: number): C => { const c = new Float64Array(d + 4); c[0] = x; c[d + 2] = x; c[d + 3] = x; return c; };
/** A form that knows only an interval: its middle, and its half width as the radius. */
function fromI(d: number, i: I): C {
  finite(i[0], i[1]);
  const c = new Float64Array(d + 4), m = i[0] / 2 + i[1] / 2;
  c[0] = m; c[d + 1] = up(Math.max(i[1] - m, m - i[0])); c[d + 2] = i[0]; c[d + 3] = i[1];
  return c;
}
/**
 * The form `x` with the interval rule's result `iv` as its [L, H]. Where the affine part is twice as wide as `iv`, it is
 * replaced by `iv`'s own form: past a wide branch the interval knows more, and the form would carry the excess on.
 * Twice, not merely wider: a form exactly as wide as its interval (t - 1, a leaf shifted) is a little wider after its
 * rounding is charged, and replacing it there threw away every correlation with the leaves at the first operator.
 */
function finish(x: C, iv: I): C {
  finite(iv[0], iv[1]);
  const k = x.length;
  x[k - 2] = iv[0]; x[k - 1] = iv[1];
  if (spread(x) > iv[1] - iv[0]) return fromI(k - 4, iv);
  return x;
}
/** A Float32Array store rounds monotonically. Keep its exact endpoint image and charge half an ulp to the affine error. */
function stored(x: C): C {
  const [lo, hi] = bounds(x), a = Math.fround(lo), b = Math.fround(hi);
  if (!Number.isFinite(a) || !Number.isFinite(b)) throw new Unbounded("overflow", "the value exceeds f32 storage");
  if (a === b) return pointC(x.length - 4, a);
  const c = x.slice();
  c[R(c)] = up(c[R(c)]! + Math.max(2 ** -150, Math.max(Math.abs(lo), Math.abs(hi)) * 2 ** -24));
  return finish(c, [a, b]);
}
/** ka a + kb b + k0 (b may be absent): exact in the form, rounding charged to r. */
function lin(a: C, ka: number, b: C | undefined, kb: number, k0: number): C {
  const k = a.length, r = k - 3, x = new Float64Array(k);
  let err = Math.abs(ka) * size(a) + Math.abs(k0);
  for (let i = 0; i < r; i++) x[i] = ka * a[i]! + (b === undefined ? 0 : kb * b[i]!);
  x[0] = x[0]! + k0;
  x[r] = Math.abs(ka) * a[r]! + (b === undefined ? 0 : Math.abs(kb) * b[r]!);
  if (b !== undefined) err += Math.abs(kb) * size(b);
  x[r] = up(x[r]! + err * EPS);
  // No interval known yet: `finish` gives one. (Left at 0, it cut every unfinished form to [0, 0].)
  x[k - 2] = -Infinity; x[k - 1] = Infinity;
  return x;
}
/** a b: (ca + ga e +- ra)(cb + gb e +- rb) = ca cb + (ca gb + cb ga) e, with |ga e +- ra| |gb e +- rb| into r. */
function mulC(a: C, b: C): C {
  const k = a.length, r = k - 3, x = new Float64Array(k), ca = a[0]!, cb = b[0]!;
  x[0] = ca * cb;
  let ga = a[r]!, gb = b[r]!, err = Math.abs(ca * cb);
  for (let i = 1; i < r; i++) {
    x[i] = ca * b[i]! + cb * a[i]!;
    ga += Math.abs(a[i]!); gb += Math.abs(b[i]!);
    err += Math.abs(ca * b[i]!) + Math.abs(cb * a[i]!);
  }
  x[r] = up(Math.abs(ca) * b[r]! + Math.abs(cb) * a[r]! + ga * gb + err * EPS);
  x[k - 2] = -Infinity; x[k - 1] = Infinity;
  return x;
}
/** Any value of a or of b (a select whose condition goes both ways): their mean, give or take half their difference. */
function hullC(a: C, b: C): C {
  const k = a.length, r = k - 3, x = new Float64Array(k);
  let half = Math.abs(a[0]! - b[0]!) / 2, err = size(a) + size(b);
  x[0] = (a[0]! + b[0]!) / 2;
  for (let i = 1; i < r; i++) { x[i] = (a[i]! + b[i]!) / 2; half += Math.abs(a[i]! - b[i]!) / 2; }
  x[r] = up(a[r]! + b[r]! + half + err * EPS);
  const ia = bounds(a), ib = bounds(b);
  return finish(x, hull(ia, ib));
}
/** min (sign -1) or max (sign 1) of a and b: the one that is wholly below (above) the other, else (a + b) / 2 -+ |a - b| / 2. */
function extremeC(a: C, b: C, sign: 1 | -1): C {
  const ia = bounds(a), ib = bounds(b);
  const iv = sign < 0 ? imin(ia, ib) : imax(ia, ib);
  if (sign < 0 ? ia[1] <= ib[0] : ia[0] >= ib[1]) return finish(Float64Array.from(a), iv);
  if (sign < 0 ? ib[1] <= ia[0] : ib[0] >= ia[1]) return finish(Float64Array.from(b), iv);
  // |a - b| / 2 lies in [0, D / 2]: the mean, moved by D / 4 toward the extreme, give or take D / 4.
  const diff = bounds(lin(a, 1, b, -1, 0)), D = Math.max(Math.abs(diff[0]), Math.abs(diff[1]));
  const x = lin(a, 0.5, b, 0.5, sign * D / 4), r = x.length - 3;
  x[r] = up(x[r]! + D / 4);
  return finish(x, iv);
}
/**
 * A smooth function of one operand: its tangent at the middle m of the operand's enclosure X, plus the Taylor remainder
 * (half a bound on |f''| over X, times the radius of X squared). `iv` is the interval rule's result.
 */
type Smooth = { readonly f: (m: number) => number; readonly df: (m: number) => number; readonly d2: (X: I) => number };
function smoothC(a: C, X: I, s: Smooth, iv: I): C {
  const d = a.length - 4, r = d + 1;
  const m = X[0] / 2 + X[1] / 2, rad = up(Math.max(X[1] - m, m - X[0]));
  const bound = s.d2(X), fm = s.f(m), alpha = s.df(m);
  const rem = up(0.5 * bound * rad * rad);
  if (!Number.isFinite(rem) || !Number.isFinite(fm) || !Number.isFinite(alpha)) return fromI(d, iv);
  // f(m) + alpha (a - m)
  const x = lin(a, alpha, undefined, 0, fm - alpha * m);
  x[r] = up(x[r]! + rem + (Math.abs(fm) + Math.abs(alpha * m)) * EPS);
  return finish(x, iv);
}
const maxAbs = (i: I): number => Math.max(Math.abs(i[0]), Math.abs(i[1]));
const SQR: Smooth = { f: (m) => m * m, df: (m) => 2 * m, d2: () => 2 };
const RECIP: Smooth = { f: (m) => 1 / m, df: (m) => -1 / (m * m), d2: (X) => 2 / Math.min(Math.abs(X[0]), Math.abs(X[1])) ** 3 };
const SQRT: Smooth = { f: Math.sqrt, df: (m) => 0.5 / Math.sqrt(m), d2: (X) => 0.25 * X[0] ** -1.5 };
const EXP: Smooth = { f: Math.exp, df: Math.exp, d2: (X) => Math.exp(X[1]) };
const LOG: Smooth = { f: Math.log, df: (m) => 1 / m, d2: (X) => 1 / (X[0] * X[0]) };
const SIN: Smooth = { f: Math.sin, df: Math.cos, d2: (X) => maxAbs(isin(X)) };
const COS: Smooth = { f: Math.cos, df: (m) => -Math.sin(m), d2: (X) => maxAbs(icos(X)) };
const TAN: Smooth = { f: Math.tan, df: (m) => 1 + Math.tan(m) ** 2, d2: (X) => { const t = maxAbs(itan(X)); return 2 * t * (1 + t * t); } };
/** |atan''(x)| = 2|x| / (1 + x^2)^2 is at most 3 sqrt(3) / 8 < 0.65 everywhere. */
const ATAN: Smooth = { f: Math.atan, df: (m) => 1 / (1 + m * m), d2: () => 0.65 };
const iatan = (a: I): I => rounded(Math.atan(a[0]), Math.atan(a[1]));
const powSmooth = (p: number): Smooth => ({
  f: (m) => Math.pow(m, p), df: (m) => p * Math.pow(m, p - 1),
  d2: (X) => Math.abs(p * (p - 1)) * maxAbs(Number.isInteger(p) ? ipowInt(X, p - 2) : [Math.pow(X[0], p - 2), Math.pow(X[1], p - 2)]),
});

// ---- the CPU twin's arithmetic, for sub-graphs that read no leaf of the box -----------------------------------------
//
// Such a node's operands are points, and it is computed as the JavaScript backend computes it (src/expr.ts, EACH), in
// f64 without padding. Padding there would turn a structural constant that is exactly a whole number (a station's
// index, k / span * span) into an interval across it, and `floor` of that into two entries of a table, at every box
// however small: an enclosure splitting cannot tighten.

const roundEvenTwin = (x: number): number => { const r = Math.round(x); return r - x === 0.5 && r % 2 !== 0 ? r - 1 : r; };
const smoothTwin = (lo: number, hi: number, x: number): number => { const t = Math.min(1, Math.max(0, (x - lo) / (hi - lo))); return t * t * (3 - 2 * t); };
const TWIN: Partial<Record<Fn, (...a: number[]) => number>> = {
  add: (a, b) => a! + b!, sub: (a, b) => a! - b!, mul: (a, b) => a! * b!, div: (a, b) => a! / b!, min: Math.min, max: Math.max,
  pow: Math.pow, atan2: Math.atan2, mod: (a, b) => a! - b! * Math.floor(a! / b!), step: (e, x) => (x! >= e! ? 1 : 0), neg: (a) => -a!,
  abs: Math.abs, sign: Math.sign, floor: Math.floor, sqrt: Math.sqrt, exp: Math.exp, log: Math.log, sin: Math.sin, cos: Math.cos, tan: Math.tan,
  fract: (a) => a! - Math.floor(a!), saturate: (a) => Math.min(1, Math.max(0, a!)), mix: (a, b, t) => a! + (b! - a!) * t!,
  clamp: (x, lo, hi) => Math.min(hi!, Math.max(lo!, x!)), smoothstep: smoothTwin,
  ceil: Math.ceil, trunc: Math.trunc, roundEven: (a) => roundEvenTwin(a!), rem: (a, b) => a! % b!, inverseSqrt: (a) => 1 / Math.sqrt(a!),
  exp2: (a) => Math.pow(2, a!), log2: Math.log2, asin: Math.asin, acos: Math.acos, atan: Math.atan, sinh: Math.sinh, cosh: Math.cosh, tanh: Math.tanh,
};
const twinDot = (a: readonly number[], b: readonly number[]): number => a.reduce((s, x, i) => (i === 0 ? x * b[0]! : s + x * b[i]!), 0);
function twin(fn: Fn, xs: readonly (readonly number[])[], width: number): number[] {
  switch (fn) {
    case "length": return [Math.sqrt(twinDot(xs[0]!, xs[0]!))];
    case "distance": { const d = xs[0]!.map((x, i) => x - xs[1]![i]!); return [Math.sqrt(twinDot(d, d))]; }
    case "dot": return [twinDot(xs[0]!, xs[1]!)];
    case "cross": { const [x, y] = [xs[0]!, xs[1]!]; return [x[1]! * y[2]! - x[2]! * y[1]!, x[2]! * y[0]! - x[0]! * y[2]!, x[0]! * y[1]! - x[1]! * y[0]!]; }
    case "normalize": { const v = xs[0]!; if (v.length === 1) return [Math.sign(v[0]!)]; const l = Math.sqrt(twinDot(v, v)); return v.map((x) => x / l); }
    default: {
      const op = TWIN[fn];
      if (op === undefined) throw new Error(`internal: interval: no twin rule for "${fn}"`);
      return Array.from({ length: width }, (_, i) => op(...xs.map((x) => (x.length === 1 ? x[0]! : x[i]!))));
    }
  }
}

// ---- library functions ------------------------------------------------------------------------------------------------

/**
 * The hashes are constant inside each integer cell, so over a box spanning few cells the exact hull is the hash at each
 * cell; past CELLS cells, their codomain [0, 1 - 2^-24]. Gradient noise is enclosed cell by cell by its expression form
 * (src/grad.ts: the same cells, gradients and interpolation): within one cell with the operand's own form, across cells
 * over the box clipped to each cell; past CELLS cells, its codomain [-1, 1] (the scale noise.ts gives it makes 1 its
 * maximum). Every other library function is refused by name.
 */
const CELLS = 64;
const HASH_TOP = 1 - 2 ** -24;
type LibRule = (args: readonly Float64Array[], ev: Evaluator) => Float64Array;
/** Every integer cell the enclosures `b` (`dims` components) cover, or undefined past CELLS. */
function cells(b: readonly I[]): number[][] | undefined {
  const lo = b.map((x) => Math.floor(x[0])), hi = b.map((x) => Math.floor(x[1]));
  let count = 1;
  for (let i = 0; i < b.length; i++) count *= hi[i]! - lo[i]! + 1;
  if (count > CELLS) return undefined;
  let out: number[][] = [[]];
  for (let i = 0; i < b.length; i++) out = out.flatMap((c) => Array.from({ length: hi[i]! - lo[i]! + 1 }, (_, k) => [...c, lo[i]! + k]));
  return out;
}
function hashRule(fn: LibFn, dims: number, outs: number): LibRule {
  const js = fn.js as (p: number[]) => number | number[];
  return (args, ev) => {
    const cs = cells(ev.split(args[0]!).map(bounds).slice(0, dims));
    const lo = new Array<number>(outs).fill(Infinity), hi = new Array<number>(outs).fill(-Infinity);
    if (cs === undefined) { ev.work.codomain++; lo.fill(0); hi.fill(HASH_TOP); }
    else for (const c of cs) {
      const v = js(c), vs = typeof v === "number" ? [v] : v;
      vs.forEach((x, i) => { lo[i] = Math.min(lo[i]!, x); hi[i] = Math.max(hi[i]!, x); });
    }
    return ev.join(lo.map((l, i) => fromI(ev.d, [l, hi[i]!])));
  };
}
function noiseRule(dims: 2 | 3, asExpr: (p: Expr<NumTy>) => Expr<NumTy>): LibRule {
  const p = variable(`_interval_noise${dims}`, dims === 2 ? "vec2" : "vec3");
  let body: Expr | undefined;
  return (args, ev) => {
    const b = ev.split(args[0]!).map(bounds);
    const cs = cells(b);
    if (cs === undefined) { ev.work.codomain++; return ev.join([fromI(ev.d, [-1, 1])]); }
    body ??= asExpr(p as Expr<NumTy>);
    if (cs.length === 1) return ev.sub(body, p, args[0]!);
    let lo = Infinity, hi = -Infinity;
    for (const c of cs) {
      // The box clipped to the cell: there the noise is a polynomial in the point with fixed corner gradients.
      const clip = ev.join(b.map((x, i) => fromI(ev.d, [Math.max(x[0], c[i]!), Math.min(x[1], c[i]! + 1 - 1e-12)])));
      const v = bounds(ev.split(ev.sub(body, p, clip))[0]!);
      lo = Math.min(lo, v[0]); hi = Math.max(hi, v[1]);
    }
    return ev.join([fromI(ev.d, [Math.max(-1, lo), Math.min(1, hi)])]);
  };
}
const libRules = new Map<string, LibRule>();
/** Registered lazily: noise.ts's LibFns are reached through the expressions that call them. */
function libRule(fn: LibFn, where: string): LibRule {
  const hit = libRules.get(fn.name);
  if (hit !== undefined) return hit;
  if (fn.js === "gpu-only") throw new RangeRefusal(where, { kind: "gpu-only", fn: rawReasonOf(fn) ?? fn.name });
  let rule: LibRule;
  switch (fn.name) {
    case "loft_hash2": rule = hashRule(fn, 2, 1); break;
    case "loft_hash3": rule = hashRule(fn, 3, 1); break;
    case "loft_hash22": rule = hashRule(fn, 2, 2); break;
    case "loft_hash33": rule = hashRule(fn, 3, 3); break;
    case "loft_noise2": rule = noiseRule(2, noise2Expr); break;
    case "loft_noise3": rule = noiseRule(3, noise3Expr); break;
    default: throw new RangeRefusal(where, { kind: "no-enclosure", fn: fn.name });
  }
  libRules.set(fn.name, rule);
  return rule;
}

// ---- the evaluator ----------------------------------------------------------------------------------------------------

/**
 * What a box binds: a leaf, keyed `var:name`, `input:name` or `param:name`, to its interval, or to a line in the box's
 * other leaves (`Track`), in a fixed order.
 */
export type Box = ReadonlyMap<string, I | Track>;
/**
 * A leaf that follows others across the box: centre + sum of slope (leaf - its middle) over the leaves named. A ranged
 * check's witness for "for every speed there is a time" over a box of speeds is such a line: the touchdown time moves
 * with the speed, and one fixed time held the condition over a box of speeds only a hundredth wide.
 */
export type Track = { readonly centre: number; readonly slopes: ReadonlyMap<string, number> };
const isTrack = (b: I | Track): b is Track => !Array.isArray(b);
/** How many selects one enclosure may split cases on (2^CASES evaluations at most). */
const CASES = 4;
/** Counters a solver reports as its work. */
export type Work = { tableFills?: Compiled["tableFills"]; nodes: number; codomain: number; checkTime?: () => void; charge?: () => void };
/** A node's value over a box: a number's components as forms laid end to end, or a bool. */
type Val = { readonly kind: "num"; readonly v: Float64Array } | { readonly kind: "bool"; readonly v: Tri };

const comps = (ty: string): number => (ty === "f32" ? 1 : ty === "vec2" ? 2 : ty === "vec3" ? 3 : 4);
type Held = (n: Extract<Node, { op: "var" | "input" | "param" }>) => readonly number[] | undefined;

/**
 * Encloses expressions over one box. `held` gives every leaf the box does not bind (inputs at their held values, params
 * at their defaults); a leaf neither binds is refused. One evaluator per box: its memo holds that box's values.
 */
export class Evaluator {
  readonly d: number;
  private readonly names: readonly string[];
  private memo = new Map<number, Val>();
  /** The nodes evaluated so far that read a leaf of the box (the rest are points, computed as the twin computes them). */
  private readonly deps = new Set<number>();
  /** Selects whose condition went both ways here (not forced): the cases `scalar` may split on. */
  private readonly eithers: number[] = [];
  constructor(private readonly box: Box, private readonly held: Held, readonly where: string, readonly work: Work,
    /** Selects taken one way regardless of their condition: a case of a case split (`scalar`). */
    private readonly forced: ReadonlyMap<number, boolean> = new Map()) {
    this.names = [...box].filter(([, b]) => !isTrack(b)).map(([k]) => k);
    this.d = this.names.length;
  }

  /** The enclosure of each component of a numeric expression. */
  enclose(e: Expr): I[] { return this.split(this.numeric(e)).map(bounds); }
  /**
   * An f32 expression's enclosure, and its first-order sensitivity to each leaf of the box (|g_k|: how much of its width
   * leaf k accounts for), which a solver uses to choose the leaf to split.
   */
  scalar(e: Expr): { readonly at: I; readonly sensitivity: readonly number[] } {
    try {
      const c = this.split(this.numeric(e))[0]!;
      return { at: bounds(c), sensitivity: this.names.map((_, k) => Math.abs(c[k + 1]!)) };
    } catch (err) {
      // Split the cases of a select that went both ways: at every point of the box the value is that of one case, so
      // the hull of the two cases' results holds it. Taken only when the hull of the two branches was unbounded: a
      // body's quaternion chain flips sign where its first station's w crosses 0 (q and -q are one rotation), and the
      // hull of q and -q holds 0, so every later nlerp's normalize was unbounded at every box holding that instant.
      const id = this.eithers[0];
      if (!(err instanceof Unbounded) || id === undefined || this.forced.size >= CASES) throw err;
      const cases = [true, false].map((way) => {
        this.work.charge?.();
        return new Evaluator(this.box, this.held, this.where, this.work, new Map([...this.forced, [id, way]])).scalar(e);
      });
      return { at: hull(cases[0]!.at, cases[1]!.at), sensitivity: cases[0]!.sensitivity.map((x, k) => Math.max(x, cases[1]!.sensitivity[k]!)) };
    }
  }
  bool(e: Expr): Tri {
    const v = this.value(e);
    if (v.kind !== "bool") throw new Error(`internal: interval: expected a bool (node ${e.id})`);
    return v.v;
  }

  /** The components of a value, as views. */
  split(v: Float64Array): C[] {
    const k = this.d + 4;
    return Array.from({ length: v.length / k }, (_, i) => v.subarray(i * k, (i + 1) * k));
  }
  join(cs: readonly C[]): Float64Array {
    const k = this.d + 4, out = new Float64Array(cs.length * k);
    cs.forEach((c, i) => out.set(c, i * k));
    return out;
  }
  /** `body` with the variable `v` bound to `val` (a library rule's private expression form), in a memo of its own. */
  sub(body: Expr, v: Expr, val: Float64Array): Float64Array {
    const saved = this.memo;
    this.memo = new Map([[v.id, { kind: "num", v: val }]]);
    this.deps.add(v.id);
    try { return this.numeric(body); } finally { this.memo = saved; }
  }

  numeric(e: Expr): Float64Array {
    const v = this.value(e);
    if (v.kind !== "num") throw new Error(`internal: interval: expected a number, got a bool (node ${e.id})`);
    return v.v;
  }
  private value(e: Expr): Val {
    const hit = this.memo.get(e.id);
    if (hit !== undefined) return hit;
    if ((this.work.nodes & 127) === 0) this.work.checkTime?.();
    const v = this.node(e);
    this.work.nodes++;
    this.memo.set(e.id, v);
    return v;
  }
  private dep(e: Expr): boolean { return this.deps.has(e.id); }
  private point(xs: readonly number[]): Float64Array {
    if (!xs.every(Number.isFinite)) throw new Unbounded("overflow", `non-finite point [${xs}]`);
    return this.join(xs.map((x) => pointC(this.d, x)));
  }
  /** The centres of a value read by no leaf: its exact components. */
  private centres(v: Float64Array): number[] { return this.split(v).map((c) => c[0]!); }

  private node(e: Expr): Val {
    const n = e.node;
    switch (n.op) {
      case "const": return n.ty === "bool" ? { kind: "bool", v: n.v[0] ? "true" : "false" } : { kind: "num", v: this.point(n.v) };
      case "var": case "input": case "param": {
        const name = n.op === "var" ? n.name : n.spec.name, key = `${n.op}:${name}`;
        const bound = this.box.get(key);
        if (bound !== undefined) {
          if (!isTrack(bound) && bound[0] === bound[1]) return { kind: "num", v: this.point([n.op === "var" ? bound[0] : Math.fround(bound[0])]) };
          this.deps.add(e.id);
          // With independent axes, range over the image of the f32 store itself. Every use of a
          // clock then shares one affine coordinate. A track still follows the pre-store coordinate.
          if (!isTrack(bound) && this.d === this.box.size && n.op !== "var") {
            const lo = Math.fround(bound[0]), hi = Math.fround(bound[1]);
            if (!Number.isFinite(lo) || !Number.isFinite(hi)) throw new Unbounded("overflow", "the value exceeds f32 storage");
            return { kind: "num", v: this.leaf(this.names.indexOf(key), [lo, hi]) };
          }
          const c = isTrack(bound) ? this.track(bound) : this.leaf(this.names.indexOf(key), bound);
          return { kind: "num", v: n.op === "var" ? c : stored(c) };
        }
        const held = this.held(n);
        if (held === undefined) throw new RangeRefusal(this.where, { kind: "unbound", leaf: n.op === "input" && name === "time" ? "time" : `${n.op} "${name}"` });
        return { kind: "num", v: this.point(held) };
      }
      case "row": return { kind: "num", v: this.row(e, n.row.samples, n.x) };
      case "fetch": return { kind: "num", v: this.fetch(e, n.table, n.lane, n.index) };
      case "vec": {
        const parts = n.args.map((a) => this.numeric(a));
        if (n.args.some((a) => this.dep(a))) this.deps.add(e.id);
        if (parts.length === 1 && parts[0]!.length === this.d + 4) return { kind: "num", v: this.join(Array.from({ length: comps(n.ty) }, () => parts[0]!)) };
        const out = new Float64Array(parts.reduce((s, p) => s + p.length, 0));
        let o = 0;
        for (const p of parts) { out.set(p, o); o += p.length; }
        return { kind: "num", v: out };
      }
      case "get": {
        const a = this.split(this.numeric(n.a));
        if (this.dep(n.a)) this.deps.add(e.id);
        return { kind: "num", v: this.join([...n.sel].map((ch) => a["xyzw".indexOf(ch)]!)) };
      }
      case "lib": {
        const raw = rawComputeOf(n.fn);
        if (raw !== undefined) throw new RangeRefusal(this.where, { kind: "raw-compute", resource: raw });
        const args = n.args.map((a) => this.numeric(a));
        if (!n.args.some((a) => this.dep(a))) {
          if (n.fn.js === "gpu-only") throw new RangeRefusal(this.where, { kind: "gpu-only", fn: n.fn.name });
          const r = n.fn.js(...args.map((a) => { const c = this.centres(a); return c.length === 1 ? c[0] : c; }));
          return { kind: "num", v: this.point(typeof r === "number" ? [r] : typeof r === "boolean" ? [r ? 1 : 0] : r) };
        }
        this.deps.add(e.id);
        return { kind: "num", v: libRule(n.fn, this.where)(args, this) };
      }
      case "iterate": {
        // A loop reads the box only through its count, its start and what its body reads from outside.
        const out = this.loop(n.loop)[n.k]!;
        if (children(e).some((c) => this.dep(c))) this.deps.add(e.id);
        return { kind: "num", v: out };
      }
      case "call": {
        const v = this.call(e, n);
        // A bool or a select reads the box when an operand it evaluated does (an operand left unevaluated did not matter).
        if (n.args.some((a) => this.dep(a))) this.deps.add(e.id);
        return v;
      }
    }
  }

  /** A tracking leaf: exact in the forms (each slope times the half width of the leaf it follows). */
  private track(t: Track): Float64Array {
    const c = new Float64Array(this.d + 4);
    c[0] = t.centre;
    let mag = Math.abs(t.centre);
    for (const [key, slope] of t.slopes) {
      const k = this.names.indexOf(key);
      if (k < 0) throw new Error(`internal: interval: a track follows "${key}", which the box does not range over`);
      const [lo, hi] = this.box.get(key) as I;
      c[k + 1] = slope * (hi / 2 - lo / 2);
      mag += Math.abs(c[k + 1]!) + Math.abs(slope) * (Math.abs(lo) + Math.abs(hi));
    }
    c[this.d + 1] = mag * EPS;
    c[this.d + 2] = -Infinity; c[this.d + 3] = Infinity;
    const b = bounds(c);
    c[this.d + 2] = b[0]; c[this.d + 3] = b[1];
    return c;
  }

  /** Leaf k of the box: its middle, plus half its width times e_k. */
  private leaf(k: number, [lo, hi]: I): Float64Array {
    const c = new Float64Array(this.d + 4), m = lo / 2 + hi / 2;
    c[0] = m; c[k + 1] = hi / 2 - lo / 2; c[this.d + 1] = (Math.abs(lo) + Math.abs(hi)) * EPS; c[this.d + 2] = lo; c[this.d + 3] = hi;
    return c;
  }

  /** A piecewise-linear curve row at x in [0, 1] (clamped): linear within one segment, else the samples' hull. */
  private row(e: Expr, samples: Float32Array, xe: Expr): Float64Array {
    const xv = this.numeric(xe);
    const sample = (i: number, c: number): number => samples[i * 4 + c]!;
    if (!this.dep(xe)) {
      const s = Math.min(1, Math.max(0, xv[0]!)) * (CURVE_N - 1), i = Math.min(CURVE_N - 2, Math.floor(s)), t = s - i;
      return this.point([0, 1, 2, 3].map((c) => sample(i, c) * (1 - t) + sample(i + 1, c) * t));
    }
    this.deps.add(e.id);
    const x = this.split(xv)[0]!, X = bounds(x);
    const s0 = Math.min(1, Math.max(0, X[0])) * (CURVE_N - 1), s1 = Math.min(1, Math.max(0, X[1])) * (CURVE_N - 1);
    const i0 = Math.min(CURVE_N - 2, Math.floor(s0)), i1 = Math.min(CURVE_N - 2, Math.floor(s1));
    return this.join([0, 1, 2, 3].map((c) => {
      const at = (s: number): number => { const i = Math.min(CURVE_N - 2, Math.floor(s)), t = s - i; return sample(i, c) * (1 - t) + sample(i + 1, c) * t; };
      let lo = Math.min(at(s0), at(s1)), hi = Math.max(at(s0), at(s1));
      for (let k = Math.floor(s0) + 1; k < s1; k++) { lo = Math.min(lo, sample(k, c)); hi = Math.max(hi, sample(k, c)); }
      const iv: I = [down(lo), up(hi)];
      // Within one segment and inside [0, 1] the row is a line in x.
      if (i0 !== i1 || X[0] < 0 || X[1] > 1) return fromI(this.d, iv);
      const slope = (sample(i0 + 1, c) - sample(i0, c)) * (CURVE_N - 1);
      return finish(lin(x, slope, undefined, 0, sample(i0, c) - slope * i0 / (CURVE_N - 1)), iv);
    }));
  }

  /** Entry trunc(index) of a table, lane `lane`: the hull over every entry the index's enclosure covers. */
  private fetch(e: Expr, t: TableSource, lane: number, indexExpr: Expr): Float64Array {
    const index = bounds(this.split(this.numeric(indexExpr))[0]!);
    if (this.dep(indexExpr)) this.deps.add(e.id);
    const k0 = Math.trunc(index[0]), k1 = Math.trunc(index[1]);
    if (k0 < 0 || k1 >= t.entries) throw new Unbounded("fetch", `the index of "${t.name}" may leave its ${t.entries} entries (it lies in [${index[0]}, ${index[1]}])`);
    let out: C[] | undefined;
    for (let k = k0; k <= k1; k++) {
      const lanes = entryOf(t, k, this.where, this.work);
      const v = lanes === ZERO4 ? this.point([0, 0, 0, 0]) : this.numeric(lanes[lane]!);
      if (lanes !== ZERO4 && this.dep(lanes[lane]!)) this.deps.add(e.id);
      const cs = this.split(v).map(stored);
      out = out === undefined ? cs : out.map((c, i) => hullC(c, cs[i]!));
    }
    return this.join(out!);
  }

  /**
   * A loop with enclosed state. Each step encloses every path's state at that step (as if every path ran on); a path
   * that may end at a step (`until` is "either", or the count may end there) adds that step's state to the exits.
   */
  private loop(l: Loop): Float64Array[] {
    const count = bounds(this.split(this.numeric(l.count))[0]!);
    const n0 = Math.max(0, Math.trunc(count[0])), n1 = Math.max(0, Math.trunc(count[1]));
    let state = l.init.map((x) => this.numeric(x));
    const stateDep = l.init.map((x) => this.dep(x));
    let exits: Float64Array[] | undefined;
    const exit = (s: readonly Float64Array[]): void => {
      exits = exits === undefined ? s.map((x) => x) : exits.map((x, j) => this.join(this.split(x).map((c, i) => hullC(c, this.split(s[j]!)[i]!))));
    };
    if (n0 === 0) exit(state);
    // The body's nodes (`inner`) take new values each step and are forgotten after it; what the body reads from
    // outside keeps the box's values.
    const forget = (): void => {
      for (const x of l.inner) { this.memo.delete(x.id); this.deps.delete(x.id); }
      for (const x of l.state) { this.memo.delete(x.id); this.deps.delete(x.id); }
      this.memo.delete(l.index.id);
    };
    try {
      for (let i = 0; i < n1; i++) {
        if ((i & 127) === 0) this.work.checkTime?.();
        forget();
        l.state.forEach((v, j) => { this.memo.set(v.id, { kind: "num", v: state[j]! }); if (stateDep[j]) this.deps.add(v.id); });
        this.memo.set(l.index.id, { kind: "num", v: this.point([i]) });
        state = l.next.map((x) => this.numeric(x));
        l.next.forEach((x, j) => { stateDep[j] = stateDep[j]! || this.dep(x); });
        const stop: Tri = l.stop === undefined ? "false" : this.bool(l.stop);
        if (stop === "true") { exit(state); return exits!; }
        if (stop === "either" || i + 1 >= n0) exit(state);
      }
    } finally { forget(); }
    return exits ?? state;
  }

  private call(e: Expr, n: Extract<Node, { op: "call" }>): Val {
    const fn = n.fn;
    switch (fn) {
      case "and": case "or": {
        const a = this.bool(n.args[0]!);
        // Short-circuit where the first decides (the second may be unbounded where it does not matter).
        if (fn === "and" && a === "false") return { kind: "bool", v: "false" };
        if (fn === "or" && a === "true") return { kind: "bool", v: "true" };
        const b = this.bool(n.args[1]!);
        if (fn === "and") return { kind: "bool", v: a === "true" ? b : b === "false" ? "false" : "either" };
        return { kind: "bool", v: a === "false" ? b : b === "true" ? "true" : "either" };
      }
      case "not": { const a = this.bool(n.args[0]!); return { kind: "bool", v: a === "true" ? "false" : a === "false" ? "true" : "either" }; }
      case "lt": case "gt": case "le": case "ge": {
        const a = bounds(this.split(this.numeric(n.args[0]!))[0]!), b = bounds(this.split(this.numeric(n.args[1]!))[0]!);
        return { kind: "bool", v: icompare(fn, a, b) };
      }
      case "eq": case "ne": {
        const a = bounds(this.split(this.numeric(n.args[0]!))[0]!), b = bounds(this.split(this.numeric(n.args[1]!))[0]!);
        const same = a[0] === a[1] && b[0] === b[1] && a[0] === b[0], apart = a[1] < b[0] || b[1] < a[0];
        return { kind: "bool", v: fn === "eq" ? triOf(same, apart) : triOf(apart, same) };
      }
      case "select": {
        // Only the branch the condition takes is evaluated: the other may be unbounded where it is not taken.
        const way = this.forced.get(e.id), width = comps(e.ty);
        const c: Tri = way === undefined ? this.bool(n.args[0]!) : way ? "true" : "false";
        if (c === "either") this.eithers.push(e.id);
        const branch = (x: Expr): C[] => { const cs = this.split(this.numeric(x)); return cs.length === width ? cs : Array.from({ length: width }, () => cs[0]!); };
        if (c === "true") return { kind: "num", v: this.join(branch(n.args[1]!)) };
        if (c === "false") return { kind: "num", v: this.join(branch(n.args[2]!)) };
        const y = branch(n.args[1]!), m = branch(n.args[2]!);
        return { kind: "num", v: this.join(y.map((a, i) => hullC(a, m[i]!))) };
      }
      default: {
        const a = n.args.map((x) => this.numeric(x)), width = comps(e.ty);
        if (!n.args.some((x) => this.dep(x))) {
          const r = twin(fn, a.map((x) => this.centres(x)), width);
          if (!r.every(Number.isFinite)) throw new Unbounded("overflow", `${fn}: a sub-graph that reads no range is not finite (${r.join(", ")})`);
          return { kind: "num", v: this.point(r) };
        }
        return { kind: "num", v: this.join(arith(fn, n.args, a.map((x) => this.split(x)), width, this.d)) };
      }
    }
  }
}

// ---- operators on forms ---------------------------------------------------------------------------------------------

/** Apply a rule component by component; a one-component operand is used for every component. */
function each(args: readonly (readonly C[])[], width: number, rule: (...xs: C[]) => C): C[] {
  return Array.from({ length: width }, (_, i) => rule(...args.map((a) => (a.length === 1 ? a[0]! : a[i]!))));
}
/** A rule known only as an interval rule on the operands' enclosures (piecewise operators). */
const byInterval = (d: number, rule: (...xs: I[]) => I) => (...xs: C[]): C => fromI(d, rule(...xs.map(bounds)));
const addC = (a: C, b: C): C => finish(lin(a, 1, b, 1, 0), iadd(bounds(a), bounds(b)));
const subC = (a: C, b: C): C => finish(lin(a, 1, b, -1, 0), isub(bounds(a), bounds(b)));
const timesC = (a: C, b: C): C => finish(mulC(a, b), imul(bounds(a), bounds(b)));
const sqrC = (a: C): C => { const X = bounds(a); return smoothC(a, X, SQR, isqr(X)); };
function recipC(b: C, op: string): C {
  const X = bounds(b);
  if (X[0] <= 0 && X[1] >= 0) throw new Unbounded(op, `the divisor may be 0 (it lies in [${X[0]}, ${X[1]}])`);
  return smoothC(b, X, RECIP, idiv([1, 1], X));
}
const divC = (a: C, b: C): C => finish(mulC(a, recipC(b, "div")), idiv(bounds(a), bounds(b)));
function sqrtC(a: C, op: string): C {
  const X = bounds(a);
  if (X[0] < 0) throw new Unbounded(op, `its operand may be negative (it lies in [${X[0]}, ${X[1]}])`);
  return X[0] === 0 ? fromI(a.length - 4, isqrt(X)) : smoothC(a, X, SQRT, isqrt(X));
}
/** x . x: never negative, kept as an interval beside the form (a length's root needs it). */
function sumSqC(v: readonly C[]): C {
  let acc = sqrC(v[0]!);
  for (let i = 1; i < v.length; i++) acc = addC(acc, sqrC(v[i]!));
  return acc;
}
function dotC(a: readonly C[], b: readonly C[]): C {
  let acc = timesC(a[0]!, b[0]!);
  for (let i = 1; i < a.length; i++) acc = addC(acc, timesC(a[i]!, b[i]!));
  return acc;
}
function powC(a: C, b: C): C {
  const X = bounds(a), Y = bounds(b), iv = ipow(X, Y);
  if (Y[0] !== Y[1]) return fromI(a.length - 4, iv);
  const p = Y[0];
  if (p === 0) return fromI(a.length - 4, [1, 1]);
  if (p === 1) return finish(Float64Array.from(a), iv);
  if (!Number.isInteger(p) && X[0] <= 0) return fromI(a.length - 4, iv);
  return smoothC(a, X, powSmooth(p), iv);
}
function atan2C(y: C, x: C): C {
  const Y = bounds(y), X = bounds(x), iv = iatan2(Y, X);
  const atanOf = (q: C): C => { const Q = bounds(q); return smoothC(q, Q, ATAN, iatan(Q)); };
  // atan2 as atan of a quotient where the box keeps clear of the axis it divides by.
  if (X[0] > 0) return finish(atanOf(divC(y, x)), iv);
  if (Y[0] > 0) return finish(lin(atanOf(divC(x, y)), -1, undefined, 0, Math.PI / 2), iv);
  if (Y[1] < 0) return finish(lin(atanOf(divC(x, y)), -1, undefined, 0, -Math.PI / 2), iv);
  return fromI(y.length - 4, iv);
}
function absC(a: C): C {
  const X = bounds(a);
  if (X[0] >= 0) return finish(Float64Array.from(a), X);
  if (X[1] <= 0) return finish(lin(a, -1, undefined, 0, 0), ineg(X));
  return fromI(a.length - 4, iabs(X));
}
/** fract and mod are a shift of their operand where its whole part is fixed over the box. */
function fractC(a: C): C {
  const X = bounds(a), k = Math.floor(X[0]);
  return Math.floor(X[1]) === k ? finish(lin(a, 1, undefined, 0, -k), ifract(X)) : fromI(a.length - 4, ifract(X));
}
function modC(a: C, b: C): C {
  const X = bounds(a), Y = bounds(b), iv = imod(X, Y);
  if (Y[0] !== Y[1]) return fromI(a.length - 4, iv);
  const q = ifloor(idiv(X, Y));
  return q[0] === q[1] ? finish(lin(a, 1, undefined, 0, -Y[0] * q[0]), iv) : fromI(a.length - 4, iv);
}
const clampC = (x: C, lo: C, hi: C): C => extremeC(extremeC(x, lo, 1), hi, -1);
function mixC(a: C, b: C, t: C): C {
  const r = addC(a, timesC(subC(b, a), t));
  return finish(r, imix(bounds(a), bounds(b), bounds(t)));
}
function smoothstepC(lo: C, hi: C, x: C): C {
  const d = x.length - 4;
  const t0 = divC(subC(x, lo), subC(hi, lo));
  const t = extremeC(extremeC(t0, pointC(d, 0), 1), pointC(d, 1), -1);
  // 3 t^2 - 2 t^3
  const t2 = sqrC(t), t3 = timesC(t2, t);
  return finish(lin(t2, 3, t3, -2, 0), ismooth(bounds(lo), bounds(hi), bounds(x)));
}
const unitC = (Smooth: Smooth, rule: (x: I) => I) => (a: C): C => { const X = bounds(a); return smoothC(a, X, Smooth, rule(X)); };

/** A numeric built-in operator over the forms `a` of its operands `args` (each a list of components). */
function arith(fn: Fn, args: readonly Expr[], a: readonly (readonly C[])[], width: number, d: number): C[] {
  switch (fn) {
    case "length": return [sqrtC(sumSqC(a[0]!), "length")];
    case "distance": return [sqrtC(sumSqC(each([a[0]!, a[1]!], a[0]!.length, subC)), "distance")];
    case "dot": return [args[0] === args[1] ? sumSqC(a[0]!) : dotC(a[0]!, a[1]!)];
    case "cross": {
      const [x, y] = [a[0]!, a[1]!];
      const term = (i: number, j: number): C => subC(timesC(x[i]!, y[j]!), timesC(x[j]!, y[i]!));
      return [term(1, 2), term(2, 0), term(0, 1)];
    }
    case "normalize": {
      const v = a[0]!;
      if (v.length === 1) return [byInterval(d, isign)(v[0]!)];
      const l = sqrtC(sumSqC(v), "normalize");
      if (bounds(l)[0] <= 0) throw new Unbounded("normalize", "the vector may be zero");
      const inv = recipC(l, "normalize"), L = bounds(l);
      return v.map((x) => { const q = idiv(bounds(x), L); return finish(mulC(x, inv), [Math.max(-1, q[0]), Math.min(1, q[1])]); });
    }
    // x * x: one operand, so the product is never negative.
    case "mul": return args[0] === args[1] ? each([a[0]!], width, sqrC) : each(a, width, timesC);
    case "add": return each(a, width, addC);
    case "sub": return each(a, width, subC);
    case "div": return each(a, width, divC);
    case "min": return each(a, width, (x, y) => extremeC(x, y, -1));
    case "max": return each(a, width, (x, y) => extremeC(x, y, 1));
    case "pow": return each(a, width, powC);
    case "mod": return each(a, width, modC);
    case "atan2": return each(a, width, atan2C);
    case "step": return each(a, width, byInterval(d, istep));
    case "neg": return each(a, width, (x) => finish(lin(x, -1, undefined, 0, 0), ineg(bounds(x))));
    case "abs": return each(a, width, absC);
    case "sign": return each(a, width, byInterval(d, isign));
    case "floor": return each(a, width, byInterval(d, ifloor));
    case "fract": return each(a, width, fractC);
    case "sqrt": return each(a, width, (x) => sqrtC(x, "sqrt"));
    case "exp": return each(a, width, unitC(EXP, iexp));
    case "log": return each(a, width, (x) => { const X = bounds(x); return smoothC(x, X, LOG, ilog(X)); });
    case "sin": return each(a, width, unitC(SIN, isin));
    case "cos": return each(a, width, unitC(COS, icos));
    case "tan": return each(a, width, unitC(TAN, itan));
    case "saturate": return each(a, width, (x) => clampC(x, pointC(d, 0), pointC(d, 1)));
    case "mix": return each(a, width, mixC);
    case "clamp": return each(a, width, clampC);
    case "smoothstep": return each(a, width, smoothstepC);
    // WGSL's own operators (src/text). Monotone ones are their ends; the rest are bounded by their shape.
    case "ceil": return each(a, width, byInterval(d, (x) => [Math.ceil(x[0]), Math.ceil(x[1])]));
    case "trunc": return each(a, width, byInterval(d, (x) => [Math.trunc(x[0]), Math.trunc(x[1])]));
    case "roundEven": return each(a, width, byInterval(d, (x) => [roundEvenTwin(x[0]), roundEvenTwin(x[1])]));
    case "rem": return each(a, width, byInterval(d, irem));
    case "inverseSqrt": return each(a, width, (x) => {
      const X = bounds(x);
      if (X[0] <= 0) throw new Unbounded("inverseSqrt", `its operand may be 0 or less (it lies in [${X[0]}, ${X[1]}])`);
      return smoothC(x, X, RSQRT, rounded(1 / Math.sqrt(X[1]), 1 / Math.sqrt(X[0])));
    });
    case "exp2": return each(a, width, (x) => { const X = bounds(x); return smoothC(x, X, EXP2, ipow2(X)); });
    case "log2": return each(a, width, (x) => { const X = bounds(x); return smoothC(x, X, LOG2, ilog2(X)); });
    case "asin": return each(a, width, byInterval(d, (x) => iarc(x, "asin", Math.asin)));
    case "acos": return each(a, width, byInterval(d, (x) => iarc(x, "acos", Math.acos)));
    case "atan": return each(a, width, unitC(ATAN, iatan));
    case "sinh": return each(a, width, unitC(SINH, (x) => rounded(Math.sinh(x[0]), Math.sinh(x[1]))));
    case "cosh": return each(a, width, unitC(COSH, icosh));
    case "tanh": return each(a, width, unitC(TANH, (x) => [Math.max(-1, down(Math.tanh(x[0]))), Math.min(1, up(Math.tanh(x[1])))]));
    default: throw new Error(`internal: interval: "${fn}" reached the arithmetic rules`);
  }
}

const isign = (a: I): I => [Math.sign(a[0]), Math.sign(a[1])];

/** WGSL's float `%`: a - b trunc(a / b), which has a's sign and is smaller than |b|. Exact shift where trunc(a / b) is fixed. */
function irem(a: I, b: I): I {
  if (b[0] <= 0 && b[1] >= 0) throw new Unbounded("rem", `the divisor may be 0 (it lies in [${b[0]}, ${b[1]}])`);
  const q = idiv(a, b), tq: I = [Math.trunc(q[0]), Math.trunc(q[1])];
  if (tq[0] === tq[1]) return isub(a, imul(b, tq));
  // |a % b| is below |b| and at most |a|.
  const m = maxAbs(b) * (1 + 4 * EPS);
  return [Math.max(Math.min(0, a[0]), -m), Math.min(Math.max(0, a[1]), m)];
}
function ipow2(x: I): I {
  const hi = Math.pow(2, x[1]);
  if (!Number.isFinite(hi)) throw new Unbounded("overflow", `exp2 overflows (its operand reaches ${x[1]})`);
  return [Math.max(0, down(Math.pow(2, x[0]))), Math.max(up(hi), Number.MIN_VALUE)];
}
function ilog2(x: I): I {
  if (x[0] <= 0) throw new Unbounded("log2", `its operand may be 0 or less (it lies in [${x[0]}, ${x[1]}])`);
  return rounded(Math.log2(x[0]), Math.log2(x[1]));
}
/** asin or acos over [lo, hi] inside [-1, 1], as the function's values at the ends (each monotone). */
function iarc(x: I, op: string, f: (v: number) => number): I {
  if (x[0] < -1 || x[1] > 1) throw new Unbounded(op, `its operand may leave [-1, 1] (it lies in [${x[0]}, ${x[1]}])`);
  const p = f(x[0]), q = f(x[1]);
  return rounded(Math.min(p, q), Math.max(p, q));
}
function icosh(x: I): I {
  const p = Math.cosh(x[0]), q = Math.cosh(x[1]);
  if (!Number.isFinite(p) || !Number.isFinite(q)) throw new Unbounded("overflow", "cosh overflows");
  return x[0] <= 0 && x[1] >= 0 ? [1, up(Math.max(p, q))] : rounded(Math.min(p, q), Math.max(p, q));
}
const LN2 = Math.LN2;
/** 1 / sqrt(x): |f''| = 0.75 x^-2.5, largest at the low end. */
const RSQRT: Smooth = { f: (m) => 1 / Math.sqrt(m), df: (m) => -0.5 * m ** -1.5, d2: (X) => 0.75 * X[0] ** -2.5 };
const EXP2: Smooth = { f: (m) => Math.pow(2, m), df: (m) => LN2 * Math.pow(2, m), d2: (X) => LN2 * LN2 * Math.pow(2, X[1]) };
const LOG2: Smooth = { f: Math.log2, df: (m) => 1 / (m * LN2), d2: (X) => 1 / (X[0] * X[0] * LN2) };
/** |sinh''| = |sinh| and |cosh''| = cosh, largest at the end farther from 0. */
const SINH: Smooth = { f: Math.sinh, df: Math.cosh, d2: (X) => Math.sinh(maxAbs(X)) };
const COSH: Smooth = { f: Math.cosh, df: Math.sinh, d2: (X) => Math.cosh(maxAbs(X)) };
/** |tanh''(x)| = 2 |tanh x| (1 - tanh^2 x) is at most 4 / (3 sqrt 3) < 0.77 everywhere. */
const TANH: Smooth = { f: Math.tanh, df: (m) => 1 - Math.tanh(m) ** 2, d2: () => 0.77 };
