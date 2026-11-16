// A recurrence's code (src/map/recur.ts): the step and start as WGSL for the bake and for the frame's replay, and as
// JavaScript for the CPU twin. The step function's text is emitted once and placed unchanged in every module that runs
// it, with only the reads of the previous state (`recur_<name>_prev`) and the writes (`recur_<name>_put`) defined per
// module: so a replay repeats the bake's own arithmetic, which is what makes it bitwise the bake on the GPU that baked.

import { POSE_WORKGROUP } from "../artefact.ts";
import { compileJs, emitWgsl, type Bind, type Env, type LibFn, type TableSource } from "../expr.ts";
import type { RecurState } from "../map/recur.ts";

/** The step and start functions' WGSL, shared text, and the library functions they call. */
export type StepCode = { readonly fns: string; readonly libs: readonly LibFn[] };

const varName = (e: { node: unknown }): string => (e.node as { name: string }).name;
const fn = (s: RecurState, what: string): string => `recur_${s.name}_${what}`;

/** The entry's index from its invocation `e` (an f32 whole number, or a cell of a grid laid out row by row). */
function indexWgsl(s: RecurState): string {
  switch (s.shape.kind) {
    case "list": return "let i: f32 = f32(e);";
    case "grid": return `let i: vec2<f32> = vec2<f32>(f32(e % ${s.shape.w}u), f32(e / ${s.shape.w}u));`;
  }
}

/**
 * `recur_<name>_start(e)` and `recur_<name>_step(e, n)`: entry e of state 0, and of the state after step n (n the step
 * number of the state read, an f32 whole number). `bind` is the scene's binding of leaves (static tables by accessor);
 * the previous state is read through `recur_<name>_prev(j, lane)` and every lane written through
 * `recur_<name>_put(e, a, lane, v)`, a the step number of the state written.
 */
export function stepCode(s: RecurState, bind: (vars: Readonly<Record<string, string>>) => Bind): StepCode {
  const base = bind({ [varName(s.index)]: "i", [varName(s.clock)]: "t" });
  const own: Bind = {
    ...base,
    // The previous state: entry j's lane, j already wrapped into range by `prev`.
    table: (t, i, lane) => (t === s.prev ? `${fn(s, "prev")}(u32(${i}), ${lane}u)` : base.table(t, i, lane)),
  };
  const start = emitWgsl(s.startLanes, own, "a");
  const step = emitWgsl(s.stepLanes, own, "e");
  const puts = (out: readonly string[], a: string) => out.map((v, l) => `  ${fn(s, "put")}(e, ${a}, ${l}u, ${v});`);
  const fns = [
    `fn ${fn(s, "start")}(e: u32) {`, `  ${indexWgsl(s)}`, ...start.lines.map((x) => `  ${x}`), ...puts(start.out, "0u"), "}",
    `fn ${fn(s, "step")}(e: u32, n: f32) {`, `  ${indexWgsl(s)}`,
    // The step's own time: `time` inside the step (src/map/recur.ts replaced it by this variable).
    `  let t: f32 = ${lit(s.from)} + n / ${lit(s.rate)};`,
    ...step.lines.map((x) => `  ${x}`), ...puts(step.out, "u32(n) + 1u"), "}",
  ].join("\n");
  const libs: LibFn[] = [];
  for (const l of [...start.libs, ...step.libs]) if (!libs.includes(l)) libs.push(l);
  return { fns, libs };
}

const lit = (x: number): string => (Number.isInteger(x) ? x.toFixed(1) : String(Math.fround(x)));

/** The clock entry's write: invocation 0 puts the step number of the state written into entry `entries`, lane 0. */
const clockPut = (s: RecurState, a: string): string => `if (e == 0u) { ${fn(s, "put")}(${s.entries}u, ${a}, 0u, vec4<f32>(f32(${a}), 0.0, 0.0, 0.0)); }`;
/** The step number of the state read: its own clock entry. */
const clockRead = (s: RecurState, src: (vec4: string) => string): string => `let n: f32 = ${src(`${s.entries * s.lanes}u`)}.x;`;

/** The static tables a step reads (by placement: buffer name, offset), for the bake's own bindings. */
export type StaticRead = { readonly table: TableSource; readonly buffer: string; readonly offset: number };

