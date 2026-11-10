// Per-frame tables: the registry drawables declare into, their packing into storage buffers, and the pose function
// that fills them (DESIGN.md section 3).
//
// Packing: a shader stage may bind at most 8 storage buffers (WebGPU's default `maxStorageBuffersPerShaderStage`) and
// the prelude's group 0 uses 3 (inputs, params, curves), so group 1 has 5. Tables are small and many, so they are
// packed, in declaration order, into as few buffers as hold them (first fit, each at most one binding's worth); a
// table's accessor adds its offset. Every buffer is visible to every shader, so which drawable reads what never
// matters to the layout. A scene that needs more than fits fails here, by name, not at pipeline creation.

import type { DynamicBuffer, FrameState } from "../artefact.ts";
import { compileJs, type Env, type Expr, type RowSource } from "../expr.ts";
import type { CpuRef, PoseFrame, TableRef, TableSpec } from "./context.ts";

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

type Placed = { readonly spec: TableSpec; readonly buffer: number; readonly offset: number };
export type TableLayout = {
  readonly dynamic: readonly DynamicBuffer[];
  /** Bind group 1 declarations and one accessor per table. */
  readonly wgsl: string;
  readonly placed: readonly Placed[];
};

const IDENT = /^[A-Za-z][A-Za-z0-9_]*$/;
const accessor = (name: string): string => `t_${name}`;
const bufferName = (k: number): string => `loft_tables_${k}`;

export class Tables {
  private readonly specs: TableSpec[] = [];

  declare(spec: TableSpec): TableRef {
    if (!IDENT.test(spec.name)) throw new Error(`table "${spec.name}": names must be identifiers (they name WGSL functions)`);
    if (this.specs.some((s) => s.name === spec.name)) throw new Error(`table "${spec.name}" is declared twice`);
    if (!(Number.isInteger(spec.vec4s) && spec.vec4s >= 1)) throw new Error(`table "${spec.name}": vec4s must be a positive integer, got ${spec.vec4s}`);
    this.specs.push(spec);
    return { name: spec.name, wgsl: accessor(spec.name) };
  }

  layout(): TableLayout {
    const sizes: number[] = [];
    const placed: Placed[] = [];
    for (const spec of this.specs) {
      if (spec.vec4s > BINDING_VEC4S) throw new TableBudgetError(`table "${spec.name}" needs ${spec.vec4s} vec4s; one storage binding holds at most ${BINDING_VEC4S}`);
      let k = sizes.findIndex((n) => n + spec.vec4s <= BINDING_VEC4S);
      if (k < 0) k = sizes.push(0) - 1;
      placed.push({ spec, buffer: k, offset: sizes[k] });
      sizes[k] += spec.vec4s;
    }
    if (sizes.length > TABLE_BUFFERS) {
      throw new TableBudgetError(`the scene's tables need ${sizes.length} storage buffers of ${BINDING_VEC4S} vec4s; a shader stage can bind ${TABLE_BUFFERS} besides the prelude's ${PRELUDE_STORAGE_BUFFERS} (tables: ${this.specs.map((s) => `${s.name} ${s.vec4s}`).join(", ")})`);
    }
    // A table-less scene still declares one (tiny) buffer, so bind group 1 is never empty.
    if (sizes.length === 0) sizes.push(1);
    const dynamic = sizes.map((vec4s, k): DynamicBuffer => ({ name: bufferName(k), binding: k, vec4s }));
    const wgsl = [
      ...dynamic.map((d) => `@group(1) @binding(${d.binding}) var<storage, read> ${d.name}: array<vec4<f32>>;`),
      ...placed.map((p) => `fn ${accessor(p.spec.name)}(i: u32) -> vec4<f32> { return ${bufferName(p.buffer)}[${p.offset}u + i]; }`),
    ].join("\n");
    return { dynamic, wgsl, placed };
  }
}

/** Every `ctx.evaluate` request, compiled into one function so requests share their common subexpressions. */
export class CpuProgram {
  private readonly roots: Expr[] = [];
  private readonly starts: number[] = [];

  add(roots: readonly Expr[]): CpuRef {
    this.starts.push(this.roots.length);
    this.roots.push(...roots);
    return { id: this.starts.length - 1, count: roots.length };
  }

  /** All roots, for slot registration. */
  all(): readonly Expr[] { return this.roots; }

  compile(rows: RowSource): { run: (env: Env) => readonly (number | number[] | boolean)[]; slice: (values: readonly (number | number[] | boolean)[], r: CpuRef) => readonly (number | number[] | boolean)[] } {
    const run = compileJs(this.roots, rows);
    return { run, slice: (values, r) => values.slice(this.starts[r.id], this.starts[r.id] + r.count) };
  }
}

/**
 * The artefact's pose function: one fresh zeroed buffer per binding, each table filled on first demand (so a table
 * may read another; a cycle throws), and the CPU program run once if anything reads it.
 */
export function poseFunction(
  layout: TableLayout,
  program: ReturnType<CpuProgram["compile"]>,
  env: (state: FrameState) => Env,
  seaHeight: (env: Env) => (x: number, z: number) => number,
): (state: FrameState) => Record<string, Float32Array> {
  const byName = new Map(layout.placed.map((p) => [p.spec.name, p]));
  return (state) => {
    const buffers = layout.dynamic.map((d) => new Float32Array(d.vec4s * 4));
    const e = env(state);
    const filled = new Set<string>(), filling = new Set<string>();
    let values: readonly (number | number[] | boolean)[] | undefined;
    const frame: PoseFrame = {
      env: e,
      seaHeight: seaHeight(e),
      values: (r) => program.slice((values ??= program.run(e)), r),
      table: (t: TableRef) => {
        const p = byName.get(t.name);
        if (!p) throw new Error(`pose: no table named "${t.name}"`);
        const view = buffers[p.buffer].subarray(p.offset * 4, (p.offset + p.spec.vec4s) * 4);
        if (filled.has(t.name)) return view;
        if (filling.has(t.name)) throw new Error(`pose: table "${t.name}" depends on itself (through ${[...filling].join(" -> ")})`);
        filling.add(t.name);
        p.spec.fill(frame, view);
        filling.delete(t.name);
        filled.add(t.name);
        return view;
      },
    };
    for (const p of layout.placed) frame.table({ name: p.spec.name, wgsl: accessor(p.spec.name) });
    return Object.fromEntries(layout.dynamic.map((d, k) => [d.name, buffers[k]]));
  };
}
