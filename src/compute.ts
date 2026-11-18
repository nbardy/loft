// Author-side declarations. The GPU-less compiler turns these into data before a host sees them.
import { COMPUTE_TEXEL_BYTES as TEXEL_BYTES, computeHistorySize } from "./host/manifest.ts";
import { SceneError, shown } from "./errors.ts";
import { everything, leaves, lift, rawCall, rawComputeExpression, time, type Expr, type In, type LibFn } from "./expr.ts";
import { WGSL_STATE, type EntryText, type WgslModule } from "./text/index.ts";

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

/** A buffer output, read by calling it: vec4 number `lane` of entry `index` (stepped: the state kept at `time`). */
type BufferRead = (index: In<"f32">, lane: number) => Expr<"vec4">;
/** A texture output, read by calling it at uv in [0, 1] per axis, filtered linearly (stepped: the state kept at `time`). */
type TextureRead<T extends "vec2" | "vec3"> = (uv: In<T>) => Expr<"vec4">;
type Handle<S extends Shape> = S extends { kind: "buffer" } ? BufferRead
  : S extends { size: readonly [number, number, number] } ? TextureRead<"vec3"> : TextureRead<"vec2">;
type Spec<W extends Readonly<Record<string, Shape>>> = {
  readonly when: When;
  readonly writes: W;
  /** The `@compute` entry each dispatch runs. Omit it when the module has one (for steps: one besides `when.start`). */
  readonly entry?: string;
  readonly dispatch: Grid;
  /** The pass's WGSL: a `wgsl` module. Its holes bring in params, inputs and other passes' outputs; it reads `time`. */
  readonly wgsl: WgslModule;
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
  /** Other passes' outputs this pass's text reads (found from its holes), each bound under its symbol. */
  readonly reads: readonly (readonly [string, ComputeResource])[];
  readonly writes: readonly (readonly [string, ComputeResource])[];
  readonly entry: string;
  readonly dispatch: readonly [number, number, number];
  /** The entry points as text, the library functions they call and the values their accessors return. */
  readonly code: EntryText;
};
const passes = new WeakMap<object, ComputePass>();
/** The output a reading library function reads (each output has one: `X_fetch` or `X_sample`). */
const readers = new WeakMap<LibFn, ComputeResource>();
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

/**
 * An output's handle: a function of expressions (TypeScript and graph text call it), which raw WGSL text calls through
 * a hole by the name of the same library function (`rawCall`), with the time it reads appended.
 */
