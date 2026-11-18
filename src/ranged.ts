// Ranged checks: a condition that holds for every time in [a, b] and every value of a param in [lo, hi], decided by
// enclosing the condition over whole boxes of those leaves (src/interval.ts) rather than at sampled frames, graded by
// how far it is from failing (the margin), with the point where it comes nearest (the witness).
//
// The condition is written with Loft's own comparisons, `ge(a, b)` and the rest, combined with `and`, `or` and `not`.
// Its margin is their robustness (signal temporal logic, Donze and Maler 2010): `ge(a, b)` is a - b, `le(a, b)` is
// b - a, `and` the least of its two, `or` the greater, `not` the negation; `always` is the least over its range and
// `eventually` the greatest. At zero margin the comparison itself decides truth.
//
// Each quantifier is a branch and bound over its box: the box whose enclosure could hold the extreme is split (along
// the leaf its value is most sensitive to), a box that cannot is dropped, and the middle of every box is evaluated as
// a point to bound the extreme from the other side. A run of quantifiers of one kind is one search over their joint
// box; a quantifier inside another of the other kind is a search of its own for each box or point the outer one asks
// about, cut short for a box (a few splits give a valid but loose enclosure) and run to the precision for a point.

import { Expr, everything, lift, max, min, neg, sub, variable, type In } from "./expr.ts";
import { SceneError, siteOf } from "./errors.ts";
import { Evaluator, RangeRefusal, Unbounded, type Tri, type Refusal, type Box, type I, type Track, type Work } from "./interval.ts";
import type { Check, Probe, Verdict } from "./check.ts";

/** A leaf and the range it takes: `[time, 0, 4]`, `[param("crab.reach", 1), 0.8, 1.2]`. */
export type Range = readonly [Expr<"f32">, In<"f32">, In<"f32">];

/** Truth of the quantified condition, bounds on its extreme, and the best point observed. */
export type Decision = {
  /** The quantified boolean result. An incomplete proof is undecided. */
  readonly holds: true | false | "undecided";
  /** Margin evaluated at the witness. NaN when the search has no point evaluation. */
  readonly margin: number;
  /** Proven bounds on the true margin. */
  readonly bounds: I;
  /** The best sampled assignment, keyed by leaf name (time as "time"). Empty before a point is found. */
  readonly witness: Readonly<Record<string, number>>;
  readonly work: { readonly boxes: number; readonly nodes: number; readonly ms: number; readonly codomain: number };
  /** Why the search stopped short of `precision`, or "finished". */
  readonly spent: Spent;
};
/** How a search ended. */
export type Spent =
  | { readonly kind: "finished" }
  | Extract<Refusal, { kind: "gpu-only" | "no-enclosure" | "cpu-table" | "recur" | "raw-compute" | "skin" }>
  | { readonly kind: "ms"; readonly ms: number }
  | { readonly kind: "unbounded"; readonly op: string; readonly why: string }
  /** The box budget ran out. */
  | { readonly kind: "boxes"; readonly boxes: number }
  /** The boxes that could hold the extreme could not be split further; `op` is why they stayed wide, if an operator was unbounded on them. */
  | { readonly kind: "width"; readonly width: number; readonly op: string | undefined }
  /** The condition reads a measured quantity (`measured`), which has no enclosure: `run` samples it instead. */
  | { readonly kind: "measured"; readonly what: string };

/** A ranged check: a `Check` for `loft check`, and `decide(probe)` for the numbers. */
export interface Ranged extends Check {
  readonly formula: Quantified;
  /** The line the `always` or `eventually` was written on (undefined when no line outside the library is on the stack). */
  readonly site: string | undefined;
  /** Proved over the range (`ranged`), or, for a condition over a measured quantity, sampled over time (`sampled`). */
  run(probe: Probe): Extract<Verdict, { kind: "ranged" | "sampled" }>;
  decide(probe: Probe): Decision;
}

/** A bound leaf: `key` is `input:time`, `param:<name>`, `input:<name>` or `var:<name>`. */
export type Leaf = { readonly key: string; readonly label: string; readonly lo: Expr<"f32">; readonly hi: Expr<"f32"> };
/** A quantified condition: what `solve` reads of an `always` it is given as a hold. */
export type Quantified = { readonly kind: "always" | "eventually"; readonly leaves: readonly Leaf[]; readonly body: Body; readonly label: string };
type Condition = Expr<"bool"> | Ranged;
type Body = { readonly kind: "atom"; readonly condition: Expr<"bool">; readonly margin: Expr<"f32"> } | { readonly kind: "nested"; readonly q: Quantified };

