// `recur`: state that is stepped, baked once on the GPU, and read as a function of time (docs/proposals/state.md).
//
// A recurrence S(n + 1) = step(S(n), t_n), t_n = from + n / rate, is a function of time once it has been run:
// S(t) = S(floor((t - from) rate)). This file is the author's side: it checks the spec, classifies what the step reads
// (once, here), chooses what is kept from the size alone (`Kept`), and builds the reads (`at`, `on`) from `fetch` and
// `mix`. The compiler (src/compile/tables.ts, src/compile/recur.ts) emits the step's code; a host bakes it
// (src/host/bake.ts); the baked states travel in the artefact's static tables, so any frame still draws alone.

import { passOf, passOfTable } from "../frame/pass.ts";
import { SceneError } from "../errors.ts";
import {
  add, clamp, eq, Expr, f, fetch, floor, input, leaves, lift, min, mix, mod, mul, replace, saturate, scopedName, select, sub, time, variable, vec2,
  type In, type TableSource,
} from "../expr.ts";

/** Entries: a count (a list) or [w, h] (a grid). */
export type Cells = number | readonly [number, number];
/** An entry's index: a whole number for a list, a cell (a vec2 of whole numbers) for a grid. */
export type Index<C extends Cells> = C extends number ? In<"f32"> : In<"vec2">;
type IndexExpr<C extends Cells> = C extends number ? Expr<"f32"> : Expr<"vec2">;
/** An entry's value: one or more vec4 lanes, as in every table. */
export type Lanes = readonly Expr<"vec4">[];

export type RecurSpec<C extends Cells> = {
  /** An identifier, unique in the scene (it names tables). */
  readonly name: string;
  /** Entries: a count (a list) or [w, h] (a grid). prev() wraps every index into range. */
  readonly size: C;
  /** Steps per second of scene time. Inside `step`, `time` is the time of the step being taken. */
  readonly rate: number;
  /** When the first step is taken, seconds (default 0; negative settles before the scene starts). */
  readonly from?: number;
  /** The last time kept, seconds. After it, reads hold the last state. */
  readonly until: number;
  /** The state at `from`: entry i's lanes, a closed form in i. Fixes how many lanes there are. */
  readonly start: (i: IndexExpr<C>) => Lanes;
  /** Entry i after one step, from the whole state before it. Gather only: it writes entry i and nothing else. */
  readonly step: (prev: (j: Index<C>) => Lanes, i: IndexExpr<C>) => Lanes;
};

export type Recurrence<C extends Cells> = {
  /** Entry i at `when` (default `time`), linear between steps. */
  readonly at: (i: Index<C>, when?: In<"f32">) => Lanes;
  /** The state at coordinates 0..1 (list: linear along it; grid: bilinear, wrapping), at `when` (default `time`). */
  readonly on: (u: C extends number ? In<"f32"> : In<"vec2">, when?: In<"f32">) => Lanes;
};

/** The memory one recurrence may keep before the compiler keeps checkpoints instead (state.md 4.1). */
export const KEPT_BYTES = 64 * 1024 * 1024;
/** The most steps a frame may replay from a checkpoint. */
export const MOST_REPLAY = 120;

/**
 * What is kept of the run, chosen by the compiler from the size alone: every state, or every K-th (a checkpoint) with
 * the frame replaying at most K - 1 steps from the nearest one. Exact either way: the frame at step n is
 * step^(n mod K)(C[floor(n / K)]).
 */
export type Kept = { readonly kind: "every" } | { readonly kind: "checkpoints"; readonly every: number };

/** A recurrence's shape: a list of n, or a grid w x h laid out row by row. */
export type Shape = { readonly kind: "list"; readonly n: number } | { readonly kind: "grid"; readonly w: number; readonly h: number };

/**
 * A recurrence as the compiler sees it: the spec checked, its numbers derived, its expressions built once. Steps are
 * counted from `from` (step 0 is `start`); `first` is the first step kept and `steps` the kept span after it.
 */
