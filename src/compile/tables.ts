// Per-frame tables: the registry they are declared into (by expressions that `fetch` them, or explicitly by raw WGSL),
// their packing into storage buffers, the schedule that fills them on the GPU, and the CPU reading of them (DESIGN.md
// section 3, docs/gpu_native.md phase 1).
//
// Programs. Each table is classified once, when it is declared, into one canonical form (`Canon`): a map, a scan, or a
// `cpu` fill (a hand-written `prepare`, or a table with no program at all, which is read as one). A map or a scan is
// expanded there, once: its run lanes and its scan step are called with fresh variables and the resulting expressions
// registered, so every leaf they read (inputs, params, curve rows, other tables) has a slot before the layout. From
// that one description come both the WGSL that fills the table at run time (`stageWgsl`) and the CPU fill (`cpuFill`).
//
// Schedule. A GPU table's level is one more than the deepest GPU table it reads; tables of one level are one compute
// dispatch (one invocation per map entry, one per scan). A `cpu` table is filled and uploaded before the dispatches. A
// `cpu` table may not wait on the GPU, so a map or scan that a `cpu` table reads is filled on the CPU too, and says so
// (`TableFillKind` "cpu", why "read by ...").
//
// Packing: a shader stage may bind at most 8 storage buffers (WebGPU's default `maxStorageBuffersPerShaderStage`) and
// the prelude's group 0 uses 3 (inputs, params, curves), so group 1 has 5. Tables are small and many, so they are
// packed, in declaration order, into as few buffers as hold them (first fit, each at most one binding's worth); a
// table's accessor adds its offset. Every buffer is visible to every shader, so which drawable reads what never
// matters to the layout. A scene that needs more than fits fails here, by name, not at pipeline creation.
//
// Static tables (role "static": a distance brick, a sculpt layer, a keyed table) are packed the same way into buffers of
// their own, after the per-frame ones, and filled here, once, on the CPU: the host uploads them at load
// (`Artefact.statics`), so a brick of megabytes costs nothing per frame. They share the binding budget with the
// per-frame buffers.

import { POSE_WORKGROUP, type CpuUpload, type DynamicBuffer, type FrameState, type PoseStage, type StaticBuffer, type TableFillKind, type TableInfo } from "../artefact.ts";
import {
  children, emitWgsl, fetch, leaves, listOf, replace, splitOn, topo, variable, vec4, WGSL_TY, type Bind, type Env, type Expr, type LibFn, type NumTy,
  type TableFill, type TableFrame, type TablePrep, type TableSource, type V,
} from "../expr.ts";
import type { TableRef } from "./context.ts";

/** WebGPU's default limits, which every adapter supports. */
export const STORAGE_BUFFERS_PER_STAGE = 8;
export const PRELUDE_STORAGE_BUFFERS = 3;
export const TABLE_BUFFERS = STORAGE_BUFFERS_PER_STAGE - PRELUDE_STORAGE_BUFFERS;
/** `maxStorageBufferBindingSize` default: 128 MiB, in vec4s. */
export const BINDING_VEC4S = (128 * 1024 * 1024) / 16;

/** The scene's per-frame tables do not fit the storage buffers a shader stage can bind. */
export class TableBudgetError extends Error {
  override readonly name = "TableBudgetError";
}

/** What the registry needs from the compiler: slots for leaves, CPU code, and the WGSL spelling of leaves. */
export type TableCompiler = {
  /** Give every leaf of these expressions a slot (declaring the tables they fetch). */
  register(roots: readonly Expr[]): void;
  /** CPU code for expressions (leaves read from an `Env`, free variables from `Env.vars`). */
  cpu(roots: readonly Expr[]): (env: Env) => (V | boolean)[];
  /** The WGSL binding of leaves, free variables mapped through `vars`. */
  wgsl(vars: Readonly<Record<string, string>>): Bind;
};

