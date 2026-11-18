// Author-side declarations. The GPU-less compiler turns these into data before a host sees them.
import { COMPUTE_TEXEL_BYTES as TEXEL_BYTES, computeHistorySize } from "./host/manifest.ts";
import { SceneError, shown } from "./errors.ts";
import { computeFetch, lift, rawComputeExpression, time, type Expr, type In, type NumTy, type RawComputeFetch } from "./expr.ts";

type Extent = readonly [number, number] | readonly [number, number, number];
type Grid = readonly [number] | Extent;
type Format = keyof typeof TEXEL_BYTES;
type Shape =
  | { readonly kind: "buffer"; readonly entries: number; readonly lanes: number }
  | { readonly kind: "texture"; readonly size: Extent; readonly format: Format };
type When =
  | { readonly kind: "build" }
  | { readonly kind: "load" }
  | { readonly kind: "frame" }
  | { readonly kind: "steps"; readonly start: string; readonly count: number; readonly rate: number; readonly keep?: number };
type Steps = Extract<When, { kind: "steps" }> & { readonly keep: number };

type BufferRead = RawComputeFetch & { readonly wgsl: string };
type TextureRead<T extends "vec2" | "vec3"> = { readonly wgsl: string; sample(uv: In<T>): Expr<"vec4"> };
type Handle<S extends Shape> = S extends { kind: "buffer" } ? BufferRead
  : S extends { size: readonly [number, number, number] } ? TextureRead<"vec3"> : TextureRead<"vec2">;
type Read = BufferRead | TextureRead<"vec2"> | TextureRead<"vec3">;
type Spec<W extends Readonly<Record<string, Shape>>> = {
  readonly when: When;
  readonly writes: W;
  readonly reads?: Readonly<Record<string, Read>>;
  /** Named WGSL getters for inputs and parameters. Frame passes also see frame.time.x, in seconds. */
  readonly values?: Readonly<Record<string, Expr<NumTy>>>;
  readonly entry: string;
  readonly dispatch: Grid;
  readonly wgsl: string;
};

export type ComputeResource = Shape & {
  readonly key: string;
  readonly symbol: string;
  readonly bytes: number;
  readonly steps?: Steps;
};
export type ComputePass = {
  readonly name: string;
  readonly when: Exclude<When, { kind: "steps" }> | Steps;
  readonly reads: readonly (readonly [string, ComputeResource])[];
  readonly writes: readonly (readonly [string, ComputeResource])[];
  readonly values: Spec<never>["values"];
  readonly entry: string;
  readonly dispatch: readonly [number, number, number];
  readonly wgsl: string;
};
const resources = new WeakMap<object, ComputeResource>();
const passes = new WeakMap<object, ComputePass>();
const IDENT = /^[A-Za-z][A-Za-z0-9_]*$/;
// Frugal by default and within baseline WebGPU per-stage and resource limits.
const MAX_BYTES = 64 * 1024 ** 2;
const MAX_STEPS = 4096;
const MAX_PASSES = 64;
const MAX_GRID = 65535;

export function resourceError(where: string, given: string, expected: string, fix: string): never {
  throw new SceneError(`compute "${where}"`, given, expected, fix);
}

function integer(where: string, field: string, value: unknown, min: number, max: number): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < min || value > max)
    resourceError(where, `${field}: ${shown(value)}`, `a whole number in [${min}, ${max}]`, `set ${field} in that range`);
  return value;
}

function identifier(where: string, field: string, value: unknown): asserts value is string {
  if (typeof value !== "string" || !IDENT.test(value) || value.length > 64 || value.includes("__") || value.startsWith("loft_"))
    resourceError(where, `${field}: ${shown(value)}`, "an identifier starting with a letter, without double underscores or a loft_ prefix", "use a name such as cells");
}

function record(where: string, field: string, value: unknown): void {
  if (value === null || typeof value !== "object" || Array.isArray(value))
    resourceError(where, `${field}: ${shown(value)}`, "a record keyed by WGSL identifiers", `write ${field}: { cells: ... }`);
}