export type RecurState = {
  readonly name: string;
  readonly shape: Shape;
  readonly entries: number;
  readonly lanes: number;
  readonly rate: number;
  readonly from: number;
  readonly until: number;
  /** Steps the bake takes, from `from` to `until`. */
  readonly total: number;
  /** The first step kept: the one at max(from, 0) clamped to `until` (the scene's clock starts at 0). */
  readonly first: number;
  /** The kept span, in steps (`total - first`): reads clamp to [0, steps]. */
  readonly steps: number;
  /** Seconds at the first kept state. */
  readonly start: number;
  readonly kept: Kept;
  /** States in the kept table: steps + 1, or one per checkpoint. Each is `entries + 1` entries: the state, then the clock. */
  readonly states: number;
  /** The free index variable `start` and `step` were called with. */
  readonly index: Expr<"f32"> | Expr<"vec2">;
  /** The free variable that stands for `time` inside the step (the step's own time). */
  readonly clock: Expr<"f32">;
  /** `start`, with `time` replaced by `from`. */
  readonly startLanes: Lanes;
  /** `step`, with `time` replaced by `clock`; it reads the previous state through `prev`. */
  readonly stepLanes: Lanes;
  /** What `prev` fetches: never placed, bound by whoever emits the step (the bake, the replay, the CPU twin). */
  readonly prev: TableSource;
  /** The kept states (a static table). */
  readonly table: TableSource;
  /** Checkpoints only: the two per-frame tables the frame replays into, and the step number the host writes. */
  readonly replay?: { readonly even: TableSource; readonly odd: TableSource; readonly step: Expr<"f32"> };
  /** Recurrences the step or start reads (each keeps every step): baked before this one. */
  readonly after: readonly RecurState[];
};

/** Which table of a recurrence a fetch names. */
export type RecurPart = "kept" | "even" | "odd" | "prev";

// ---- what a step may read ---------------------------------------------------------------------------------------

/**
 * Everything a step (or its start) reads, each classified once. The verdict is one exhaustive switch (`refusal`), and
 * one site throws. A new reader (a rendered pass, when passes land) is a new variant refused there.
 */
type Read =
  | { readonly kind: "prev" }
  | { readonly kind: "index" }
  | { readonly kind: "clock" }
  | { readonly kind: "static table"; readonly name: string }
  | { readonly kind: "recurrence"; readonly of: RecurState }
  | { readonly kind: "checkpointed recurrence"; readonly name: string }
  | { readonly kind: "frame table"; readonly name: string }
  | { readonly kind: "input"; readonly name: string }
  | { readonly kind: "param"; readonly name: string }
  | { readonly kind: "curve row"; readonly label: string }
  | { readonly kind: "free variable"; readonly name: string }
  /** A picture drawn off screen (src/frame/pass.ts): its `at`, or its camera through `project`. */
  | { readonly kind: "pass"; readonly name: string };

/** Why a read cannot be baked, and the fix; undefined when it can. */
function refusal(r: Read): { readonly given: string; readonly fix: string } | undefined {
  switch (r.kind) {
    case "prev": case "index": case "clock": case "static table": case "recurrence": return undefined;
    case "checkpointed recurrence":
      return { given: `a read of the recurrence "${r.name}", which keeps checkpoints`, fix: `make "${r.name}" keep every step (a smaller size, a lower rate or a shorter span), or merge both into one recurrence whose entries hold both` };
    case "frame table":
      return { given: `a read of the per-frame table "${r.name}" (a posed body's stations, a frame map)`, fix: "read the closed form it is made from (the spine as a function of time), or `tabulate` it as \"static\"" };
    case "input":
      return { given: `a read of the host input "${r.name}"`, fix: "a baked state cannot follow an input a game changes: use the closed forms that look like state (`jolt` with the kick time as an input), or make the value a constant" };
    case "param":
      return { given: `a read of the param "${r.name}"`, fix: "use a constant (a param edit would need a re-bake, which is not built)" };
    case "curve row":
      return { given: `a read of the curve row "${r.label}" (it can be edited after the bake)`, fix: "`tabulate` the curve as \"static\" and read that" };
    case "free variable":
      return { given: `a free variable "${r.name}" from outside the step`, fix: "a step is a function of its entry, `prev` and `time` only: pass constants, or the closed form" };
    // A state stepped from a rendered picture would depend on the baking GPU's rasterisation and carry pictures from
    // frame to frame (headroom A4): history by the back door.
    case "pass":
      return { given: `a read of the pass "${r.name}" (a picture drawn this frame)`, fix: "a step is baked before anything is drawn: compute what it needs from closed forms (a pass's `project` is arithmetic: write the same projection from the camera's numbers)" };
  }
}