/** A map run expanded: entries `start .. start + count - 1`, each `lanes` at the run index `i`. */
type Run = { readonly start: number; readonly count: number; readonly i: Expr<"f32">; readonly lanes: readonly Expr<"vec4">[] };
/** A table's program, expanded once (see the header). `reads`: the tables its expressions fetch. */
type Canon =
  | { readonly kind: "map"; readonly runs: readonly Run[]; readonly reads: readonly TableSource[] }
  | {
    readonly kind: "scan"; readonly first: number; readonly steps: number; readonly init: readonly Expr<NumTy>[];
    readonly state: readonly Expr<NumTy>[]; readonly i: Expr<"f32">; readonly next: readonly Expr<NumTy>[]; readonly lanes: readonly Expr<"vec4">[];
    readonly reads: readonly TableSource[];
  }
  /** `reads`: the tables its fill compiles expressions over (a fill may also read others through `frame.table`). */
  | { readonly kind: "cpu"; readonly why: string; readonly fill: TableFill; readonly reads: readonly TableSource[] };

type Declared = { readonly table: TableSource; canon: Canon | undefined };
/** Where a table lives, its program, and how it is filled after scheduling. */
type Placed = {
  readonly table: TableSource;
  readonly canon: Canon;
  readonly kind: "dynamic" | "static";
  readonly buffer: number;
  readonly offset: number;
  /** For a dynamic table: where it is filled. Static tables are filled at build (`cpu` here, unused). */
  readonly fill: TableFillKind;
  /** The CPU fill (derived from the program for maps and scans; compiled on first use). */
  readonly cpu: TableFill;
};
export type TableLayout = {
  readonly dynamic: readonly DynamicBuffer[];
  readonly statics: readonly StaticBuffer[];
  /** Bind group 1 declarations and one accessor per table. */
  readonly wgsl: string;
  readonly placed: ReadonlyMap<TableSource, Placed>;
  /** Entries of the tables whose role is "stations". */
  readonly stations: number;
  /** The pose pass's compute dispatches, in order. */
  readonly stages: readonly PoseStage[];
  /** Every dynamic table: where it is and how it is filled. */
  readonly info: readonly TableInfo[];
};

const IDENT = /^[A-Za-z][A-Za-z0-9_]*$/;
const accessor = (name: string): string => `t_${name}`;
export const bufferName = (k: number): string => `loft_tables_${k}`;
const staticName = (k: number): string => `loft_static_${k}`;
const vec4sOf = (t: TableSource): number => t.entries * t.lanes;
/** What a static table's fill is handed: no frame. Reading it is a mistake in the table (it is not static). */
const noFrame = (name: string): TableFrame => {
  const fail = (): never => { throw new Error(`table "${name}" is static but its fill read the frame (a table that follows the frame must have role "data")`); };
  return { get env(): Env { return fail(); }, table: fail };
};

/** First fit of tables into buffers of at most one binding each: (buffer, offset) per table, and each buffer's size. */
function pack(tables: readonly TableSource[]): { at: ReadonlyMap<TableSource, { buffer: number; offset: number }>; sizes: number[] } {
  const sizes: number[] = [];
  const at = new Map<TableSource, { buffer: number; offset: number }>();
  for (const t of tables) {
    const n = vec4sOf(t);
    if (n > BINDING_VEC4S) throw new TableBudgetError(`table "${t.name}" needs ${n} vec4s; one storage binding holds at most ${BINDING_VEC4S}`);
    let k = sizes.findIndex((s) => s + n <= BINDING_VEC4S);
    if (k < 0) k = sizes.push(0) - 1;
    at.set(t, { buffer: k, offset: sizes[k] });
    sizes[k] += n;
  }
  return { at, sizes };
}

let nextProgram = 0;
/** A static table's fill: on the CPU, once, at build. */
const STATIC: TableFillKind = { kind: "cpu", why: "filled once, at build" };

/**
 * Classify a declared table into its canonical form, once (the boundary: nothing after this asks what a table is).
 * `prep` is the compiler's, for hand-written fills; `c` registers a program's expressions.
 */