function schedule(name: string, when: When): ComputePass["when"] {
  switch (when?.kind) {
    case "build": return { kind: "build" };
    case "load": return { kind: "load" };
    case "frame": return { kind: "frame" };
    case "steps": {
      identifier(name, "when.start", when.start);
      const count = integer(name, "when.count", when.count, 1, MAX_STEPS);
      const keep = integer(name, "when.keep", when.keep ?? 1, 1, count);
      if (typeof when.rate !== "number" || !Number.isFinite(when.rate) || when.rate < 0.001 || when.rate > 100000)
        resourceError(name, `when.rate: ${shown(when.rate)}`, "a rate in [0.001, 100000] steps per second", "set when.rate to a finite rate, such as 60");
      return { kind: "steps", start: when.start, count, rate: when.rate, keep };
    }
    default: return resourceError(name, `when: ${shown((when as { kind?: unknown } | undefined)?.kind)}`, "one of build, load, frame or steps", 'write when: { kind: "frame" }');
  }
}

function shapeOf(where: string, shape: Shape): Shape & { bytes: number } {
  switch (shape?.kind) {
    case "buffer": {
      const entries = integer(where, "entries", shape.entries, 1, MAX_BYTES / 16);
      const lanes = integer(where, "lanes", shape.lanes, 1, MAX_BYTES / (16 * entries));
      return { kind: "buffer", entries, lanes, bytes: 16 * entries * lanes };
    }
    case "texture": {
      const size = shape.size;
      if (!Array.isArray(size) || (size.length !== 2 && size.length !== 3))
        resourceError(where, `size: ${shown(size)}`, "[width, height] or [width, height, depth]", "give two or three positive dimensions");
      const side = size.length === 2 ? 8192 : 256;
      const dims = [...size].map((n, i) => integer(where, `size[${i}]`, n, 1, side)) as unknown as Extent;
      if (typeof shape.format !== "string" || !Object.hasOwn(TEXEL_BYTES, shape.format))
        resourceError(where, `format: ${shown(shape.format)}`, Object.keys(TEXEL_BYTES).join(", "), 'use format: "r32float" for a scalar field or "rgba8unorm" for colour');
      return { kind: "texture", size: dims, format: shape.format, bytes: dims.reduce((a, b) => a * b, TEXEL_BYTES[shape.format]) };
    }
    default: return resourceError(where, `kind: ${shown((shape as { kind?: unknown } | undefined)?.kind)}`, "buffer or texture", 'give a resource such as { kind: "buffer", entries: 32, lanes: 1 }');
  }
}

export function keptStates(s: Steps): number { return 1 + Math.ceil(s.count / s.keep); }

function output(r: ComputeResource): Read {
  if (r.kind === "buffer") {
    const name = `${r.symbol}_fetch`;
    const read = r.steps === undefined ? `${r.symbol}[u32(i) * ${r.lanes}u + u32(lane)]` : `${r.symbol}(u32(i), u32(lane), t)`;
    const fn = { name, ret: "vec4", js: "gpu-only", wgsl: `fn ${name}(i: f32, lane: f32, t: f32) -> vec4<f32> { return ${read}; }` } as const;
    return {
      wgsl: r.symbol,
      [computeFetch]: (i, lane) => {
        if (!Number.isInteger(lane) || lane < 0 || lane >= r.lanes) resourceError(r.key, `lane ${lane}`, `a lane in [0, ${r.lanes - 1}]`, "use a declared lane");
        return rawComputeExpression(fn, r.key, lift<"f32">(i), lane, r.steps === undefined ? 0 : time);
      },
    };
  }
  const name = `${r.symbol}_sample`, dim = r.size.length === 2 ? "vec2" : "vec3";
  const steps = r.steps;
  const size = `${dim}<i32>(${r.size.join(", ")})`;
  // Explicit texture loads give the same filtering to r32float and UNORM without an optional sampler feature.
  const coords = (x: string, y: string, z: string) => dim === "vec2" ? `vec2<i32>(${x}, ${y})` : `vec3<i32>(${x}, ${y}, ${z})`;
  const sample = (x: string, y: string, z: string) => steps === undefined
    ? `textureLoad(${r.symbol}, ${coords(x, y, z)}, 0)`
    : `${r.symbol}(${coords(x, y, z)}, t)`;
  const layer = (z: string) => `mix(mix(${sample("a.x", "a.y", z)}, ${sample("b.x", "a.y", z)}, f.x), mix(${sample("a.x", "b.y", z)}, ${sample("b.x", "b.y", z)}, f.x), f.y)`;
  const fn = { name, ret: "vec4", js: "gpu-only", wgsl: `
fn ${name}(uv: ${dim}<f32>, t: f32) -> vec4<f32> {
  let n = ${size};
  let p = clamp(uv * ${dim}<f32>(n) - ${dim}<f32>(0.5), ${dim}<f32>(0), ${dim}<f32>(n - ${dim}<i32>(1)));
  let a = ${dim}<i32>(floor(p));
  let b = min(a + ${dim}<i32>(1), n - ${dim}<i32>(1));
  let f = fract(p);
  return ${dim === "vec2" ? layer("") : `mix(${layer("a.z")}, ${layer("b.z")}, f.z)`};
}` } as const;
  return { wgsl: r.symbol, sample: (uv: In<typeof dim>) => rawComputeExpression(fn, r.key, lift(uv), steps === undefined ? 0 : time) };
}