const recurOf = new WeakMap<TableSource, { readonly state: RecurState; readonly part: RecurPart }>();
/** The recurrence a table belongs to, and which of its tables it is (undefined for any other table). */
export const recurTable = (t: TableSource): { readonly state: RecurState; readonly part: RecurPart } | undefined => recurOf.get(t);

function classifyReads(roots: readonly Expr[], own: { readonly prev: TableSource; readonly index: Expr; readonly clock: Expr }): Read[] {
  const l = leaves(roots);
  const reads: Read[] = [];
  // Tables first: a checkpointed recurrence also reads a hidden input, and its table names it better.
  for (const t of l.tables) {
    const r = recurOf.get(t), camera = passOfTable(t);
    if (t === own.prev) reads.push({ kind: "prev" });
    else if (camera !== undefined) reads.push({ kind: "pass", name: camera.name });
    else if (r !== undefined) {
      switch (r.part) {
        case "kept": reads.push(r.state.kept.kind === "every" ? { kind: "recurrence", of: r.state } : { kind: "checkpointed recurrence", name: r.state.name }); break;
        case "even": case "odd": case "prev": reads.push({ kind: "checkpointed recurrence", name: r.state.name }); break;
      }
    } else reads.push(t.role === "static" ? { kind: "static table", name: t.name } : { kind: "frame table", name: t.name });
  }
  const ownVar = (e: Expr): string => (e.node as { name: string }).name;
  for (const [name] of l.vars) reads.push(name === ownVar(own.index) ? { kind: "index" } : name === ownVar(own.clock) ? { kind: "clock" } : { kind: "free variable", name });
  for (const s of l.inputs) reads.push({ kind: "input", name: s.name });
  for (const p of l.params) reads.push({ kind: "param", name: p.name });
  for (const r of l.rows) reads.push({ kind: "curve row", label: r.label });
  for (const fn of l.libs) { const p = passOf(fn); if (p !== undefined) reads.push({ kind: "pass", name: p.name }); }
  return reads;
}

// ---- the spec ----------------------------------------------------------------------------------------------------

/** A whole number of steps, or a refusal naming the field. */
function wholeSteps(where: string, what: string, seconds: number, rate: number, fix: string): number {
  const n = seconds * rate, k = Math.round(n);
  if (Math.abs(n - k) > 1e-6 * Math.max(1, Math.abs(n))) throw new SceneError(where, `${what} of ${seconds} s at ${rate} steps a second (${n} steps)`, "a whole number of steps", fix);
  return k;
}

function lanesOf(where: string, what: string, lanes: unknown, want: number | undefined): Lanes {
  if (!Array.isArray(lanes) || lanes.length === 0) throw new SceneError(where, `${what} returned ${Array.isArray(lanes) ? "no lanes" : "something that is not a list"}`, "a list of vec4 lanes, e.g. [vec4(position, 0), vec4(velocity, 0)]", `return [vec4(...)] from ${what}`);
  lanes.forEach((x, k) => {
    if (!(x instanceof Expr) || x.ty !== "vec4") throw new SceneError(where, `${what} lane ${k} is ${x instanceof Expr ? `a ${x.ty}` : "not an expression"}`, "a vec4", `wrap it: vec4(value, 0) for a vec3, vec4(x, 0, 0, 0) for a number`);
  });
  if (want !== undefined && lanes.length !== want) throw new SceneError(where, `${what} returned ${lanes.length} lanes`, `${want}, as many as \`start\` returns`, "return one vec4 per lane, in the same order as `start`");
  return lanes as Lanes;
}