function classify(t: TableSource, prep: TablePrep, c: TableCompiler): Canon {
  if (!("program" in t)) {
    const reads: TableSource[] = [];
    const fill = t.prepare({ compile: (roots) => { reads.push(...leaves(roots).tables); return prep.compile(roots); } });
    const why = t.role === "static" ? "filled once, at build" : "a hand-written fill with no program";
    return { kind: "cpu", why, fill, reads };
  }
  const p = t.program;
  const id = nextProgram++;
  const checkLanes = (lanes: readonly Expr[], what: string): void => {
    if (lanes.length !== t.lanes) throw new Error(`table "${t.name}": ${what} gives ${lanes.length} lanes; the table has ${t.lanes}`);
    lanes.forEach((l, k) => { if (l.ty !== "vec4") throw new Error(`table "${t.name}": ${what} lane ${k} is ${l.ty}, not vec4`); });
  };
  switch (p.kind) {
    case "map": {
      let start = 0;
      const runs = p.runs.map((r, k): Run => {
        if (!(Number.isInteger(r.count) && r.count >= 1)) throw new Error(`table "${t.name}": run ${k} has count ${r.count}`);
        const i = variable(`_T${id}r${k}`, "f32");
        const lanes = r.lanes(i);
        checkLanes(lanes, `run ${k}`);
        const run = { start, count: r.count, i, lanes };
        start += r.count;
        return run;
      });
      if (start !== t.entries) throw new Error(`table "${t.name}": its runs cover ${start} entries; the table has ${t.entries}`);
      const shared = hoistShared(t.name, runs);
      const roots = shared.runs.flatMap((r) => r.lanes);
      c.register(roots);
      return { kind: "map", runs: shared.runs, reads: leaves(roots).tables };
    }
    case "scan": {
      if (!(Number.isInteger(p.first) && Number.isInteger(p.steps) && p.first >= 0 && p.steps >= 1 && p.first + p.steps <= t.entries)) {
        throw new Error(`table "${t.name}": a scan over entries ${p.first} .. ${p.first + p.steps - 1} does not fit its ${t.entries}`);
      }
      const state = p.init.map((e, j) => variable(`_T${id}s${j}`, e.ty));
      const i = variable(`_T${id}i`, "f32");
      const { next, lanes } = p.step(state, i);
      if (next.length !== p.init.length) throw new Error(`table "${t.name}": the scan's step returned ${next.length} state components; it has ${p.init.length}`);
      next.forEach((e, j) => { if (e.ty !== p.init[j]!.ty) throw new Error(`table "${t.name}": state component ${j} is ${p.init[j]!.ty} but the step returned ${e.ty}`); });
      checkLanes(lanes, "the scan's step");
      const roots = [...p.init, ...next, ...lanes];
      c.register(roots);
      return { kind: "scan", first: p.first, steps: p.steps, init: p.init, state, i, next, lanes, reads: leaves(roots).tables };
    }
    case "cpu": {
      const reads: TableSource[] = [];
      const fill = p.prepare({ compile: (roots) => { reads.push(...leaves(roots).tables); return prep.compile(roots); } });
      return { kind: "cpu", why: p.why, fill, reads };
    }
  }
}

/** Fewer shared nodes than this stay where they are: a level of its own costs more than recomputing them. */
const HOIST_MIN = 8;
const padded = (e: Expr<NumTy>): Expr<"vec4"> => {
  switch (e.ty) {
    case "f32": return vec4(e, 0, 0, 0);
    case "vec2": return vec4(e, 0, 0);
    case "vec3": return vec4(e, 0);
    case "vec4": return e as Expr<"vec4">;
  }
};
const unpadded = (ty: NumTy, e: Expr<"vec4">): Expr => {
  switch (ty) {
    case "f32": return e.x;
    case "vec2": return e.xy;
    case "vec3": return e.xyz;
    case "vec4": return e;
  }
};

/**
 * Work that two or more runs of a map share, computed once. On the GPU each run is a branch, and the invocations of one
 * SIMD group that take different branches run one after another: a list of ropes whose ends all ride one ship
 * recomputed the ship's sea-following frame in every branch, serially (2 MB of WGSL and about a millisecond for the
 * crab scene's rigging and shadows, 2026-11-11). The CPU fill had shared it within one compiled function. So the nodes
 * that more than one run reads (and that do not depend on a run's index) are evaluated by a table of their own, one
 * invocation with one lane per value, a level earlier, and the runs read them from it.
 */