/**
 * Declare a raw WGSL compute pass. Add its returned record to the scene's `compute` list. Each key in `writes`
 * becomes a WGSL variable and a handle on that record. Use handles in another pass's `reads`, in `fetch` or
 * `.sample(uv)` expressions, or via `.wgsl` in a raw drawable. Mark the pass build, load, frame, or steps.
 */
export function compute<const W extends Readonly<Record<string, Shape>>>(name: string, spec: Spec<W>): { readonly [K in keyof W]: Handle<W[K]> } {
  identifier("name", "name", name);
  record(name, "options", spec);
  const when = schedule(name, spec.when);
  identifier(name, "entry", spec.entry);
  if (typeof spec.wgsl !== "string" || spec.wgsl.trim() === "")
    resourceError(name, "empty WGSL", "a WGSL module defining the declared entry points", "give wgsl with an @compute function");
  if (!Array.isArray(spec.dispatch) || spec.dispatch.length < 1 || spec.dispatch.length > 3)
    resourceError(name, `dispatch: ${shown(spec.dispatch)}`, "[x], [x, y] or [x, y, z] workgroup counts", "give one to three positive workgroup counts");
  const dims = [...spec.dispatch].map((n, i) => integer(name, `dispatch[${i}]`, n, 1, MAX_GRID));
  const dispatch = [dims[0]!, dims[1] ?? 1, dims[2] ?? 1] as const;
  if (dispatch.reduce((a, b) => a * b, 1) > 2 ** 20)
    resourceError(name, `dispatch: ${shown(dispatch)}`, "at most 1,048,576 workgroups per dispatch", "increase the work done per invocation or reduce the dispatch size");
  if (when.kind === "steps" && dispatch.reduce((a, b) => a * b, when.count + 1) > 2 ** 22)
    resourceError(name, "over 4,194,304 workgroups across the steps", "a bounded step bake", "reduce dispatch or step count");
  record(name, "writes", spec.writes);
  if (spec.reads !== undefined) record(name, "reads", spec.reads);
  if (spec.values !== undefined) record(name, "values", spec.values);
  const aliases = new Set(["frame", "inputs", "params", "curves", "loft_detail", "view_dir", "curve", "CURVE_N", "loft_dt"]);
  const alias = (field: string, key: string) => {
    identifier(name, field, key);
    if (aliases.has(key) || (when.kind === "steps" && key.startsWith("prev_")))
      resourceError(name, `alias "${key}" in ${field}`, "a unique, unreserved alias", "rename the alias in the declaration and WGSL");
    aliases.add(key);
  };
  const handles = Object.create(null) as Record<string, Read>;
  const writes = Object.entries(spec.writes).map(([key, shape]) => {
    alias("writes", key);
    const at = `${name}.${key}`;
    const s = shapeOf(at, shape);
    const r: ComputeResource = { ...s, key: at, symbol: `loft_compute_${name.length}_${name}_${key}`, ...(when.kind === "steps" ? { steps: when } : {}) };
    const bytes = r.bytes * (r.steps === undefined ? 1 : keptStates(r.steps));
    integer(at, "output bytes", bytes, 1, MAX_BYTES);
    if (r.steps !== undefined && r.kind === "texture") {
      const size = computeHistorySize(r.size, keptStates(r.steps)), limit = size.length === 3 ? 256 : 8192;
      if (Math.max(...size) > limit)
        resourceError(at, `${size.join("×")} kept texels`, `a texture history at most ${limit} texels on each axis`, "reduce the texture size, keep fewer steps or pack state into a buffer");
    }
    const h = output(r);
    resources.set(h, r);
    handles[key] = Object.freeze(h);
    return [key, r] as const;
  });
  if (writes.length === 0) resourceError(name, "no outputs", "at least one declared output", "add an output to writes");
  const reads = Object.entries(spec.reads ?? {}).map(([key, h]) => {
    alias("reads", key);
    const r = typeof h === "object" && h !== null ? resources.get(h) : undefined;
    if (r === undefined) resourceError(name, `reads.${key}`, "an output handle returned by compute", "use a handle such as palette.tones");
    return [key, r] as const;
  });
  const values = spec.values === undefined ? undefined : { ...spec.values };
  for (const [key, e] of Object.entries(values ?? {})) {
    alias("values", key);
    if (when.kind !== "frame" || (e?.node?.op !== "input" && e?.node?.op !== "param"))
      resourceError(name, `values.${key}`, "an input or param in a frame pass", "pass input(...) or param(...) directly and do its maths in WGSL, or use a WGSL constant outside a frame pass");
  }
  if (when.kind === "steps" && when.start === spec.entry)
    resourceError(name, "one entry for both start and step", "two entry points", "give when.start and entry different functions");
  passes.set(handles, { name, when, reads, writes, values, entry: spec.entry, dispatch, wgsl: spec.wgsl });
  return Object.freeze(handles) as { readonly [K in keyof W]: Handle<W[K]> };
}