/** Search settings. */
export type Settings = {
  /** A name for the check (default: the quantifiers and their ranges). */
  readonly label?: string;
  /** How close the margin must be bounded, in its own units (default 1e-4). */
  readonly precision?: number;
  /** How many boxes all searches together may evaluate (default 20000). */
  readonly boxes?: number;
  /** Wall-clock milliseconds for one decision, shared by all nested searches. */
  readonly ms?: number;
};

// ---- measured quantities ------------------------------------------------------------------------------------------

/** A quantity the CPU measures at an instant by a search, not an expression (a body's clearance from a skin). */
type Measure = { readonly what: string; readonly name: string; readonly at: (probe: Probe, t: number) => number };
// Keyed by the leaf a measure stands in for in expressions: a variable of its own, never bound by a range.
const measures = new WeakMap<Expr, Measure>();
let measureCount = 0;

/**
 * A quantity measured on the CPU at each instant (`at`), standing in an expression as a leaf of its own, so it is
 * compared with the same vocabulary as any expression. It has no enclosure: a condition reading it is sampled at
 * `SAMPLE_RATE` over its time range, and its verdict is `sampled` ("no failure at N samples"), never a proof. Library
 * use: `clearance` from a skin.
 */
export function measured(what: string, at: Measure["at"]): Expr<"f32"> {
  const name = `loft.measured.${++measureCount}`, leaf = variable(name, "f32");
  measures.set(leaf, { what, name, at });
  return leaf;
}

/** The measured quantities an expression reads (classified once, where a check or a hold is built). */
export function measuredIn(roots: readonly Expr[]): Measure[] {
  return everything(roots).flatMap((e) => measures.get(e) ?? []);
}

/** Samples per second of a condition over a measured quantity: the rate the sampled checks it replaces used. */
const SAMPLE_RATE = 60;

// ---- the authoring surface ----------------------------------------------------------------------------------------

/**
 * The condition holds for every value of the range's leaf in [lo, hi]: `always([time, 0, 4], ge(gap, 0.1))`. The leaf
 * is `time`, an `input`, a `param` or a free `variable`; the condition is a comparison (`ge`, `gt`, `le`, `lt`) or
 * several joined by `and`, `or`, `not`, or another `always` or `eventually`. Its margin is the least of the
 * condition's margin over the range, found within `precision`, with the leaf values where it is least.
 * cannot: compare with `eq` or `ne` (no margin); read a GPU-only function, a cellular noise, or a table filled on the
 *   CPU (refused by name when the check runs).
 * The same formulas are `solve`'s holds, and a measure (`clearance`) is a quantity to compare: `always([time, 1, 3],
 * ge(clearance(tail, ship), 0.1))` is proved over every t, where a sampled check only fails to find a failure.
 * relatives: eventually, clearance, equal.
 */
export function always(range: Range, condition: Condition, o: Settings = {}): Ranged {
  return ranged("always", range, condition, o);
}

/**
 * The condition holds for some value of the range's leaf in [lo, hi]: `eventually([time, 0, 3], le(height, 0))`.
 * Its margin is the greatest of the condition's margin over the range; the witness is where it is greatest.
 * relatives: always.
 */
export function eventually(range: Range, condition: Condition, o: Settings = {}): Ranged {
  return ranged("eventually", range, condition, o);
}

function leafOf(kind: string, range: Range): Leaf {
  const [e, lo, hi] = range;
  if (!(e instanceof Expr) || e.ty !== "f32") throw new SceneError(kind, `a range over ${e instanceof Expr ? `a ${e.ty}` : typeof e}`, "a range over an f32 leaf: [time, 0, 4]", `${kind}([time, 0, 4], ...)`);
  const n = e.node;
  switch (n.op) {
    case "var": return { key: `var:${n.name}`, label: n.name, lo: lift<"f32">(lo), hi: lift<"f32">(hi) };
    case "input": case "param": return { key: `${n.op}:${n.spec.name}`, label: n.spec.name, lo: lift<"f32">(lo), hi: lift<"f32">(hi) };
    default: throw new SceneError(kind, `a range over a computed expression (${n.op})`, "a range over a leaf: time, an input, a param or a variable", `bind the leaf the expression is built from, e.g. ${kind}([time, 0, 4], ...)`);
  }
}

/**
 * The margin of a comparison and its combinations: how far the condition is from failing (negative: by how much it
 * fails). Classified once, here.
 */