function hoistShared(name: string, runs: readonly Run[]): { runs: readonly Run[] } {
  if (runs.length < 2) return { runs };
  const trivial = (e: Expr): boolean => {
    const op = e.node.op;
    return op === "const" || op === "var" || op === "input" || op === "param" || e.ty === "bool";
  };
  const seen = new Map<number, number>(), node = new Map<number, Expr>();
  const lists = runs.map((r) => {
    const inner = new Set(splitOn(r.lanes, [r.i]).inner.map((e) => e.id));
    const list = topo(r.lanes);
    for (const e of list) {
      if (inner.has(e.id) || e === r.i || trivial(e)) continue;
      seen.set(e.id, (seen.get(e.id) ?? 0) + 1);
      node.set(e.id, e);
    }
    return list;
  });
  const shared = new Set([...seen].filter(([, n]) => n >= 2).map(([id]) => id));
  if (shared.size < HOIST_MIN) return { runs };
  // The frontier: shared values read by something that is not shared, or that are lanes themselves.
  const frontier = new Map<number, Expr<NumTy>>();
  lists.forEach((list, k) => {
    for (const e of list) if (!shared.has(e.id)) for (const c of children(e)) if (shared.has(c.id)) frontier.set(c.id, c as Expr<NumTy>);
    for (const l of runs[k]!.lanes) if (shared.has(l.id)) frontier.set(l.id, l);
  });
  const values = [...frontier.values()];
  const common: TableSource = { name: `${name}_common`, lanes: values.length, entries: 1, role: "data", program: listOf([values.map(padded)]) };
  const by = new Map(values.map((v, j) => [v.id, unpadded(v.ty as NumTy, fetch(common, 0, j))]));
  return { runs: runs.map((r) => ({ ...r, lanes: replace(r.lanes, by) as Expr<"vec4">[] })) };
}

/** The CPU fill a program gives (compiled on first use: most compilations never read a table on the CPU). */
function cpuFill(t: TableSource, canon: Canon, c: TableCompiler): TableFill {
  switch (canon.kind) {
    case "cpu": return canon.fill;
    case "map": {
      const L = t.lanes * 4;
      let runs: ((env: Env) => (V | boolean)[])[] | undefined;
      return (frame, out) => {
        runs ??= canon.runs.map((r) => c.cpu(r.lanes));
        canon.runs.forEach((r, k) => {
          const run = runs![k]!, name = (r.i.node as { name: string }).name;
          const vars: Record<string, number> = { [name]: 0 };
          const env: Env = { ...frame.env, vars };
          for (let i = 0; i < r.count; i++) {
            vars[name] = i;
            const v = run(env);
            for (let l = 0; l < t.lanes; l++) out.set(v[l] as number[], (r.start + i) * L + l * 4);
          }
        });
      };
    }
    case "scan": {
      const L = t.lanes * 4, n = canon.next.length;
      let code: { init: (env: Env) => (V | boolean)[]; step: (env: Env) => (V | boolean)[] } | undefined;
      const names = canon.state.map((s) => (s.node as { name: string }).name), iName = (canon.i.node as { name: string }).name;
      return (frame, out) => {
        code ??= { init: c.cpu(canon.init), step: c.cpu([...canon.next, ...canon.lanes]) };
        const vars: Record<string, V> = {};
        const env: Env = { ...frame.env, vars };
        let s = code.init(env) as V[];
        for (let k = 0; k < canon.steps; k++) {
          names.forEach((name, j) => { vars[name] = s[j]!; });
          vars[iName] = k;
          const v = code.step(env);
          for (let l = 0; l < t.lanes; l++) out.set(v[n + l] as number[], (canon.first + k) * L + l * 4);
          s = v.slice(0, n) as V[];
        }
      };
    }
  }
}

export class Tables {
  private readonly declared = new Map<TableSource, Declared>();
  private readonly names = new Map<string, TableSource>();
  private sealed = false;

  constructor(private readonly c: TableCompiler) {}

  /** Declare a table (once per object; later calls return the same accessor) and classify its program. */
  declare(t: TableSource, prep: TablePrep): TableRef {
    const ref = { name: t.name, wgsl: accessor(t.name) };
    if (this.declared.has(t)) return ref;
    if (this.sealed) throw new Error(`table "${t.name}" was first used after the artefact was laid out`);
    if (!IDENT.test(t.name)) throw new Error(`table "${t.name}": names must be identifiers (they name WGSL functions)`);
    if (this.names.has(t.name)) throw new Error(`two different tables are named "${t.name}" (two things with one name?)`);
    if (!(Number.isInteger(t.entries) && t.entries >= 1 && Number.isInteger(t.lanes) && t.lanes >= 1)) {
      throw new Error(`table "${t.name}": entries and lanes must be positive integers, got ${t.entries} x ${t.lanes}`);
    }
    // Registered before it is classified: classifying declares the tables this one reads (a host's), and those come
    // after it in the layout, which is harmless; levels and first reads decide fill order, not declaration order.
    const d: Declared = { table: t, canon: undefined };
    this.declared.set(t, d);
    this.names.set(t.name, t);
    d.canon = classify(t, prep, this.c);
    return ref;
  }