const PHASE = { build: 0, load: 1, steps: 1, frame: 2 } as const;
export function orderCompute(given: readonly object[] | undefined): ComputePass[] {
  if (given === undefined) return [];
  if (!Array.isArray(given) || given.length > MAX_PASSES)
    resourceError("scene", shown(given), `a list of up to ${MAX_PASSES} passes`, "put compute results in scene.compute");
  const names = new Set<string>(), writers = new Map<ComputeResource, ComputePass>();
  const list = given.map((handle) => {
    const p = passes.get(handle);
    if (p === undefined) resourceError("scene", shown(handle), "a record returned by compute", "put the whole return value in scene.compute");
    if (names.has(p.name)) resourceError(p.name, "two passes with this name", "a unique pass name", "rename one pass or remove the duplicate scene.compute entry");
    names.add(p.name);
    for (const [, r] of p.writes) writers.set(r, p);
    return p;
  });
  let bytes = 0;
  for (const r of writers.keys()) bytes += r.bytes * (r.steps === undefined ? (r.kind === "texture" && writers.get(r)!.when.kind === "frame" ? 2 : 1) : (r.kind === "texture" ? 3 : 2) + keptStates(r.steps));
  if (bytes > 256 * 1024 ** 2) resourceError("scene", `${bytes} bytes of compute storage`, "at most 256 MiB", "reduce output sizes, or keep fewer steps");
  const ordered: ComputePass[] = [], active = new Set<ComputePass>(), done = new Set<ComputePass>();
  const visit = (p: ComputePass): void => {
    if (done.has(p)) return;
    if (active.has(p)) resourceError(p.name, "a cycle of compute reads", "passes where producers finish before consumers", "remove the cycle or express the feedback with steps");
    active.add(p);
    for (const [alias, r] of p.reads) {
      const from = writers.get(r);
      if (from === undefined) resourceError(p.name, `reads.${alias} from "${r.key}" outside this scene`, "a producer in scene.compute", "add its compute record to scene.compute");
      if (PHASE[from.when.kind] > PHASE[p.when.kind])
        resourceError(p.name, `${p.when.kind} reads ${from.when.kind} output "${r.key}"`, "a producer that runs before this pass", "move the producer earlier or the consumer later");
      visit(from);
    }
    active.delete(p);
    done.add(p);
    ordered.push(p);
  };
  for (const p of list) visit(p);
  return ordered.sort((a, b) => PHASE[a.when.kind] - PHASE[b.when.kind]);
}
