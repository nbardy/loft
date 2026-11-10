// Per-frame tables: the registry they are declared into (by expressions that `fetch` them, or explicitly by raw WGSL),
// their packing into storage buffers, and the pose function that fills them (DESIGN.md section 3).
//
// Packing: a shader stage may bind at most 8 storage buffers (WebGPU's default `maxStorageBuffersPerShaderStage`) and
// the prelude's group 0 uses 3 (inputs, params, curves), so group 1 has 5. Tables are small and many, so they are
// packed, in declaration order, into as few buffers as hold them (first fit, each at most one binding's worth); a
// table's accessor adds its offset. Every buffer is visible to every shader, so which drawable reads what never
// matters to the layout. A scene that needs more than fits fails here, by name, not at pipeline creation.

import type { DynamicBuffer, FrameState } from "../artefact.ts";
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
type Placed = { readonly table: TableSource; readonly fill: TableFill; readonly buffer: number; readonly offset: number };
export type TableLayout = {
  readonly dynamic: readonly DynamicBuffer[];
  /** Bind group 1 declarations and one accessor per table. */
  readonly wgsl: string;
  readonly placed: ReadonlyMap<TableSource, Placed>;
  /** Entries of the tables whose role is "stations". */
  readonly stations: number;
};

const IDENT = /^[A-Za-z][A-Za-z0-9_]*$/;
const accessor = (name: string): string => `t_${name}`;
export const bufferName = (k: number): string => `loft_tables_${k}`;
const vec4sOf = (t: TableSource): number => t.entries * t.lanes;
const UNFILLED: TableFill = () => { throw new Error("internal: a table was read while it was being prepared"); };

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
    const sizes: number[] = [];
    const placed = new Map<TableSource, Placed>();
    for (const { table, fill } of this.declared.values()) {
      const n = vec4sOf(table);
      if (n > BINDING_VEC4S) throw new TableBudgetError(`table "${table.name}" needs ${n} vec4s; one storage binding holds at most ${BINDING_VEC4S}`);
      let k = sizes.findIndex((s) => s + n <= BINDING_VEC4S);
      if (k < 0) k = sizes.push(0) - 1;
      placed.set(table, { table, fill, buffer: k, offset: sizes[k] });
      sizes[k] += n;
    }
    if (sizes.length > TABLE_BUFFERS) {
      const all = [...this.declared.keys()].map((t) => `${t.name} ${vec4sOf(t)}`).join(", ");
      throw new TableBudgetError(`the scene's tables need ${sizes.length} storage buffers of ${BINDING_VEC4S} vec4s; a shader stage can bind ${TABLE_BUFFERS} besides the prelude's ${PRELUDE_STORAGE_BUFFERS} (tables: ${all})`);
    }
    // A table-less scene still declares one (tiny) buffer, so bind group 1 is never empty.
    if (sizes.length === 0) sizes.push(1);
    const dynamic = sizes.map((vec4s, k): DynamicBuffer => ({ name: bufferName(k), binding: k, vec4s }));
    const wgsl = [
      ...dynamic.map((d) => `@group(1) @binding(${d.binding}) var<storage, read> ${d.name}: array<vec4<f32>>;`),
      ...[...placed.values()].map((p) => `fn ${accessor(p.table.name)}(i: u32) -> vec4<f32> { return ${bufferName(p.buffer)}[${p.offset}u + i]; }`),
    ].join("\n");
    const stations = [...placed.keys()].filter((t) => t.role === "stations").reduce((n, t) => n + t.entries, 0);
    return { dynamic, wgsl, placed, stations };
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
    for (const t of layout.placed.keys()) frame.table(t);
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