  layout(): TableLayout {
    this.sealed = true;
    const all = [...this.declared.values()].map((d) => ({ table: d.table, canon: d.canon! }));
    const perFrame = pack(all.filter((d) => d.table.role !== "static").map((d) => d.table));
    const once = pack(all.filter((d) => d.table.role === "static").map((d) => d.table));
    // A table-less scene still declares one (tiny) per-frame buffer, so bind group 1 is never empty.
    if (perFrame.sizes.length === 0) perFrame.sizes.push(1);
    if (perFrame.sizes.length + once.sizes.length > TABLE_BUFFERS) {
      const list = all.map((d) => `${d.table.name} ${vec4sOf(d.table)}${d.table.role === "static" ? " (static)" : ""}`).join(", ");
      throw new TableBudgetError(`the scene's tables need ${perFrame.sizes.length} per-frame and ${once.sizes.length} static storage buffers of up to ${BINDING_VEC4S} vec4s; a shader stage can bind ${TABLE_BUFFERS} besides the prelude's ${PRELUDE_STORAGE_BUFFERS} (tables: ${list})`);
    }
    const fills = schedule(all);
    const placed = new Map<TableSource, Placed>();
    for (const { table, canon } of all) {
      const kind = table.role === "static" ? "static" : "dynamic";
      const fill: TableFillKind = kind === "static" ? STATIC : fills.get(table)!;
      placed.set(table, { table, canon, kind, ...(kind === "static" ? once : perFrame).at.get(table)!, fill, cpu: cpuFill(table, canon, this.c) });
    }
    const dynamic = perFrame.sizes.map((vec4s, k): DynamicBuffer => ({ name: bufferName(k), binding: k, vec4s }));
    const data = once.sizes.map((vec4s) => new Float32Array(vec4s * 4));
    for (const p of placed.values()) {
      if (p.kind === "static") p.cpu(noFrame(p.table.name), data[p.buffer]!.subarray(p.offset * 4, (p.offset + vec4sOf(p.table)) * 4));
    }
    const statics = data.map((d, k): StaticBuffer => ({ name: staticName(k), binding: dynamic.length + k, data: d }));
    const wgsl = groupWgsl(dynamic, statics, placed, "read");
    const stations = [...placed.keys()].filter((t) => t.role === "stations").reduce((n, t) => n + t.entries, 0);
    const info = [...placed.values()].filter((p) => p.kind === "dynamic").map((p): TableInfo => ({ name: p.table.name, buffer: bufferName(p.buffer), offset: p.offset, vec4s: vec4sOf(p.table), lanes: p.table.lanes, fill: p.fill }));
    const stages = poseStages(placed, groupWgsl(dynamic, statics, placed, "read_write"), this.c);
    return { dynamic, statics, wgsl, placed, stations, stages, info };
  }
}

/** Bind group 1 declarations and one accessor per table; `access` is how the dynamic buffers are bound. */
function groupWgsl(dynamic: readonly DynamicBuffer[], statics: readonly StaticBuffer[], placed: ReadonlyMap<TableSource, Placed>, access: "read" | "read_write"): string {
  const nameOf = (p: Placed): string => (p.kind === "static" ? staticName(p.buffer) : bufferName(p.buffer));
  return [
    ...dynamic.map((d) => `@group(1) @binding(${d.binding}) var<storage, ${access}> ${d.name}: array<vec4<f32>>;`),
    ...statics.map((d) => `@group(1) @binding(${d.binding}) var<storage, read> ${d.name}: array<vec4<f32>>;`),
    ...[...placed.values()].map((p) => `fn ${accessor(p.table.name)}(i: u32) -> vec4<f32> { return ${nameOf(p)}[${p.offset}u + i]; }`),
  ].join("\n");
}

/**
 * Where each table is filled: a dynamic map or scan on the GPU at its level, unless a `cpu` table reads it (then on the
 * CPU, and so is everything it reads); a `cpu` table on the CPU. Throws on a cycle among GPU tables.
 */
