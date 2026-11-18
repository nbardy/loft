// Ranged checks: a condition that holds for every time in [a, b] and every value of a param in [lo, hi], decided by
// enclosing the condition over whole boxes of those leaves (src/interval.ts) rather than at sampled frames, graded by
// how far it is from failing (the margin), with the point where it comes nearest (the witness).
//
// The condition is written with Loft's own comparisons, `ge(a, b)` and the rest, combined with `and`, `or` and `not`.
// Its margin is their robustness (signal temporal logic, Donze and Maler 2010): `ge(a, b)` is a - b, `le(a, b)` is
// b - a, `and` the least of its two, `or` the greater, `not` the negation; `always` is the least over its range and
// `eventually` the greatest. The condition holds where the margin is at least 0.
//
// Each quantifier is a branch and bound over its box: the box whose enclosure could hold the extreme is split (along
// the leaf its value is most sensitive to), a box that cannot is dropped, and the middle of every box is evaluated as
// a point to bound the extreme from the other side. A run of quantifiers of one kind is one search over their joint
// box; a quantifier inside another of the other kind is a search of its own for each box or point the outer one asks
// about, cut short for a box (a few splits give a valid but loose enclosure) and run to the precision for a point.

import { Expr, lift, max, min, neg, sub, type In } from "./expr.ts";
import { SceneError } from "./errors.ts";
import { Evaluator, Unbounded, type Box, type I, type Track, type Work } from "./interval.ts";
import type { Check, Probe, Verdict } from "./check.ts";

/** A leaf and the range it takes: `[time, 0, 4]`, `[param("crab.reach", 1), 0.8, 1.2]`. */
export type Range = readonly [Expr<"f32">, In<"f32">, In<"f32">];

/** What a ranged check decides. `margin` is within `precision` of the true extreme when the search finished. */
export type Decision = {
  /** `"undecided"` only when a budget ran out before the margin's sign was settled; `spent` says which. */
  readonly holds: true | false | "undecided";
  /** The margin at the witness: the least (always) or greatest (eventually) value the search found. */
  readonly margin: number;
  /** Proven bounds on the true margin. */
  readonly bounds: I;
  /** Where the margin is `margin`: each bound leaf's value (time as "time"). */
  readonly witness: Readonly<Record<string, number>>;
  readonly work: { readonly boxes: number; readonly nodes: number; readonly ms: number; readonly codomain: number };
  /** Why the search stopped short of `precision`, or "finished". */
  readonly spent: Spent;
};
/** How a search ended. */
export type Spent =
  | { readonly kind: "finished" }
  /** The box budget ran out. */
  | { readonly kind: "boxes"; readonly boxes: number }
  /** The boxes that could hold the extreme could not be split further; `op` is why they stayed wide, if an operator was unbounded on them. */
  | { readonly kind: "width"; readonly width: number; readonly op: string | undefined };

/** A ranged check: a `Check` for `loft check`, and `decide(probe)` for the numbers. */
export interface Ranged extends Check {
  readonly formula: Quantified;
  decide(probe: Probe): Decision;
}

type Leaf = { readonly key: string; readonly label: string; readonly expr: Expr<"f32">; readonly lo: Expr<"f32">; readonly hi: Expr<"f32"> };
type Quantified = { readonly kind: "always" | "eventually"; readonly leaves: readonly Leaf[]; readonly body: Body; readonly label: string };
type Body = { readonly kind: "atom"; readonly margin: Expr<"f32"> } | { readonly kind: "nested"; readonly q: Quantified };

/** Search settings. */
export type Settings = {
  /** A name for the check (default: the quantifiers and their ranges). */
  readonly label?: string;
  /** How close the margin must be bounded, in its own units (default 1e-4). */
  readonly precision?: number;
  /** How many boxes all searches together may evaluate (default 20000). */
  readonly boxes?: number;
};

// ---- the authoring surface ----------------------------------------------------------------------------------------