/**
 * A state stepped by your own function, baked once on the GPU and read as a function of time.
 *
 * The law: S(n + 1) = step(S(n)), run once from `start` at `from`, `rate` steps a second, kept to `until`. Inside
 * `step`, `time` is the time of the step being taken, so a step reads the scene's closed forms (a lure, an anchor) where
 * they were then. Reads are linear between steps in time, and linear (a list) or bilinear and wrapping (a grid) between
 * entries in space. Every state is kept when the history fits 64 MiB; otherwise every K-th, and a frame replays at most
 * K - 1 steps from the nearest (exact on the GPU that baked; within K steps of rounding on another). The baked states
 * go into the scene's files, so any frame still draws alone and checks measure what is drawn.
 *
 * Parameters: `size` (a count, or [w, h]), `rate`, `from` and `until` are fixed numbers: they decide memory. `start(i)`
 * is a closed form in the entry and fixes the lane count (each lane a vec4); `step(prev, i)` returns entry i from
 * `prev(j)`, the lanes of any entry before the step (indices wrap). `.at(i, when)` reads entry i; `.on(u, when)` reads
 * at coordinates 0..1.
 *
 * What it cannot do: read a host input, a param, a curve row or a per-frame table (the trajectory exists before the
 * game does); scatter (a step writes only its own entry: a particle depositing into a grid is the grid gathering from
 * the particles); go past `until` (the state holds); read a checkpointed state at any time but `time`; couple two
 * recurrences both ways (one reads the other, or one recurrence holds both); grow (a fixed capacity with birth times);
 * let substeps see each other within a step (Jacobi at a higher rate, not Gauss-Seidel); stay within 64 MiB at any size
 * and rate (a 256 x 256 grid for 120 s at 60 steps a second already replays up to 113 steps a frame).
 *
 * Relatives: closed form first. A kick is a `jolt` or `response`, a trail is the thing at earlier times (`dragged`,
 * `echo`), a small swing is a sum of modes, a ballistic particle is an arc from its birth time, a few numbers step
 * inside one expression with `iterate(floor(time * rate), ...)`. A static pattern is `until: 0`; a periodic loop
 * settles into its forced answer by starting several periods early (`loft check` measures the seam). The level below
 * is a `lib` function with its own WGSL inside the step.
 */