function schedule(all: readonly { table: TableSource; canon: Canon }[]): Map<TableSource, TableFillKind> {
  const canonOf = new Map(all.map((d) => [d.table, d.canon]));
  const out = new Map<TableSource, TableFillKind>();
  const force = (t: TableSource, by: string): void => {
    if (t.role === "static" || out.has(t)) return;
    const c = canonOf.get(t)!;
    out.set(t, { kind: "cpu", why: c.kind === "cpu" ? c.why : `read by ${by}, which is filled on the CPU` });
    for (const r of c.reads) force(r, t.name);
  };
  for (const { table, canon } of all) if (canon.kind === "cpu") force(table, table.name);
  const level = new Map<TableSource, number>();
  const visiting: TableSource[] = [];
  const levelOf = (t: TableSource): number => {
    const known = level.get(t);
    if (known !== undefined) return known;
    if (visiting.includes(t)) throw new Error(`pose: table "${t.name}" depends on itself (through ${[...visiting, t].map((x) => x.name).join(" -> ")})`);
    visiting.push(t);
    const deps = canonOf.get(t)!.reads.filter((r) => r.role !== "static" && out.get(r)?.kind !== "cpu");
    const l = deps.reduce((m, r) => Math.max(m, levelOf(r) + 1), 0);
    visiting.pop();
    level.set(t, l);
    return l;
  };
  for (const { table, canon } of all) {
    if (table.role === "static" || out.has(table)) continue;
    switch (canon.kind) {
      case "map": out.set(table, { kind: "map", level: levelOf(table) }); break;
      case "scan": out.set(table, { kind: "scan", level: levelOf(table) }); break;
      case "cpu": throw new Error("internal: a cpu table was not scheduled on the CPU");
    }
  }
  return out;
}

// ---- the pose stages' WGSL ------------------------------------------------------------------------------------------

/**
 * The statement that runs `calls[k]` when `x` lies in [starts[k], starts[k + 1]) (starts ascending, from 0; x below the
 * end of the last range): a balanced tree of comparisons. A chain of `else if` would be as long as the list, and WGSL
 * limits statement nesting and chaining to 127 (a list of 255 shadow joints hit it).
 */
function rangeTree(x: string, starts: readonly number[], calls: readonly string[]): string {
  const tree = (lo: number, hi: number): string => {
    if (hi - lo === 1) return calls[lo]!;
    const mid = (lo + hi) >> 1;
    return `if (${x} < ${starts[mid]}u) { ${tree(lo, mid)} } else { ${tree(mid, hi)} }`;
  };
  return tree(0, calls.length);
}

/** One table's code in a stage module: its functions, its invocations, and the call from `pose_main` (local index `e`). */
type TableCode = { readonly fns: string[]; readonly libs: readonly LibFn[]; readonly invocations: number; readonly call: string };

/** `loft_tables_k[offset + entry * lanes + lane] = value;` for each lane. */
const writes = (p: Placed, entry: string, outs: readonly string[]): string[] =>
  outs.map((v, l) => `${bufferName(p.buffer)}[${p.offset}u + (${entry}) * ${p.table.lanes}u + ${l}u] = ${v};`);

/**
 * Invocations of one SIMD group (Apple GPUs: 32). The invocations of a group that take different branches run one
 * after another, so each table, and each run of a table of several runs, starts a group of its own: the scans of one
 * level, or a list's entries, then run side by side instead of in turn (measured on the crab scene, 2026-11-11).
 */
const SIMD = 32;
const simdUp = (n: number): number => Math.ceil(n / SIMD) * SIMD;

function mapCode(p: Placed, canon: Extract<Canon, { kind: "map" }>, c: TableCompiler, fn: string): TableCode {
  const fns: string[] = [];
  const libs: LibFn[] = [];
  canon.runs.forEach((r, k) => {
    const name = (r.i.node as { name: string }).name;
    const { lines, out, libs: l } = emitWgsl(r.lanes, c.wgsl({ [name]: "f32(i)" }));
    libs.push(...l);
    fns.push([`fn ${fn}_${k}(i: u32) {`, ...lines.map((x) => `  ${x}`), ...writes(p, `${r.start}u + i`, out).map((x) => `  ${x}`), "}"].join("\n"));
  });
  // Runs by range (a run of one entry is a list element), each from a SIMD group of its own when there are several.
  const one = canon.runs.length === 1;
  let end = 0;
  const starts = canon.runs.map((r) => { const at = end; end += one ? r.count : simdUp(r.count); return at; });
  const calls = canon.runs.map((r, k) => `{ let i = e - ${starts[k]}u; if (i < ${r.count}u) { ${fn}_${k}(i); } }`);
  fns.push(`fn ${fn}(e: u32) {\n  ${rangeTree("e", starts, calls)}\n}`);
  return { fns, libs, invocations: end, call: `${fn}(e)` };
}