/**
 * The bake's module (src/artefact.ts `Recurrence.bake`): its own bindings, so it is legal whatever the scene binds;
 * `statics` are the static buffers the step reads, at bindings 3, 4, ...
 */
export function bakeWgsl(s: RecurState, code: StepCode, reads: readonly StaticRead[]): { wgsl: string; statics: string[] } {
  const statics = [...new Set(reads.map((r) => r.buffer))];
  const L = s.lanes, S = (s.entries + 1) * L, K = s.kept.kind === "every" ? 1 : s.kept.every;
  const wgsl = [
    "@group(0) @binding(0) var<storage, read_write> recur_src: array<vec4<f32>>;",
    "@group(0) @binding(1) var<storage, read_write> recur_dst: array<vec4<f32>>;",
    "@group(0) @binding(2) var<storage, read_write> recur_kept: array<vec4<f32>>;",
    ...statics.map((b, k) => `@group(0) @binding(${3 + k}) var<storage, read> ${b}: array<vec4<f32>>;`),
    ...reads.map((r) => `fn t_${r.table.name}(i: u32) -> vec4<f32> { return ${r.buffer}[${r.offset}u + i]; }`),
    ...code.libs.map((l) => l.wgsl),
    `fn ${fn(s, "prev")}(j: u32, lane: u32) -> vec4<f32> { return recur_src[j * ${L}u + lane]; }`,
    `fn ${fn(s, "put")}(e: u32, a: u32, lane: u32, v: vec4<f32>) {`,
    `  recur_dst[e * ${L}u + lane] = v;`,
    // The states to keep are written here, by the step that makes them: no copy between dispatches.
    `  if (a >= ${s.first}u && (a - ${s.first}u) % ${K}u == 0u) { recur_kept[((a - ${s.first}u) / ${K}u) * ${S}u + e * ${L}u + lane] = v; }`,
    "}",
    code.fns,
    `@compute @workgroup_size(${POSE_WORKGROUP}) fn recur_start(@builtin(global_invocation_id) g: vec3<u32>) {`,
    "  let e = g.x;",
    `  if (e >= ${s.entries}u) { return; }`,
    `  ${fn(s, "start")}(e);`,
    `  ${clockPut(s, "0u")}`,
    "}",
    `@compute @workgroup_size(${POSE_WORKGROUP}) fn recur_step(@builtin(global_invocation_id) g: vec3<u32>) {`,
    "  let e = g.x;",
    `  if (e >= ${s.entries}u) { return; }`,
    // Each dispatch needs its own step number, and it cannot come from a writeBuffer between dispatches: writes are
    // ordered with submits, not with dispatches, so every dispatch of the bake would see the last write. So the state
    // carries it, in a hidden clock entry that invocation 0 advances into the next buffer of the ping-pong pair (and
    // that the kept states keep, so a replay from a checkpoint knows its step too).
    `  ${clockRead(s, (i) => `recur_src[${i}]`)}`,
    `  ${fn(s, "step")}(e, n);`,
    `  ${clockPut(s, "u32(n) + 1u")}`,
    "}",
  ].join("\n");
  return { wgsl, statics };
}

/** Where the replay's tables live in the dynamic buffers, and the kept table in the static ones (vec4 offsets). */
export type ReplayPlace = {
  readonly even: { readonly buffer: string; readonly offset: number };
  readonly odd: { readonly buffer: string; readonly offset: number };
  readonly kept: { readonly buffer: string; readonly offset: number };
  /** The WGSL of the host's step number (`inputs[slot]`). */
  readonly step: string;
};

/**
 * The frame's replay of a checkpointed recurrence: three pose-stage modules over the pose pass's own bindings (`group1`,
 * the scene's group 1 with dynamic buffers writable). `first` reads checkpoint floor(n / K) from the kept table, copies
 * it to the even table and writes its next state to the odd one; `ping` steps odd into even, `pong` even into odd. The
 * host issues first, then k more alternating ping, pong (src/artefact.ts `replayStep`): states n and n + 1 end in the
 * even and odd tables, which holds n by the parity of k.
 */