/**
 * The condition holds for every value of the range's leaf in [lo, hi]: `always([time, 0, 4], ge(gap, 0.1))`. The leaf
 * is `time`, an `input`, a `param` or a free `variable`; the condition is a comparison (`ge`, `gt`, `le`, `lt`) or
 * several joined by `and`, `or`, `not`, or another `always` or `eventually`. Its margin is the least of the
 * condition's margin over the range, found within `precision`, with the leaf values where it is least.
 * cannot: compare with `eq` or `ne` (no margin); read a GPU-only function, a cellular noise, or a table filled on the
 *   CPU (refused by name when the check runs).
 * relatives: eventually, staysClear (the sampled check it replaces where the scene is a closed form).
 */
export function always(range: Range, condition: Expr<"bool"> | Ranged, o: Settings = {}): Ranged {
  return ranged("always", range, condition, o);
}

/**
 * The condition holds for some value of the range's leaf in [lo, hi]: `eventually([time, 0, 3], le(height, 0))`.
 * Its margin is the greatest of the condition's margin over the range; the witness is where it is greatest.
 * relatives: always.
 */
export function eventually(range: Range, condition: Expr<"bool"> | Ranged, o: Settings = {}): Ranged {
  return ranged("eventually", range, condition, o);
}

function leafOf(kind: string, range: Range): Leaf {
  const [e, lo, hi] = range;
  if (!(e instanceof Expr) || e.ty !== "f32") throw new SceneError(kind, `a range over ${e instanceof Expr ? `a ${e.ty}` : typeof e}`, "a range over an f32 leaf: [time, 0, 4]", `${kind}([time, 0, 4], ...)`);
  const n = e.node;
  switch (n.op) {
    case "var": return { key: `var:${n.name}`, label: n.name, expr: e, lo: lift<"f32">(lo), hi: lift<"f32">(hi) };
    case "input": case "param": return { key: `${n.op}:${n.spec.name}`, label: n.spec.name, expr: e, lo: lift<"f32">(lo), hi: lift<"f32">(hi) };
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

function ranged(kind: "always" | "eventually", range: Range, condition: Expr<"bool"> | Ranged, o: Settings): Ranged {
  const leaf = leafOf(kind, range);
  const body: Body = condition instanceof Expr ? { kind: "atom", margin: marginOf(kind, condition) } : { kind: "nested", q: condition.formula };
  // A run of one kind is one search over the joint box.
  const merged = body.kind === "nested" && body.q.kind === kind;
  const leaves = merged ? [leaf, ...body.q.leaves] : [leaf];
  const inner = merged ? body.q.body : body;
  const keys = new Set<string>();
  const check = (q: { leaves: readonly Leaf[]; body: Body }): void => {
    for (const l of q.leaves) {
      if (keys.has(l.key)) throw new SceneError(kind, `${l.label} bound twice`, "each leaf bound by one range", `drop one of the two ranges over ${l.label}`);
      keys.add(l.key);
    }
    if (q.body.kind === "nested") check(q.body.q);
  };
  check({ leaves, body: inner });
  const label = `${kind} ${leaves.map(shownRange).join(", ")}${inner.kind === "nested" ? `: ${inner.q.label}` : ""}`;
  const formula: Quantified = { kind, leaves, body: inner, label };
  const precision = o.precision ?? 1e-4, boxes = o.boxes ?? 20000;
  if (!(precision > 0)) throw new SceneError(kind, `precision ${precision}`, "a precision above 0", `${kind}(range, condition, { precision: 1e-4 })`);
  const name = o.label ?? label;
  return {
    name, formula,
    decide: (probe) => decide(formula, probe, name, precision, boxes),
    run(probe): Verdict {
      const d = decide(formula, probe, name, precision, boxes);
      const at = Object.entries(d.witness).map(([k, v]) => `${k}=${fmt(v, 4)}`).join(", ");
      const said = `margin ${fmt(d.margin, 4)} in [${fmt(d.bounds[0], 4)}, ${fmt(d.bounds[1], 4)}] at ${at}; ${d.work.boxes} boxes, ${fmt(d.work.ms, 0)} ms`;
      return {
        name, pass: d.holds === true, value: d.margin,
        measured: d.holds === "undecided" ? `undecided (${spentText(d.spent)}): ${said}` : said,
        limit: `margin >= 0 (${kind === "always" ? "everywhere" : "somewhere"} in range)`,
      };
    },
  };
}

const fmt = (x: number, digits: number): string => (Number.isFinite(x) ? x.toFixed(digits) : String(x));
function spentText(s: Spent): string {
  switch (s.kind) {
    case "finished": return "finished";
    case "boxes": return `box budget of ${s.boxes} spent`;
    case "width": return `boxes ${s.width.toExponential(1)} wide could not be split further${s.op === undefined ? "" : `; unbounded there: ${s.op}`}`;
  }
}

// ---- the search ---------------------------------------------------------------------------------------------------

/** What every search in one decision shares: the probe's held values, the budget and the counters. */
type Run = {
  readonly probe: Probe;
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
  readonly why: string | undefined;
  /** Where the inner search found its extreme (leaf key to value): where the inner searches of this box's halves start. */
  readonly inner: ReadonlyMap<string, number>;
};

/** The answer of a search over its leaves with the outer leaves held to `outer`, and where (leaf key to value) it found its extreme. */
type Found = { readonly lo: number; readonly hi: number; readonly at: ReadonlyMap<string, number>; readonly spent: Spent };

function decide(q: Quantified, probe: Probe, where: string, precision: number, limit: number): Decision {
  const t0 = performance.now();
  const run: Run = { probe, where, work: { nodes: 0, codomain: 0 }, limit, boxes: 0 };
  const found = search(q, new Map(), run, precision, "point", NO_HINT);
  // `search` answers in the quantifier's own sense; the margin is the extreme it found at a point.
  const margin = q.kind === "always" ? found.hi : found.lo;
  const holds = found.lo >= 0 ? true : found.hi < 0 ? false : "undecided";
  const labels = new Map<string, string>();
  for (let f: Quantified | undefined = q; f !== undefined; f = f.body.kind === "nested" ? f.body.q : undefined) for (const l of f.leaves) labels.set(l.key, l.label);
  const witness = Object.fromEntries([...found.at].map(([k, v]) => [labels.get(k) ?? k, v]));
  return {
    holds, margin, bounds: [found.lo, found.hi], witness, spent: found.spent,
    work: { boxes: run.boxes, nodes: run.work.nodes, ms: performance.now() - t0, codomain: run.work.codomain },
  };
}

/** The value of a held leaf (an input at the probe's value, a param at its default); undefined for a free variable. */
function held(probe: Probe) {
  return (n: Parameters<ConstructorParameters<typeof Evaluator>[1]>[0]): readonly number[] | undefined => {
    switch (n.op) {
      case "var": return undefined;
      case "param": return [n.spec.def];
      case "input": {
        const set = probe.inputs[n.spec.name];
        return set === undefined ? n.spec.def : typeof set === "number" ? [set] : set;
      }
    }
  };
}

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
  const ranges = q.leaves.map((l): I => {
    const lo = run.probe.scalar(l.lo), hi = run.probe.scalar(l.hi);
    if (!(lo <= hi)) throw new SceneError(q.kind, `the range ${shownRange(l)} = [${lo}, ${hi}]`, "lo <= hi", "swap the two ends");
    return [lo, hi];
  });
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
    run.boxes++;
    const box = boxOf(at), cellAt = at.map((b): I => (Array.isArray(b) ? (b as unknown as I) : [(b as Track).centre, (b as Track).centre]));
    try {
      switch (q.body.kind) {
        case "atom": {
          const ev = new Evaluator(box, held(run.probe), run.where, run.work);
          const v = ev.scalar(q.body.margin);
          const [lo, hi] = sign > 0 ? v.at : [-v.at[1], -v.at[0]];
          return { at: cellAt, lo, hi, sensitivity: v.sensitivity.slice(outer.size), why: undefined, inner: new Map() };
        }
        case "nested": {
          // Run to the precision only where every leaf is held to a point; over a box a few splits must do.
          const f = search(q.body.q, box, run, precision, point && outerPoint && precise ? "point" : "box", from);
          const [lo, hi] = sign > 0 ? [f.lo, f.hi] : [-f.hi, -f.lo];
          return { at: cellAt, lo, hi, sensitivity: [], why: f.spent.kind === "width" ? f.spent.op : undefined, inner: f.at };
        }
      }
    } catch (e) {
      if (!(e instanceof Unbounded)) throw e;
      return { at: cellAt, lo: -Infinity, hi: Infinity, sensitivity: [], why: `${e.op} (${e.why})`, inner: from.at };
    }
  };

  // Best first: the box whose enclosure reaches lowest. `upper` is the least value proven at a point, at `best`.
  let upper = Infinity, best: ReadonlyMap<string, number> = new Map();
  /** Evaluate a point (`xs`, one value per leaf); its inner witness is where searches inside the box it centres start. */
  const tryPoint = (xs: readonly number[], from: Hint): ReadonlyMap<string, number> => {
    // An inner search runs to the precision only at a point that may lower `upper`: a few splits from `from` first.
    // Run in full at every middle, the inner searches spent the whole budget proving values far above the extreme.
    const at = pointOf(xs);
    let p = enclose(at, true, from, q.body.kind === "atom");
    if (q.body.kind === "nested" && p.lo < upper) p = enclose(at, true, { at: p.inner, slopes: from.slopes });
    if (p.hi < upper) { upper = p.hi; best = pointAt(xs, p.inner); }
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

  const whole = enclose(ranges, false, hint);
  if (q.leaves.every((l) => hint.at.has(l.key))) tryPoint(q.leaves.map((l) => hint.at.get(l.key)!), hint);
  let live: Cell[] = [{ ...whole, inner: tryPoint(middle(ranges), { at: whole.inner, slopes: hint.slopes }) }];
  // The corners of the range too: an extreme often sits at an end (the fastest throw lands nearest the pad's edge),
  // where no box's middle ever falls.
  if (q.leaves.length <= 4) {
    for (let bits = 0; bits < 1 << q.leaves.length; bits++) tryPoint(ranges.map(([a, b], j) => ((bits >> j) & 1 ? b : a)), { at: live[0]!.inner, slopes: hint.slopes });
  }
  polish(hint);
  const cap = effort === "box" ? Math.min(run.limit, run.boxes + 8) : run.limit;
  let spent: Spent = { kind: "finished" };
  for (;;) {
    live = live.filter((c) => c.lo <= upper);
    if (live.length === 0) break;
    let k = 0;
    for (let i = 1; i < live.length; i++) if (live[i]!.lo < live[k]!.lo) k = i;
    const lower = live[k]!.lo;
    if (upper - lower <= precision && (lower >= 0 || upper < 0 || effort === "box")) break;
    if (run.boxes + 4 > cap) { spent = effort === "box" ? { kind: "finished" } : { kind: "boxes", boxes: run.limit }; break; }
    const c = live[k]!;
    // Split along the leaf the value is most sensitive to; without sensitivities, the widest relative to its range.
    const widths = c.at.map(([a, b], j) => b - a > floor[j]! ? (b - a) / Math.max(ranges[j]![1] - ranges[j]![0], 1e-300) : 0);
    const by = c.sensitivity.length === widths.length && c.sensitivity.some((s, j) => s > 0 && widths[j]! > 0)
      ? c.sensitivity.map((s, j) => (widths[j]! > 0 ? s : -1)) : widths;
    let j = 0;
    for (let i = 1; i < by.length; i++) if (by[i]! > by[j]!) j = i;
    if (!(widths[j]! > 0)) { spent = { kind: "width", width: Math.max(...c.at.map(([a, b]) => b - a)), op: c.why }; break; }
    const [a, b] = c.at[j]!, m = a / 2 + b / 2;
    const halves = [[a, m], [m, b]].map((h) => c.at.map((x, i): I => (i === j ? (h as unknown as I) : x)));
    live.splice(k, 1);
    // The middles first: their inner witnesses, found to the precision where they matter, start the halves' inner
    // searches; the two together give the slope with which the inner witness moves along the split leaf.
    const inners = halves.map((h) => tryPoint(middle(h), { at: c.inner, slopes: hint.slopes }));
    const slopes = slopesAlong(q.leaves[j]!.key, inners, middle(halves[0]!)[j]!, middle(halves[1]!)[j]!, c.inner, hint.slopes);
    halves.forEach((h, i) => live.push({ ...enclose(h, false, { at: inners[i]!, slopes }), inner: inners[i]! }));
  }
  const lower = live.length === 0 ? upper : Math.min(...live.map((c) => c.lo));
  // Back to the quantifier's own sense.
  return sign > 0 ? { lo: lower, hi: upper, at: best, spent } : { lo: -upper, hi: -lower, at: best, spent };
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