function scanCode(p: Placed, canon: Extract<Canon, { kind: "scan" }>, c: TableCompiler, fn: string): TableCode {
  const tys = canon.init.map((e) => WGSL_TY[e.ty]);
  const iName = (canon.i.node as { name: string }).name;
  const vars: Record<string, string> = { [iName]: "f32(k)" };
  canon.state.forEach((s, j) => { vars[(s.node as { name: string }).name] = `s${j}`; });
  const bind = c.wgsl(vars);
  const init = emitWgsl(canon.init, bind, "a");
  // Work the step does not need the state or the index for is done once, before the loop.
  const body = [...canon.next, ...canon.lanes];
  const { outer } = splitOn(body, [...canon.state, canon.i]);
  const hoisted = emitWgsl(outer, bind, "h");
  const step = emitWgsl(body, bind, "e", hoisted.names);
  const n = canon.next.length;
  const lines = [
    ...init.lines,
    ...tys.map((ty, j) => `var s${j}: ${ty} = ${init.out[j]};`),
    ...hoisted.lines,
    `for (var k = 0u; k < ${canon.steps}u; k++) {`,
    ...step.lines.map((x) => `  ${x}`),
    ...writes(p, `${canon.first}u + k`, step.out.slice(n)).map((x) => `  ${x}`),
    // Every next value is read before any is assigned (a step may swap two state components).
    ...step.out.slice(0, n).map((v, j) => `  let n${j}: ${tys[j]} = ${v};`),
    ...tys.map((_, j) => `  s${j} = n${j};`),
    "}",
  ];
  return { fns: [`fn ${fn}() {\n${lines.map((x) => `  ${x}`).join("\n")}\n}`], libs: [...init.libs, ...hoisted.libs, ...step.libs], invocations: 1, call: `${fn}()` };
}

function tableCode(p: Placed, c: TableCompiler, fn: string): TableCode {
  const canon = p.canon;
  switch (canon.kind) {
    case "map": return mapCode(p, canon, c, fn);
    case "scan": return scanCode(p, canon, c, fn);
    case "cpu": throw new Error(`internal: table "${p.table.name}" is filled on the CPU and has no stage code`);
  }
}

/** One compute module per level: every GPU table of that level, its invocations laid end to end. */
function poseStages(placed: ReadonlyMap<TableSource, Placed>, group1: string, c: TableCompiler): PoseStage[] {
  const byLevel: Placed[][] = [];
  for (const p of placed.values()) {
    if (p.kind !== "dynamic") continue;
    const f = p.fill;
    switch (f.kind) {
      case "cpu": break;
      case "map": case "scan": (byLevel[f.level] ??= []).push(p); break;
    }
  }
  return byLevel.map((tables, level): PoseStage => {
    const codes = tables.map((p) => tableCode(p, c, `pose_${p.table.name}`));
    const libs: LibFn[] = [];
    for (const code of codes) for (const l of code.libs) if (!libs.includes(l)) libs.push(l);
    let end = 0;
    const starts = codes.map((code) => { const start = end; end += simdUp(code.invocations); return start; });
    const wgsl = [
      group1,
      ...libs.map((l) => l.wgsl),
      ...codes.flatMap((code) => code.fns),
      `@compute @workgroup_size(${POSE_WORKGROUP}) fn pose_main(@builtin(global_invocation_id) g: vec3<u32>) {`,
      `  if (g.x >= ${end}u) { return; }`,
      `  ${rangeTree("g.x", starts, codes.map((code, k) => `{ let e = g.x - ${starts[k]}u; if (e < ${code.invocations}u) { ${code.call}; } }`))}`,
      "}",
    ].join("\n");
    return { label: `pose level ${level}: ${tables.map((p) => p.table.name).join(", ")}`, wgsl, invocations: end };
  });
}

// ---- the CPU reading ------------------------------------------------------------------------------------------------