function marginOf(kind: string, c: Expr<"bool">): Expr<"f32"> {
  const n = c.node;
  if (n.op !== "call") throw new SceneError(kind, `a condition that is a ${n.op === "const" ? `constant ${n.v[0] ? "true" : "false"}` : n.op}`, "a comparison: ge, gt, le or lt (or several joined by and, or, not)", `${kind}(range, ge(a, b))`);
  const [a, b] = n.args as Expr[];
  switch (n.fn) {
    case "ge": case "gt": return sub(a as Expr<"f32">, b as Expr<"f32">);
    case "le": case "lt": return sub(b as Expr<"f32">, a as Expr<"f32">);
    case "and": return min(marginOf(kind, a as Expr<"bool">), marginOf(kind, b as Expr<"bool">));
    case "or": return max(marginOf(kind, a as Expr<"bool">), marginOf(kind, b as Expr<"bool">));
    case "not": return neg(marginOf(kind, a as Expr<"bool">));
    case "eq": case "ne": throw new SceneError(kind, `a comparison by ${n.fn}`, "ge, gt, le or lt (a margin is how far apart the two sides are)", "compare with a tolerance: and(ge(a, b - tol), le(a, b + tol))");
    default: throw new SceneError(kind, `a condition built by ${n.fn}`, "a comparison: ge, gt, le or lt (or several joined by and, or, not)", `${kind}(range, ge(a, b))`);
  }
}

const shownRange = (l: Leaf): string => `${l.label} in [${show(l.lo)}, ${show(l.hi)}]`;
const show = (e: Expr<"f32">): string => (e.node.op === "const" ? String(e.node.v[0]) : "(expr)");

function ranged(kind: "always" | "eventually", range: Range, condition: Condition, o: Settings): Ranged {
  const site = siteOf(new Error().stack);
  const leaf = leafOf(kind, range);
  let body: Body;
  if (condition instanceof Expr) body = { kind: "atom", condition, margin: marginOf(kind, condition) };
  else if (typeof condition === "object" && condition !== null && "formula" in condition) body = { kind: "nested", q: condition.formula };
  else throw new SceneError(kind, "an unknown condition", "a comparison or a quantified condition", `${kind}(range, ge(a, b))`);
  // A run of one kind is one search over the joint box.
  const merged = body.kind === "nested" && body.q.kind === kind ? body.q : undefined;
  const leaves = merged ? [leaf, ...merged.leaves] : [leaf];
  const inner = merged?.body ?? body;
  const keys = new Set<string>();
  const ends: Expr[] = [];
  const check = (q: { leaves: readonly Leaf[]; body: Body }): void => {
    for (const l of q.leaves) {
      if (keys.has(l.key)) throw new SceneError(kind, `${l.label} bound twice`, "each leaf bound by one range", `drop one of the two ranges over ${l.label}`);
      keys.add(l.key);
      ends.push(l.lo, l.hi);
    }
    if (q.body.kind === "nested") check(q.body.q);
  };
  check({ leaves, body: inner });
  // Collect the full set first: an outer range can bind a leaf read by an inner end, or vice versa.
  for (const { node } of everything(ends)) {
    if (node.op === "var" || node.op === "input" || node.op === "param") {
      const key = `${node.op}:${node.op === "var" ? node.name : node.spec.name}`;
      if (keys.has(key)) throw new RangeRefusal(kind, { kind: "dependent-end", leaf: key });
    }
  }
  const label = `${kind} ${leaves.map(shownRange).join(", ")}${inner.kind === "nested" ? `: ${inner.q.label}` : ""}`;
  const formula: Quantified = { kind, leaves, body: inner, label };
  const precision = o.precision ?? 1e-4, boxes = o.boxes ?? 20000;
  if (!(precision > 0)) throw new SceneError(kind, `precision ${precision}`, "a precision above 0", `${kind}(range, condition, { precision: 1e-4 })`);
  if (!Number.isSafeInteger(boxes) || boxes < 0) throw new SceneError(kind, `boxes ${boxes}`, "a nonnegative integer", "set boxes to the maximum number of enclosures");
  if (o.ms !== undefined && !(Number.isFinite(o.ms) && o.ms >= 0)) throw new SceneError(kind, `ms ${o.ms}`, "a finite nonnegative duration", "give the time budget in milliseconds");
  const name = o.label ?? label;
  const sampled = inner.kind === "atom" ? measuredIn([inner.condition]) : [];
  if (sampled.length > 0 && (leaves.length !== 1 || leaf.key !== "input:time" || inner.kind !== "atom"))
    throw new SceneError(kind, `a condition over ${sampled[0]!.what} (measured, not an expression) quantified over ${leaves.map((l) => l.label).join(", ")}`,
      "a measured quantity under one range over time, which is sampled", `${kind}([time, a, b], ge(...))`);
  if (sampled.length > 0 && inner.kind === "atom") {
    const what = sampled.map((m) => m.what).join(", ");
    return {
      name, formula, site,
      decide: () => ({ holds: "undecided", margin: NaN, bounds: [-Infinity, Infinity], witness: {}, spent: { kind: "measured", what }, work: { boxes: 0, nodes: 0, ms: 0, codomain: 0 } }),
      run: (probe) => sampleRun(kind, leaf, inner.condition, inner.margin, sampled, probe, name, what),
    };
  }
  return {
    name, formula, site,
    decide: (probe) => decide(formula, probe, name, precision, boxes, o.ms, keys),
    run(probe) {
      const d = decide(formula, probe, name, precision, boxes, o.ms, keys);
      const at = Object.entries(d.witness).map(([k, v]) => `${k}=${v}`).join(", ");
      const said = `bounds [${d.bounds.join(", ")}], ${at ? `witness ${at}, margin ${d.margin}` : "no witness"}; ${d.work.boxes} boxes, ${fmt(d.work.ms, 0)} ms`;
      return {
        ...d, kind: "ranged", name, pass: d.holds === "undecided" ? undefined : d.holds, value: d.margin,
        measured: `${d.holds === "undecided" ? "undecided: " : ""}${spentText(d.spent)}; ${said}`,
        limit: `condition true ${kind === "always" ? "everywhere" : "somewhere"} in range`,
      };
    },
  };
}