export function recur<const C extends Cells>(spec: RecurSpec<C>): Recurrence<C> {
  const name = scopedName(spec.name);
  const where = `recur "${name}"`;
  if (!/^[A-Za-z][A-Za-z0-9_]*$/.test(name)) throw new SceneError(where, `the name "${spec.name}"`, "an identifier (letters, digits, underscores, starting with a letter): it names tables", `recur({ name: "${spec.name.replace(/[^A-Za-z0-9_]/g, "_").replace(/^[^A-Za-z]+/, "") || "state"}", ... })`);
  const whole = (x: number) => Number.isInteger(x) && x >= 1;
  const shape: Shape = typeof spec.size === "number"
    ? (whole(spec.size) ? { kind: "list", n: spec.size } : (() => { throw new SceneError(where, `size ${spec.size}`, "a whole number of entries, at least 1 (or [w, h] for a grid)", `size: ${Math.max(1, Math.round(Number(spec.size)) || 1)}`); })())
    : (Array.isArray(spec.size) && spec.size.length === 2 && whole(spec.size[0]) && whole(spec.size[1]) ? { kind: "grid", w: spec.size[0], h: spec.size[1] } : (() => { throw new SceneError(where, `size [${String(spec.size)}]`, "[w, h], two whole numbers of cells", "size: [128, 128]"); })());
  const entries = shape.kind === "list" ? shape.n : shape.w * shape.h;
  const rate = spec.rate, from = spec.from ?? 0, until = spec.until;
  if (!(rate > 0 && Number.isFinite(rate))) throw new SceneError(where, `rate ${rate}`, "a positive number of steps a second", "rate: 60");
  if (!(Number.isFinite(from) && Number.isFinite(until) && until >= from)) throw new SceneError(where, `from ${from}, until ${until}`, "finite times with until at or after from", `until: ${Number.isFinite(from) ? from + 10 : 10}`);
  const total = wholeSteps(where, "a span (until - from)", until - from, rate, `choose until so that (until - from) x rate is whole, e.g. until: ${from + Math.round((until - from) * rate) / rate}`);
  // The scene's clock starts at 0: states before it are never read, so the first kept is the one at max(from, 0).
  const keepFrom = Math.min(Math.max(from, 0), until);
  const first = wholeSteps(where, "time 0 after from", keepFrom - from, rate, `choose from so that -from x rate is whole, e.g. from: ${-Math.round(-from * rate) / rate}`);
  const steps = total - first;

  const index = (shape.kind === "list" ? variable(`_R_${name}_i`, "f32") : variable(`_R_${name}_i`, "vec2")) as Expr<"f32"> | Expr<"vec2">;
  const clock = variable(`_R_${name}_t`, "f32");
  const startLanes = lanesOf(where, "start", replace(lanesOf(where, "start", spec.start(index as IndexExpr<C>), undefined), new Map([[time.id, f(from)]])), undefined) as Lanes;
  const lanes = startLanes.length;

  // The kept states: what the history costs, and K from that alone.
  const stateBytes = entries * lanes * 16;
  const history = (steps + 1) * stateBytes;
  const raw = history <= KEPT_BYTES ? 1 : Math.ceil(history / KEPT_BYTES);
  // K is even whenever checkpoints are kept: then state j lives in table j mod 2 however the frame reached it (by replay or
  // by resuming the last frame's pair, src/host/host.ts `advance`). MOST_REPLAY is even, so rounding up cannot pass it.
  const every = raw === 1 ? 1 : raw + (raw % 2);
  if (raw > MOST_REPLAY) {
    const mb = (n: number) => `${(n / 1e6).toFixed(0)} MB`;
    throw new SceneError(where, `${steps + 1} states of ${mb(stateBytes)} (${mb(history)} in all), which would replay up to ${raw - 1} steps a frame`, `a history of at most ${MOST_REPLAY} x ${KEPT_BYTES / 2 ** 20} MiB (${mb(MOST_REPLAY * KEPT_BYTES)})`, `a lower rate, a smaller size or a shorter span (now from ${from} to ${until} s): the history must shrink about ${(history / (MOST_REPLAY * KEPT_BYTES)).toFixed(1)}x`);
  }
  const kept: Kept = every === 1 ? { kind: "every" } : { kind: "checkpoints", every };
  const states = every === 1 ? steps + 1 : Math.floor(steps / every) + 1;

  const table: TableSource = { name, lanes, entries: states * (entries + 1), role: "static", recurrence: name };
  const prev: TableSource = { name: `${name}_prev`, lanes, entries: entries + 1, role: "data", recurrence: name };
  const replay = kept.kind === "every" ? undefined : {
    even: { name: `${name}_even`, lanes, entries: entries + 1, role: "data", recurrence: name } as TableSource,
    odd: { name: `${name}_odd`, lanes, entries: entries + 1, role: "data", recurrence: name } as TableSource,
    // The step number n the frame replays to, written by the host from time (docs/artefact_format.md, "Replays"), so the
    // GPU and the CPU agree on n exactly.
    step: input(`loft_step.${name}`, "f32", [0]),
  };

  /** Entry j, wrapped into range, as a whole-number f32. */
  const wrap = (j: Index<C>): Expr<"f32"> => {
    switch (shape.kind) {
      case "list": return mod(lift<"f32">(j as In<"f32">), shape.n);
      case "grid": {
        const c = lift<"vec2">(j as In<"vec2">);
        if (c.ty !== "vec2") throw new SceneError(where, `a cell index of type ${c.ty}`, "a vec2 (x, y) of whole numbers", "prev(add(c, [1, 0]))");
        return add(mod(c.x, shape.w), mul(mod(c.y, shape.h), shape.w));
      }
    }
  };
  const read = (t: TableSource, base: In<"f32">, j: Expr<"f32">): Lanes => Array.from({ length: lanes }, (_, l) => fetch(t, add(base, j), l));
  const prevOf = (j: Index<C>): Lanes => read(prev, 0, wrap(j));
  const stepLanes = lanesOf(where, "step", replace(lanesOf(where, "step", spec.step(prevOf, index as IndexExpr<C>), lanes), new Map([[time.id, clock]])), lanes) as Lanes;

  const reads = classifyReads([...startLanes, ...stepLanes], { prev, index, clock });
  for (const r of reads) {
    const no = refusal(r);
    if (no !== undefined) throw new SceneError(where, no.given, "constants, `time`, static tables, `prev`, other recurrences that keep every step and `lib` functions", no.fix);
  }
  const after = [...new Set(reads.flatMap((r) => (r.kind === "recurrence" ? [r.of] : [])))];

  const state: RecurState = {
    name, shape, entries, lanes, rate, from, until, total, first, steps, start: from + first / rate, kept, states,
    index, clock, startLanes, stepLanes, prev, table, ...(replay === undefined ? {} : { replay }), after,
  };
  recurOf.set(table, { state, part: "kept" });
  recurOf.set(prev, { state, part: "prev" });
  if (replay !== undefined) { recurOf.set(replay.even, { state, part: "even" }); recurOf.set(replay.odd, { state, part: "odd" }); }

  const stride = entries + 1;
  /** Entry i at `when`. */
  const at = (i: Index<C>, when?: In<"f32">): Lanes => {
    const j = wrap(i);
    switch (kept.kind) {
      case "every": {
        // A still state (one kept) is a plain read; otherwise linear between the two kept states around `when`.
        if (steps === 0) return read(table, 0, j);
        const s = clamp(mul(sub(when ?? time, state.start), rate), 0, steps);
        const n = min(floor(s), steps - 1);
        const a = read(table, mul(n, stride), j), b = read(table, mul(add(n, 1), stride), j);
        return a.map((x, l) => mix(x, b[l]!, sub(s, n)));
      }
      case "checkpoints": {
        if (when !== undefined && lift<"f32">(when) !== time) {
          throw new SceneError(where, "a read at a time other than `time`", `\`time\`: this state keeps checkpoints, and a frame replays one time only (keeping every step, which a read at any time needs, would take ${((steps + 1) * stateBytes / 1e6).toFixed(0)} MB)`, "read at `time` (drop the second argument), or keep every step: a smaller size, a lower rate or a shorter span");
        }
        // The frame replayed n and n + 1 into the even and odd tables; which holds n depends on the replay's parity.
        const r = replay!;
        const n = r.step;
        // mod divides (n / K is exact in f32 for whole numbers); a multiply by 1 / K could land just below a whole number.
        const even = eq(mod(mod(n, every), 2), 0);
        const a = read(r.even, 0, j), b = read(r.odd, 0, j);
        const t = saturate(sub(mul(sub(time, state.start), rate), n));
        return a.map((x, l) => mix(select(even, x, b[l]!), select(even, b[l]!, x), t));
      }
    }
  };
  const on = (u: C extends number ? In<"f32"> : In<"vec2">, when?: In<"f32">): Lanes => {
    switch (shape.kind) {
      case "list": {
        if (shape.n === 1) return at(0 as Index<C>, when);
        const x = mul(clamp(lift<"f32">(u as In<"f32">), 0, 1), shape.n - 1);
        const i0 = min(floor(x), shape.n - 2);
        const a = at(i0 as Index<C>, when), b = at(add(i0, 1) as Index<C>, when);
        return a.map((v, l) => mix(v, b[l]!, sub(x, i0)));
      }
      case "grid": {
        // Cell centres at (i + 0.5) / w: bilinear between the four nearest, wrapping at the edges (a torus).
        const p = lift<"vec2">(u as In<"vec2">);
        const x = sub(mul(p.x, shape.w), 0.5), y = sub(mul(p.y, shape.h), 0.5);
        const x0 = floor(x), y0 = floor(y), fx = sub(x, x0), fy = sub(y, y0);
        const cell = (dx: number, dy: number) => at(vec2(add(x0, dx), add(y0, dy)) as Index<C>, when);
        const c00 = cell(0, 0), c10 = cell(1, 0), c01 = cell(0, 1), c11 = cell(1, 1);
        return c00.map((v, l) => mix(mix(v, c10[l]!, fx), mix(c01[l]!, c11[l]!, fx), fy));
      }
    }
  };
  return { at, on } as Recurrence<C>;
}
