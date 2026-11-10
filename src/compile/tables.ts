// Per-frame tables: the registry they are declared into (by expressions that `fetch` them, or explicitly by raw WGSL),
// their packing into storage buffers, and the pose function that fills them (DESIGN.md section 3).
//
// Packing: a shader stage may bind at most 8 storage buffers (WebGPU's default `maxStorageBuffersPerShaderStage`) and
// the prelude's group 0 uses 3 (inputs, params, curves), so group 1 has 5. Tables are small and many, so they are
// packed, in declaration order, into as few buffers as hold them (first fit, each at most one binding's worth); a
// table's accessor adds its offset. Every buffer is visible to every shader, so which drawable reads what never
// matters to the layout. A scene that needs more than fits fails here, by name, not at pipeline creation.
//
// Static tables (role "static": a distance brick, a sculpt layer, a keyed table) are packed the same way into buffers of
// their own, after the per-frame ones, and filled here, once: the host uploads them at load (`Artefact.statics`), so a
// brick of megabytes costs nothing per frame. They share the binding budget with the per-frame buffers.

import type { DynamicBuffer, FrameState, StaticBuffer } from "../artefact.ts";
import type { Env, TableFill, TableFrame, TablePrep, TableSource } from "../expr.ts";
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

type Declared = { readonly table: TableSource; fill: TableFill };
/** Where a table lives: a per-frame buffer (filled by the pose pass) or a static one (filled once, here). */
type Placed = { readonly table: TableSource; readonly fill: TableFill; readonly kind: "dynamic" | "static"; readonly buffer: number; readonly offset: number };
export type TableLayout = {
  readonly dynamic: readonly DynamicBuffer[];
  readonly statics: readonly StaticBuffer[];
  /** Bind group 1 declarations and one accessor per table. */
  readonly wgsl: string;
  readonly placed: ReadonlyMap<TableSource, Placed>;
  /** Entries of the tables whose role is "stations". */
  readonly stations: number;
};

const IDENT = /^[A-Za-z][A-Za-z0-9_]*$/;
const accessor = (name: string): string => `t_${name}`;
export const bufferName = (k: number): string => `loft_tables_${k}`;
const staticName = (k: number): string => `loft_static_${k}`;
const vec4sOf = (t: TableSource): number => t.entries * t.lanes;
const UNFILLED: TableFill = () => { throw new Error("internal: a table was read while it was being prepared"); };
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

export class Tables {
  private readonly declared = new Map<TableSource, Declared>();
  private readonly names = new Map<string, TableSource>();
  private sealed = false;

  /** Declare a table (once per object; later calls return the same accessor) and prepare its fill. */
  declare(t: TableSource, prep: TablePrep): TableRef {
    const ref = { name: t.name, wgsl: accessor(t.name) };
    if (this.declared.has(t)) return ref;
    if (this.sealed) throw new Error(`table "${t.name}" was first used after the artefact was laid out`);
    if (!IDENT.test(t.name)) throw new Error(`table "${t.name}": names must be identifiers (they name WGSL functions)`);
    if (this.names.has(t.name)) throw new Error(`two different tables are named "${t.name}" (two things with one name?)`);
    if (!(Number.isInteger(t.entries) && t.entries >= 1 && Number.isInteger(t.lanes) && t.lanes >= 1)) {
      throw new Error(`table "${t.name}": entries and lanes must be positive integers, got ${t.entries} x ${t.lanes}`);
    }
    // Registered before it is prepared: preparing may declare the tables this one reads (a host's), and those come
    // after it in the layout, which is harmless; fills run on first read, not in declaration order.
    const d: Declared = { table: t, fill: UNFILLED };
    this.declared.set(t, d);
    this.names.set(t.name, t);
    d.fill = t.prepare(prep);
    return ref;
  }

  layout(): TableLayout {
    this.sealed = true;
    const all = [...this.declared.values()];
    const perFrame = pack(all.filter((d) => d.table.role !== "static").map((d) => d.table));
    const once = pack(all.filter((d) => d.table.role === "static").map((d) => d.table));
    // A table-less scene still declares one (tiny) per-frame buffer, so bind group 1 is never empty.
    if (perFrame.sizes.length === 0) perFrame.sizes.push(1);
    if (perFrame.sizes.length + once.sizes.length > TABLE_BUFFERS) {
      const list = all.map((d) => `${d.table.name} ${vec4sOf(d.table)}${d.table.role === "static" ? " (static)" : ""}`).join(", ");
      throw new TableBudgetError(`the scene's tables need ${perFrame.sizes.length} per-frame and ${once.sizes.length} static storage buffers of up to ${BINDING_VEC4S} vec4s; a shader stage can bind ${TABLE_BUFFERS} besides the prelude's ${PRELUDE_STORAGE_BUFFERS} (tables: ${list})`);
    }
    const placed = new Map<TableSource, Placed>();
    for (const { table, fill } of all) {
      const kind = table.role === "static" ? "static" : "dynamic";
      placed.set(table, { table, fill, kind, ...(kind === "static" ? once : perFrame).at.get(table)! });
    }
    const dynamic = perFrame.sizes.map((vec4s, k): DynamicBuffer => ({ name: bufferName(k), binding: k, vec4s }));
    const data = once.sizes.map((vec4s) => new Float32Array(vec4s * 4));
    for (const p of placed.values()) {
      if (p.kind === "static") p.fill(noFrame(p.table.name), data[p.buffer]!.subarray(p.offset * 4, (p.offset + vec4sOf(p.table)) * 4));
    }
    const statics = data.map((d, k): StaticBuffer => ({ name: staticName(k), binding: dynamic.length + k, data: d }));
    const nameOf = (p: Placed): string => (p.kind === "static" ? staticName(p.buffer) : bufferName(p.buffer));
    const wgsl = [
      ...[...dynamic, ...statics].map((d) => `@group(1) @binding(${d.binding}) var<storage, read> ${d.name}: array<vec4<f32>>;`),
      ...[...placed.values()].map((p) => `fn ${accessor(p.table.name)}(i: u32) -> vec4<f32> { return ${nameOf(p)}[${p.offset}u + i]; }`),
    ].join("\n");
    const stations = [...placed.keys()].filter((t) => t.role === "stations").reduce((n, t) => n + t.entries, 0);
    return { dynamic, statics, wgsl, placed, stations };
  }
}

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
    p.fill(frame, view);
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

/** The artefact's pose function: fresh zeroed buffers each call, every table filled. */
export function poseFunction(layout: TableLayout, env: (state: FrameState) => Omit<Env, "tables">): (state: FrameState) => Record<string, Float32Array> {
  return (state) => {
    const { frame, buffers } = poseFrame(layout, env, state);
    for (const p of layout.placed.values()) if (p.kind === "dynamic") frame.table(p.table);
    return Object.fromEntries(layout.dynamic.map((d, k) => [d.name, buffers[k]]));
  };
}

/**
 * CPU milliseconds per table for one pose, by name: each fill's own time, without the tables it read (those count as
 * themselves). For profiling; the artefact's pose function does the same work without the clock.
 */
export function poseProfile(layout: TableLayout, env: (state: FrameState) => Omit<Env, "tables">, state: FrameState): Map<string, number> {
  const own = new Map<string, number>();
  const { frame } = poseFrame(layout, env, state, own);
  for (const t of layout.placed.keys()) frame.table(t);
  return own;
}