function output(r: ComputeResource): BufferRead | TextureRead<"vec2" | "vec3"> {
  // A stepped output is read at the scene's time; any other has one state, and its readers ignore the time.
  const at: number | Expr<"f32"> = r.steps === undefined ? 0 : time;
  if (r.kind === "buffer") {
    const name = `${r.symbol}_fetch`;
    const read = r.steps === undefined ? `${r.symbol}[u32(i) * ${r.lanes}u + u32(lane)]` : `${r.symbol}(u32(i), u32(lane), t)`;
    const fn: LibFn = { name, ret: "vec4", js: "gpu-only", wgsl: `fn ${name}(i: f32, lane: f32, t: f32) -> vec4<f32> { return ${read}; }` };
    readers.set(fn, r);
    const h: BufferRead = (i, lane) => {
      if (!Number.isInteger(lane) || lane < 0 || lane >= r.lanes) resourceError(r.key, `lane ${lane}`, `a lane in [0, ${r.lanes - 1}]`, "use a declared lane");
      return rawComputeExpression(fn, r.key, lift<"f32">(i), lane, at);
    };
    return Object.assign(h, { [rawCall]: { lib: fn, trailing: [at] } });
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
  const fn: LibFn = { name, ret: "vec4", js: "gpu-only", wgsl: `
fn ${name}(uv: ${dim}<f32>, t: f32) -> vec4<f32> {
  let n = ${size};
  let p = clamp(uv * ${dim}<f32>(n) - ${dim}<f32>(0.5), ${dim}<f32>(0), ${dim}<f32>(n - ${dim}<i32>(1)));
  let a = ${dim}<i32>(floor(p));
  let b = min(a + ${dim}<i32>(1), n - ${dim}<i32>(1));
  let f = fract(p);
  return ${dim === "vec2" ? layer("") : `mix(${layer("a.z")}, ${layer("b.z")}, f.z)`};
}` };
  readers.set(fn, r);
  const h: TextureRead<typeof dim> = (uv) => rawComputeExpression(fn, r.key, lift(uv), at);
  return Object.assign(h, { [rawCall]: { lib: fn, trailing: [at] } });
}

/** Every library function these call, their dependencies included. */
function closure(libs: readonly LibFn[]): LibFn[] {
  const out: LibFn[] = [];
  const add = (l: LibFn): void => { if (out.includes(l)) return; out.push(l); for (const d of l.deps ?? []) add(d); };
  libs.forEach(add);
  return out;
}

/** The library functions an entry text and its accessors call. */
export function passLibs(code: EntryText): LibFn[] {
  const fromValues = everything(code.values).flatMap((e) => (e.node.op === "lib" ? [e.node.fn] : []));
  return closure([...code.libs, ...fromValues]);
}

/**
 * Declare a raw WGSL compute pass, written as a `wgsl` module with an `@compute` entry. Add its returned record to the
 * scene's `compute` list. Each key in `writes` is a WGSL variable in the pass (for steps, `prev_<key>` too) and a
 * handle on the record: call it to read the output (`baked.sdf(uv)`, `sim.state(i, lane)`) in TypeScript or in text,
 * and `${baked.sdf}(uv)` in a hole of any `wgsl` text, raw or not. A hole is the dependency: a pass that reads another's
 * output runs after it. Mark the pass build, load, frame, or steps; a frame pass also reads params, inputs and `time`.
 */
export function compute<const W extends Readonly<Record<string, Shape>>>(name: string, spec: Spec<W>): { readonly [K in keyof W]: Handle<W[K]> } {
  identifier("name", "name", name);
  record(name, "options", spec);
  for (const gone of ["reads", "values"] as const) {
    if (gone in spec) resourceError(name, gone, "no reads or values (a hole brings them in)", gone === "reads" ? "call another pass's output through a hole in the text: ${palette.tones}(0.0, 0.0)" : "read a param or input through a hole in the text: ${speed}");
  }
  const when = schedule(name, spec.when);
  const state = typeof spec.wgsl === "object" || typeof spec.wgsl === "function" ? (spec.wgsl as WgslModule | null)?.[WGSL_STATE] : undefined;
  if (state === undefined)
    resourceError(name, `wgsl: ${typeof spec.wgsl === "string" ? "a string" : shown(spec.wgsl)}`, "a wgsl module", "write wgsl: wgsl`@compute @workgroup_size(64) fn main(...) { ... }` (import wgsl from loft)");
  const kernels = state.module.fns.flatMap((f) => (f.kind === "entry" && f.stage === "compute" ? [f.sig.name] : []));
  if (when.kind === "steps" && !kernels.includes(when.start))
    resourceError(name, `when.start: "${when.start}"`, `one of the module's @compute entries (${kernels.join(", ") || "none"})`, "name the entry that seeds the state");
  const candidates = kernels.filter((k) => when.kind !== "steps" || k !== when.start);
  const entry = spec.entry ?? (candidates.length === 1 ? candidates[0]! : resourceError(name, `${candidates.length} @compute entries${when.kind === "steps" ? " besides when.start" : ""} (${candidates.join(", ") || "none"})`, "the entry to run", candidates.length === 0 ? "add an @compute fn" : `name it: entry: "${candidates[0]}"`));
  identifier(name, "entry", entry);
  if (!candidates.includes(entry))
    resourceError(name, `entry: "${entry}"`, `one of ${candidates.join(", ") || "no @compute entries"}`, when.kind === "steps" && entry === when.start ? "give when.start and entry different functions" : "name an @compute fn of the module");
  if (!Array.isArray(spec.dispatch) || spec.dispatch.length < 1 || spec.dispatch.length > 3)
    resourceError(name, `dispatch: ${shown(spec.dispatch)}`, "[x], [x, y] or [x, y, z] workgroup counts", "give one to three positive workgroup counts");
  const dims = [...spec.dispatch].map((n, i) => integer(name, `dispatch[${i}]`, n, 1, MAX_GRID));
  const dispatch = [dims[0]!, dims[1] ?? 1, dims[2] ?? 1] as const;
  if (dispatch.reduce((a, b) => a * b, 1) > 2 ** 20)
    resourceError(name, `dispatch: ${shown(dispatch)}`, "at most 1,048,576 workgroups per dispatch", "increase the work done per invocation or reduce the dispatch size");
  if (when.kind === "steps" && dispatch.reduce((a, b) => a * b, when.count + 1) > 2 ** 22)
    resourceError(name, "over 4,194,304 workgroups across the steps", "a bounded step bake", "reduce dispatch or step count");
  record(name, "writes", spec.writes);
  const aliases = new Set(["frame", "inputs", "params", "curves", "loft_detail", "view_dir", "curve", "CURVE_N"]);
  const handles = Object.create(null) as Record<string, BufferRead | TextureRead<"vec2" | "vec3">>;
  const writes = Object.entries(spec.writes).map(([key, shape]) => {
    identifier(name, "writes", key);
    if (aliases.has(key) || (when.kind === "steps" && key.startsWith("prev_")))
      resourceError(name, `output "${key}"`, "a unique, unreserved name", "rename the output in writes and in the WGSL");
    aliases.add(key);
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
    handles[key] = Object.freeze(output(r));
    return [key, r] as const;
  });
  if (writes.length === 0) resourceError(name, "no outputs", "at least one declared output", "add an output to writes");
  const code = state.entries(when.kind === "steps" ? [when.start, entry] : [entry]);
  // What the text reads from outside: only a frame pass has the frame's inputs and params bound; nothing binds curves
  // or tables in a pass.
  const read = leaves(code.values);
  if (read.rows.length > 0 || read.tables.length > 0)
    resourceError(name, `a hole reading ${read.rows.length > 0 ? `the curve "${read.rows[0]!.label}"` : `the table "${read.tables[0]!.name}"`}`, "params, inputs, time and other passes' outputs", "bake the curve or table into a compute output, or read it in a drawable");
  if (when.kind !== "frame" && (read.inputs.length > 0 || read.params.length > 0))
    resourceError(name, `a ${when.kind} pass reading ${read.inputs.length > 0 ? (read.inputs[0]!.name === "time" ? "time" : `the input "${read.inputs[0]!.name}"`) : `the param "${read.params[0]!.name}"`}`, "time, inputs and params in frame passes only", when.kind === "steps" ? "a step's clock is its count: keep it in the state" : "use a constant here, or make it a frame pass");
  const reads = passLibs(code).flatMap((l) => { const r = readers.get(l); return r === undefined ? [] : [[r.symbol, r] as const]; });
  passes.set(handles, { name, when, reads, writes, entry, dispatch, code });
  return Object.freeze(handles) as unknown as { readonly [K in keyof W]: Handle<W[K]> };
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
    for (const [, r] of p.reads) {
      const alias = r.key;
      const from = writers.get(r);
      if (from === undefined) resourceError(p.name, `a read of "${alias}" outside this scene`, "a producer in scene.compute", "add its compute record to scene.compute");
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