export function replayWgsl(s: RecurState, code: StepCode, group1: string, at: ReplayPlace): { first: string; ping: string; pong: string } {
  const L = s.lanes, S = (s.entries + 1) * L, K = (s.kept as { every: number }).every;
  const head = [group1, ...code.libs.map((l) => l.wgsl)];
  const cell = (p: { buffer: string; offset: number }, i: string) => `${p.buffer}[${p.offset}u + ${i}]`;
  const checkpoint = `(u32(${at.step}) / ${K}u) * ${S}u`;
  const module = (src: (i: string) => string, dst: { buffer: string; offset: number }, copy: string[]): string => [
    ...head,
    `fn ${fn(s, "prev")}(j: u32, lane: u32) -> vec4<f32> { return ${src(`j * ${L}u + lane`)}; }`,
    `fn ${fn(s, "put")}(e: u32, a: u32, lane: u32, v: vec4<f32>) { ${cell(dst, `e * ${L}u + lane`)} = v; }`,
    code.fns,
    `@compute @workgroup_size(${POSE_WORKGROUP}) fn pose_main(@builtin(global_invocation_id) g: vec3<u32>) {`,
    "  let e = g.x;",
    `  if (e >= ${s.entries}u) { return; }`,
    `  ${clockRead(s, src)}`,
    ...copy.map((c) => `  ${c}`),
    `  ${fn(s, "step")}(e, n);`,
    `  ${clockPut(s, "u32(n) + 1u")}`,
    "}",
  ].join("\n");
  const fromKept = (i: string) => `${at.kept.buffer}[${at.kept.offset}u + ${checkpoint} + ${i}]`;
  const copy = [
    `for (var l = 0u; l < ${L}u; l++) { ${cell(at.even, `e * ${L}u + l`)} = ${fromKept(`e * ${L}u + l`)}; }`,
    `if (e == 0u) { ${cell(at.even, `${s.entries * L}u`)} = ${fromKept(`${s.entries * L}u`)}; }`,
  ];
  return {
    first: module(fromKept, at.odd, copy),
    ping: module((i) => cell(at.odd, i), at.even, []),
    pong: module((i) => cell(at.even, i), at.odd, []),
  };
}

// ---- the CPU twin ------------------------------------------------------------------------------------------------

/**
 * One step on the CPU (JavaScript, f64): `next` gets the state after `prev` (both `(entries + 1) * lanes * 4` floats,
 * the clock included). `tables` reads the static tables (and other recurrences) the step reads. A step that calls a
 * GPU-only library function has no twin (it throws when compiled).
 */
export type CpuStep = (prev: Float32Array, next: Float32Array, tables: (t: TableSource) => Float32Array) => void;

export function cpuStep(s: RecurState): CpuStep {
  const run = compileJs(s.stepLanes);
  const iName = varName(s.index), tName = varName(s.clock);
  const L4 = s.lanes * 4, clock = s.entries * L4;
  return (prev, next, tables) => {
    const n = prev[clock]!;
    const vars: Record<string, number | number[]> = { [tName]: s.from + n / s.rate };
    const env: Env = { vars, inputs: {}, params: {}, tables: (t) => (t === s.prev ? prev : tables(t)) };
    for (let e = 0; e < s.entries; e++) {
      vars[iName] = s.shape.kind === "list" ? e : [e % s.shape.w, Math.floor(e / s.shape.w)];
      const v = run(env);
      for (let l = 0; l < s.lanes; l++) next.set(v[l] as number[], e * L4 + l * 4);
    }
    next.fill(0, clock, clock + L4);
    next[clock] = n + 1;
  };
}

/** State 0 on the CPU (the start, a closed form): what the rest frame reads of an unbaked recurrence. */
export function cpuStart(s: RecurState, tables: (t: TableSource) => Float32Array): Float32Array {
  const run = compileJs(s.startLanes);
  const iName = varName(s.index);
  const L4 = s.lanes * 4;
  const out = new Float32Array((s.entries + 1) * L4);
  const vars: Record<string, number | number[]> = {};
  const env: Env = { vars, inputs: {}, params: {}, tables };
  for (let e = 0; e < s.entries; e++) {
    vars[iName] = s.shape.kind === "list" ? e : [e % s.shape.w, Math.floor(e / s.shape.w)];
    const v = run(env);
    for (let l = 0; l < s.lanes; l++) out.set(v[l] as number[], e * L4 + l * 4);
  }
  return out;
}