/** One frame's buffers, with every table filled on first read (so a table may read another; a cycle throws). */
export function poseFrame(layout: TableLayout, env: (state: FrameState) => Omit<Env, "tables">, state: FrameState, clock?: Map<string, number>): { frame: TableFrame; buffers: Float32Array[] } {
  const buffers = layout.dynamic.map((d) => new Float32Array(d.vec4s * 4));
  const filled = new Set<TableSource>(), filling: TableSource[] = [];
  const children: number[] = [];
  const table = (t: TableSource): Float32Array => {
    const p = layout.placed.get(t);
    if (!p) throw new Error(`pose: the table "${t.name}" is not in this artefact (nothing drawn reads it)`);
    if (p.kind === "static") return layout.statics[p.buffer]!.data.subarray(p.offset * 4, (p.offset + vec4sOf(t)) * 4);
    const view = buffers[p.buffer].subarray(p.offset * 4, (p.offset + vec4sOf(t)) * 4);
    if (filled.has(t)) return view;
    if (filling.includes(t)) throw new Error(`pose: table "${t.name}" depends on itself (through ${[...filling, t].map((x) => x.name).join(" -> ")})`);
    filling.push(t);
    const t0 = performance.now();
    children.push(0);
    p.cpu(frame, view);
    // Exclusive time: the tables this fill read were filled inside it and charged their own time.
    const spent = performance.now() - t0, inner = children.pop()!;
    clock?.set(t.name, spent - inner);
    if (children.length > 0) children[children.length - 1] += spent;
    filling.pop();
    filled.add(t);
    return view;
  };
  const frame: TableFrame = { env: { ...env(state), tables: table }, table };
  return { frame, buffers };
}

/** The artefact's pose function (the CPU reading): fresh zeroed buffers each call, every table filled. */
export function poseFunction(layout: TableLayout, env: (state: FrameState) => Omit<Env, "tables">): (state: FrameState) => Record<string, Float32Array> {
  return (state) => {
    const { frame, buffers } = poseFrame(layout, env, state);
    for (const p of layout.placed.values()) if (p.kind === "dynamic") frame.table(p.table);
    return Object.fromEntries(layout.dynamic.map((d, k) => [d.name, buffers[k]]));
  };
}

const onCpu = (p: Placed): boolean => p.kind === "dynamic" && p.fill.kind === "cpu";

/**
 * The host's per-frame CPU work: every `cpu` table, filled into its own array. A `cpu` fill reading a table the GPU
 * fills is an error (the scheduler moved every table a `cpu` table compiles reads of to the CPU; one read only through
 * `frame.table` was not seen, and would silently cost a CPU evaluation of that table every frame).
 */
export function cpuPoseFunction(layout: TableLayout, env: (state: FrameState) => Omit<Env, "tables">): (state: FrameState) => CpuUpload[] {
  const list = [...layout.placed.values()].filter(onCpu);
  return (state) => {
    const data = new Map<TableSource, Float32Array>();
    const filling: TableSource[] = [];
    const table = (t: TableSource): Float32Array => {
      const p = layout.placed.get(t);
      if (!p) throw new Error(`pose: the table "${t.name}" is not in this artefact (nothing drawn reads it)`);
      if (p.kind === "static") return layout.statics[p.buffer]!.data.subarray(p.offset * 4, (p.offset + vec4sOf(t)) * 4);
      if (!onCpu(p)) throw new Error(`pose: a CPU fill read table "${t.name}", which the GPU fills; read it through an expression (so the scheduler sees it) or port the reader`);
      const seen = data.get(t);
      if (seen) return seen;
      if (filling.includes(t)) throw new Error(`pose: table "${t.name}" depends on itself (through ${[...filling, t].map((x) => x.name).join(" -> ")})`);
      filling.push(t);
      const out = new Float32Array(vec4sOf(t) * 4);
      p.cpu(frame, out);
      filling.pop();
      data.set(t, out);
      return out;
    };
    const frame: TableFrame = { env: { ...env(state), tables: table }, table };
    return list.map((p) => ({ buffer: bufferName(p.buffer), offset: p.offset, data: table(p.table) }));
  };
}

/**
 * CPU milliseconds per `cpu` table for one frame, by name: each fill's own time, without the tables it read (those count
 * as themselves). For profiling: what the host spends per frame besides uploads (the other tables are on the GPU).
 */
export function poseProfile(layout: TableLayout, env: (state: FrameState) => Omit<Env, "tables">, state: FrameState): Map<string, number> {
  const own = new Map<string, number>();
  const { frame } = poseFrame(layout, env, state, own);
  for (const p of layout.placed.values()) if (onCpu(p)) frame.table(p.table);
  return own;
}