const fmt = (x: number, digits: number): string => (Number.isFinite(x) ? x.toFixed(digits) : String(x));

/**
 * A condition over a measured quantity, sampled at `SAMPLE_RATE` over its time range (both ends included): the least
 * (`always`) or greatest (`eventually`) margin seen and where. Between samples nothing is known, and the verdict says so.
 */
function sampleRun(kind: "always" | "eventually", leaf: Leaf, condition: Expr<"bool">, margin: Expr<"f32">, sampled: ReturnType<typeof measuredIn>, probe: Probe, name: string, what: string): Extract<Verdict, { kind: "sampled" }> {
  const [a, b] = [probe.scalar(leaf.lo), probe.scalar(leaf.hi)];
  if (!(Number.isFinite(a) && Number.isFinite(b) && a <= b)) throw new SceneError(kind, `the range ${shownRange(leaf)} = [${a}, ${b}]`, "finite ends, lo <= hi", "swap the two ends");
  const n = Math.max(1, Math.ceil((b - a) * SAMPLE_RATE));
  const roots = [margin, condition], sign = kind === "always" ? 1 : -1;
  let best = { t: a, m: Infinity * sign }, found = kind === "eventually" ? false : true;
  for (let i = 0; i <= (a === b ? 0 : n); i++) {
    const t = i === n ? b : a + ((b - a) * i) / n;
    const vars = Object.fromEntries(sampled.map((m) => [m.name, m.at(probe, t)]));
    const [m, truth] = probe.eval(roots, t, vars) as [number, number | boolean];
    if (sign * m < sign * best.m) best = { t, m };
    if (kind === "always" && !truth) found = false;
    if (kind === "eventually" && truth) found = true;
  }
  const samples = a === b ? 1 : n + 1;
  return {
    kind: "sampled", name, pass: found, samples, value: best.m,
    measured: `${kind === "always" ? "least" : "greatest"} margin ${fmt(best.m, 4)} at t=${fmt(best.t, 3)} (${fmt(a, 2)}..${fmt(b, 2)}; ${what} is measured, not an expression)`,
    limit: `condition true ${kind === "always" ? "at every" : "at some"} sample`,
  };
}
function spentText(s: Spent): string {
  switch (s.kind) {
    case "finished": return "finished";
    case "boxes": return `boxes (${s.boxes} spent; raise boxes)`;
    case "unbounded": return `unbounded ${s.op} (${s.why}; use finite range ends)`;
    case "ms": return `ms (${s.ms} ms; raise ms)`;
    case "gpu-only": case "no-enclosure": return `${s.kind} (${s.fn}; use an expression with an enclosure)`;
    case "cpu-table": return `cpu-table (${s.table}, ${s.why}; use a sampled check)`;
    case "raw-compute": return `raw compute "${s.resource}"`;
    case "recur": return `recur (${s.table}; measure the baked trajectory with a sampled check)`;
    case "skin": return "skin (measure the skin with a sampled contact check)";
    case "measured": return `measured (${s.what} has no enclosure: run samples the condition)`;
    case "width": return `width: boxes ${s.width.toExponential(1)} wide could not be split further${s.op === undefined ? "" : `; unbounded there: ${s.op}`}`;
  }
}

// ---- the search ---------------------------------------------------------------------------------------------------

/** What every search in one decision shares: the probe's held values, the budget and the counters. */
type Run = {
  readonly ranges: Map<Leaf, I>;
  readonly held: ConstructorParameters<typeof Evaluator>[1];
  readonly where: string;
  readonly work: Work;
  readonly limit: number;
  boxes: number;
};

/** Evaluations a pattern search may spend polishing a witness (`search`). */
const POLISH = 24;

/**
 * Where a search should start, as an enclosing search knows it: a value for each leaf (key to value), and for some of
 * them how they move with the enclosing search's leaves (inner key to outer key to slope). With slopes, a point of the
 * inner search over a box of the outer one is a line, not a constant: `Track` (src/interval.ts).
 */
type Hint = { readonly at: ReadonlyMap<string, number>; readonly slopes: ReadonlyMap<string, ReadonlyMap<string, number>> };
const NO_HINT: Hint = { at: new Map(), slopes: new Map() };

/** A box of a search: its leaves' intervals, the enclosure of the value over it (min form: see `search`), where to start inside. */
type Cell = {
  readonly at: readonly I[];
  readonly lo: number;
  readonly hi: number;
  readonly sensitivity: readonly number[];
  readonly truth: Tri;
  readonly margin: number;
  readonly why: string | undefined;
  /** Where the inner search found its extreme (leaf key to value): where the inner searches of this box's halves start. */
  readonly inner: ReadonlyMap<string, number>;
};

/** The answer of a search over its leaves with the outer leaves held to `outer`, and where (leaf key to value) it found its extreme. */
type Found = { readonly margin: number; readonly truth: Tri; readonly lo: number; readonly hi: number; readonly at: ReadonlyMap<string, number>; readonly spent: Spent };

class BudgetStop extends Error {
  constructor(readonly spent: Extract<Spent, { kind: "boxes" | "ms" }>) { super(spent.kind); }
}

function decide(q: Quantified, probe: Probe, where: string, precision: number, limit: number, ms: number | undefined, keys: ReadonlySet<string>): Decision {
  const t0 = performance.now(), deadline = t0 + (ms ?? Infinity);
  const checkTime = () => {
    if (performance.now() >= deadline) throw new BudgetStop({ kind: "ms", ms: ms! });
  };
  const charge = () => {
    checkTime();
    if (run.boxes >= limit) throw new BudgetStop({ kind: "boxes", boxes: limit });
    run.boxes++;
  };
  const work = { tableFills: probe.compiled.tableFills, nodes: 0, codomain: 0, charge, checkTime };
  const state = probe.state(0);
  const slots = new Map<string, readonly number[]>();
  for (const s of probe.artefact.inputs) slots.set(`input:${s.name}`, Array.from(state.inputs.subarray(s.slot, s.slot + s.def.length)));
  for (const s of probe.artefact.params) slots.set(`param:${s.name}`, [state.params[s.slot]!]);
  const held: Run["held"] = (n) => n.op === "var" ? undefined : slots.get(`${n.op}:${n.spec.name}`) ??
    (n.op === "param" ? [Math.fround(n.spec.def)] : n.spec.def.map(Math.fround));
  const run: Run = { ranges: new Map(), held, where, work, limit, boxes: 0 };
  const labels = new Map<string, string>();
  let found: Found;
  try {
    const recurrence = probe.artefact.recurrences?.[0];
    if (recurrence) throw new RangeRefusal(where, { kind: "recur", table: recurrence.name });
    for (let f: Quantified | undefined = q; f; f = f.body.kind === "nested" ? f.body.q : undefined) {
      for (const l of f.leaves) {
        labels.set(l.key, l.label);
        const end = (e: Expr<"f32">): number => {
          if (e.node.op === "const") return e.node.v[0]!;
          charge();
          // A table program may hide a bound leaf from the endpoint's syntactic walk.
          const ev = new Evaluator(new Map(), (n) => {
            const key = `${n.op}:${n.op === "var" ? n.name : n.spec.name}`;
            if (keys.has(key)) throw new RangeRefusal(where, { kind: "dependent-end", leaf: key });
            return held(n);
          }, where, work);
          const [lo, hi] = ev.scalar(e).at;
          if (lo !== hi) throw new SceneError(where, `a range end enclosed by [${lo}, ${hi}]`, "a fixed endpoint", "use an endpoint computed only from held leaves");
          return lo;
        };
        const lo = end(l.lo), hi = end(l.hi);
        if (!Number.isFinite(lo) || !Number.isFinite(hi)) throw new Unbounded("overflow", "a range end is non-finite");
        if (lo > hi) throw new SceneError(q.kind, `the range ${shownRange(l)} = [${lo}, ${hi}]`, "lo <= hi", "swap the two ends");
        run.ranges.set(l, [lo, hi]);
      }
    }
    found = search(q, new Map(), run, precision, "point", NO_HINT);
  } catch (e) {
    let spent: Spent;
    if (e instanceof BudgetStop) spent = e.spent;
    else if (e instanceof Unbounded) spent = { kind: "unbounded", op: e.op, why: e.why };
    else if (e instanceof RangeRefusal && e.reason.kind !== "dependent-end" && e.reason.kind !== "unbound" && e.reason.kind !== "equality") spent = e.reason;
    else throw e;
    found = { lo: -Infinity, hi: Infinity, truth: "either", margin: NaN, at: new Map(), spent };
  }
  const witness = Number.isFinite(found.margin) ? Object.fromEntries([...found.at].map(([k, v]) => [labels.get(k)!, v])) : {};
  return {
    holds: found.truth === "either" ? "undecided" : found.truth === "true",
    margin: found.margin,
    bounds: [found.lo, found.hi], witness, spent: found.spent,
    work: { boxes: run.boxes, nodes: work.nodes, ms: performance.now() - t0, codomain: work.codomain },
  };
}

const invert = (t: Tri): Tri => t === "true" ? "false" : t === "false" ? "true" : "either";

const isPoint = (b: I | Track): boolean => Array.isArray(b) && b[0] === b[1];

/**
 * Bounds on the quantified value for every point of `outer`: `lo <= value <= hi`. With `outer` a point and enough
 * budget, `hi - lo <= precision`; with `outer` a box (`effort` "box"), a few splits only, so the bounds are valid but
 * loose. `hint` is where an enclosing search last found this one's extreme: evaluated first, it makes the few splits of
 * a box enough (without it, the inner search of a ∀ speed ∃ time check lost a 7 ms touchdown window in two seconds of
 * flight at every narrow speed box, and the outer search split down to its width floor).
 * The search is written for `always` (a least value); `eventually` is the same search on the negated value.
 */
function search(q: Quantified, outer: Box, run: Run, precision: number, effort: "point" | "box", hint: Hint): Found {
  const sign = q.kind === "always" ? 1 : -1;
  const ranges = q.leaves.map((l) => run.ranges.get(l)!);
  const outerPoint = [...outer.values()].every(isPoint);
  const floor = ranges.map(([a, b]) => Math.max((b - a) * 1e-9, 1e-12));
  const boxOf = (at: readonly (I | Track)[]): Map<string, I | Track> => { const m = new Map(outer); q.leaves.forEach((l, k) => m.set(l.key, at[k]!)); return m; };
  /**
   * A point of this search's leaves, as bound over the outer box: a constant, or where the hint gives slopes and the
   * outer leaves they follow are ranges, a line that stays inside this leaf's range across the outer box.
   */
  const pointOf = (xs: readonly number[]): (I | Track)[] => xs.map((x, k) => {
    const slopes = hint.slopes.get(q.leaves[k]!.key);
    if (slopes === undefined) return [x, x];
    let reach = 0;
    const live = new Map<string, number>();
    for (const [key, s] of slopes) {
      const b = outer.get(key);
      if (b === undefined || !Array.isArray(b) || b[0] === b[1]) continue;
      live.set(key, s);
      reach += Math.abs(s) * (b[1] - b[0]) / 2;
    }
    const [a, b] = ranges[k]!;
    return live.size === 0 || x - reach < a || x + reach > b ? [x, x] : { centre: x, slopes: live };
  });
  const valueAt = (at: readonly (I | Track)[]): number[] => at.map((b) => (Array.isArray(b) ? b[0] / 2 + b[1] / 2 : (b as Track).centre));
  const pointAt = (xs: readonly number[], inner: ReadonlyMap<string, number>): Map<string, number> => new Map([...q.leaves.map((l, k): [string, number] => [l.key, xs[k]!]), ...inner]);

  /** The body's enclosure over a box (min form), its sensitivity per leaf, and where an inner search found its extreme. */
  const enclose = (at: readonly (I | Track)[], point: boolean, from: Hint, precise = true): Cell => {
    run.work.charge!();
    const box = boxOf(at), cellAt = at.map((b): I => (Array.isArray(b) ? (b as unknown as I) : [(b as Track).centre, (b as Track).centre]));
    try {
      switch (q.body.kind) {
        case "atom": {
          const ev = new Evaluator(box, run.held, run.where, run.work);
          const v = ev.scalar(q.body.margin);
          const [lo, hi] = sign > 0 ? v.at : [-v.at[1], -v.at[0]];
          const truth = lo > 0 ? "true" : hi < 0 ? "false" : sign > 0 ? ev.bool(q.body.condition) : invert(ev.bool(q.body.condition));
          return { at: cellAt, lo, hi, truth, margin: v.at[0] === v.at[1] ? v.at[0] : NaN, sensitivity: v.sensitivity.slice(outer.size), why: undefined, inner: new Map() };
        }
        case "nested": {
          // Run to the precision only where every leaf is held to a point; over a box a few splits must do.
          const f = search(q.body.q, box, run, precision, point && outerPoint && precise ? "point" : "box", from);
          const [lo, hi] = sign > 0 ? [f.lo, f.hi] : [-f.hi, -f.lo];
          return { at: cellAt, lo, hi, margin: f.margin, truth: sign > 0 ? f.truth : invert(f.truth), sensitivity: [], why: f.spent.kind === "width" ? f.spent.op : undefined, inner: f.at };
        }
      }
    } catch (e) {
      if (!(e instanceof Unbounded)) throw e;
      return { at: cellAt, lo: -Infinity, hi: Infinity, truth: "either", margin: NaN, sensitivity: [], why: `${e.op} (${e.why})`, inner: from.at };
    }
  };

  // Best first: the box whose enclosure reaches lowest. `upper` is the least value proven at a point, at `best`.
  let counterexample = false;
  let margin = NaN, upper = Infinity, best: ReadonlyMap<string, number> = new Map();
  /** Evaluate a point (`xs`, one value per leaf); its inner witness is where searches inside the box it centres start. */
  const tryPoint = (xs: readonly number[], from: Hint): ReadonlyMap<string, number> => {
    // An inner search runs to the precision only at a point that may lower `upper`: a few splits from `from` first.
    // Run in full at every middle, the inner searches spent the whole budget proving values far above the extreme.
    const at = pointOf(xs);
    let p = enclose(at, true, from, q.body.kind === "atom");
    if (q.body.kind === "nested" && p.lo < upper) p = enclose(at, true, { at: p.inner, slopes: from.slopes });
    if (p.hi < upper || (p.hi === upper && p.truth === "false" && !counterexample)) { upper = p.hi; margin = p.margin; best = pointAt(xs, p.inner); }
    if (p.truth === "false") counterexample = true;
    return p.inner;
  };
  /**
   * Move `best` downhill by a pattern search: a step along each leaf each way, doubled after a success and halved after
   * a failure, `POLISH` evaluations at most. It only finds points (every bound stays a proof); what it buys is a
   * witness near the extreme early, which an enclosing search uses to start each box of its own. From a neighbouring
   * box's witness the touchdown instant had moved 3.6 ms, half its 7 ms window, and splitting time alone took ~150
   * boxes per box to find it again.
   */
  const polish = (from: Hint): void => {
    if (!q.leaves.every((l) => best.has(l.key))) return;
    const steps = ranges.map(([a, b]) => (b - a) / 512);
    for (let n = 0; n < POLISH && steps.some((h, j) => h > floor[j]!);) {
      for (let j = 0; j < ranges.length; j++) {
        let moved = false;
        for (const dir of [1, -1]) {
          const here = q.leaves.map((l) => best.get(l.key)!), [a, b] = ranges[j]!;
          const x = Math.min(b, Math.max(a, here[j]! + dir * steps[j]!));
          if (x === here[j]) continue;
          here[j] = x;
          const was = upper;
          tryPoint(here, { at: best, slopes: from.slopes });
          n++;
          if (upper < was) { moved = true; break; }
        }
        steps[j] = moved ? steps[j]! * 2 : steps[j]! / 2;
      }
    }
  };
  const middle = (at: readonly I[]): number[] => at.map(([a, b]) => a / 2 + b / 2);

  let live: Cell[] = [];
  let spent: Spent = { kind: "finished" };
  let whole: Cell | undefined;
  const truth = (): Tri => {
    if (counterexample || upper < 0 || whole?.truth === "false" || live.some((c) => c.truth === "false")) return "false";
    if (whole?.truth === "true" || (live.length > 0 && live.every((c) => c.truth === "true" || c.lo > 0))) return "true";
    return "either";
  };
  try {
    whole = enclose(ranges, false, hint);
    live = [whole];
    if (q.leaves.every((l) => hint.at.has(l.key))) tryPoint(q.leaves.map((l) => hint.at.get(l.key)!), hint);
    live = [{ ...whole, inner: tryPoint(middle(ranges), { at: whole.inner, slopes: hint.slopes }) }];
    // The corners of the range too: an extreme often sits at an end (the fastest throw lands nearest the pad's edge),
    // where no box's middle ever falls.
    if (q.leaves.length <= 4) {
      for (let bits = 0; bits < 1 << q.leaves.length; bits++) tryPoint(ranges.map(([a, b], j) => ((bits >> j) & 1 ? b : a)), { at: live[0]!.inner, slopes: hint.slopes });
    }
    polish(hint);
    const cap = effort === "box" ? Math.min(run.limit, run.boxes + 8) : run.limit;
    for (;;) {
      live = live.filter((c) => c.lo <= upper);
      if (live.length === 0) break;
      let k = 0;
      for (let i = 1; i < live.length; i++) if (live[i]!.lo < live[k]!.lo) k = i;
      const lower = live[k]!.lo;
      if (upper - lower <= precision && (truth() !== "either" || effort === "box")) break;
      if (effort === "box" && run.boxes >= cap) break;
      const c = live[k]!;
      // Split along the leaf the value is most sensitive to; without sensitivities, the widest relative to its range.
      const widths = c.at.map(([a, b], j) => b - a > floor[j]! ? (b - a) / Math.max(ranges[j]![1] - ranges[j]![0], 1e-300) : 0);
      const by = c.sensitivity.length === widths.length && c.sensitivity.some((s, j) => s > 0 && widths[j]! > 0)
        ? c.sensitivity.map((s, j) => (widths[j]! > 0 ? s : -1)) : widths;
      let j = 0;
      for (let i = 1; i < by.length; i++) if (by[i]! > by[j]!) j = i;
      if (!(widths[j]! > 0)) { spent = { kind: "width", width: Math.max(...c.at.map(([a, b]) => b - a)), op: c.why }; break; }
      const [a, b] = c.at[j]!, m = a / 2 + b / 2;
      if (m <= a || m >= b) { spent = { kind: "width", width: b - a, op: c.why }; break; }
      const halves = [[a, m], [m, b]].map((h) => c.at.map((x, i): I => (i === j ? (h as unknown as I) : x)));

      // The middles first: their inner witnesses, found to the precision where they matter, start the halves' inner
      // searches; the two together give the slope with which the inner witness moves along the split leaf.
      const inners = halves.map((h) => tryPoint(middle(h), { at: c.inner, slopes: hint.slopes }));
      const slopes = slopesAlong(q.leaves[j]!.key, inners, middle(halves[0]!)[j]!, middle(halves[1]!)[j]!, c.inner, hint.slopes);
      // Commit both children together. If a budget interrupts the split, the parent still covers them.
      live.splice(k, 1, ...halves.map((h, i) => ({ ...enclose(h, false, { at: inners[i]!, slopes }), inner: inners[i]! })));
    }
    run.work.checkTime!();
  } catch (e) {
    if (!(e instanceof BudgetStop) || outer.size > 0) throw e;
    spent = e.spent;
  }
  const t = spent.kind === "boxes" || spent.kind === "ms" ? "either" : truth();
  const lower = whole === undefined ? -Infinity : live.reduce((lo, c) => Math.min(lo, c.lo), upper);
  upper = Math.min(upper, whole?.hi ?? Infinity);
  // Back to the quantifier's own sense.
  return sign > 0 ? { lo: lower, hi: upper, truth: t, margin, at: best, spent } : { lo: -upper, hi: -lower, truth: invert(t), margin, at: best, spent };
}

/**
 * How each inner leaf's witness moves along the outer leaf `key`, from the witnesses at two points `x0` and `x1` of it
 * (the middles of a box's halves), added to the slopes already known along other leaves.
 */
function slopesAlong(key: string, inners: readonly ReadonlyMap<string, number>[], x0: number, x1: number, parent: ReadonlyMap<string, number>,
  known: ReadonlyMap<string, ReadonlyMap<string, number>>): ReadonlyMap<string, ReadonlyMap<string, number>> {
  const out = new Map<string, ReadonlyMap<string, number>>(known);
  for (const inner of parent.keys()) {
    const a = inners[0]!.get(inner), b = inners[1]!.get(inner);
    if (a === undefined || b === undefined || x1 === x0) continue;
    out.set(inner, new Map([...(known.get(inner) ?? []), [key, (b - a) / (x1 - x0)]]));
  }
  return out;
}
