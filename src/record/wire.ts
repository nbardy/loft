// The wire (docs/proposals/2026-11-19_lean_rust_loft.md section 3.2): one job is a header and eleven spans, written by
// src/record/recorder.ts and decoded by `loft-ir`. These tables are the one place the numbers live on the TS side;
// loft-ir's `Op`, `Ty` and decl tags must equal them, which the shadow print (src/record/print.ts) checks end to end.
//
// node   [op | ty << 8 | flags << 16,  args offset,  n args,  imm]
//   Lit     imm = offset of its first component in `lits`, n = component count (args offset 0)
//   Time    -
//   Param   imm = Param decl ordinal                Input  imm = Input decl ordinal
//   Bind    imm = sym of its name; flags BOUND for a loop's or a graph function's binder (named by its owner: L3s0,
//           L3i, F2a1), FREE for a variable its consumer binds (u, v, p)
//   Call    imm = Fn decl ordinal (a graph function), args = the call's arguments
//   Raw     imm = Fn decl ordinal (a raw WGSL function), args = its arguments
//   Fetch   imm = Kernel decl ordinal (the resource), flags = lane, args = [index]
//   Loop    imm = Loop decl ordinal, flags = which component of the final state
//   Grad    reserved: the recorder of wave 1 writes derivatives already expanded (src/grad.ts)
//   prims   args = operands; Swizzle's imm = selection (2 bits per component, length << 8)
//
// decls  [program root count, then per decl: tag, n roots, n words, words...]; `roots` holds the program's roots, then
//        each decl's in decl order.
//   Fn      graph [0, name, ret ty, n params, binder node...]  roots [body]
//           raw   [1, name, ret ty, text sym, n deps, dep Fn ordinal...]
//   Loop    [n state, has stop, binder node per state component, index binder]  roots [count, init..., next..., stop?]
//   Kernel  table [0, name, lanes, entries, role]  rows [1, label, offset in `rows`, length]
//   Param   [name, def lit, min lit, max lit]        Input  [name, ty, first def lit]

import type { Fn, TableSource, Ty } from "../expr.ts";

export const MAGIC = 0x5754464c; // "LFTW", little-endian
export const WIRE = 1;

export enum Program { Scene, Solve, Evaluate, Check }
export enum Decl { Fn, Loop, Kernel, Draw, Pass, Medium, Check, Solve, Param, Input, Camera }
export enum Op { Lit, Time, Param, Input, Bind, Call, Fetch, Loop, Grad, Raw, Swizzle, Vec } // then PRIMS from PRIM0
export const BOUND = 1, FREE = 0;
export const TYS = ["f32", "vec2", "vec3", "vec4", "bool"] as const satisfies readonly Ty[];
export const ROLES = ["stations", "data", "static"] as const satisfies readonly TableSource["role"][];

/** The operators of src/expr.ts `Fn`, in code order: op = 16 + index. */
export const PRIMS = [
  "add", "sub", "mul", "div", "neg", "min", "max", "pow", "mod", "atan2", "step",
  "abs", "sign", "floor", "fract", "sqrt", "exp", "log", "sin", "cos", "tan", "saturate", "normalize",
  "mix", "clamp", "smoothstep", "length", "distance", "dot", "cross",
  "lt", "gt", "le", "ge", "eq", "ne", "and", "or", "not", "select",
  "ceil", "trunc", "roundEven", "rem", "inverseSqrt", "exp2", "log2", "asin", "acos", "atan", "sinh", "cosh", "tanh",
] as const satisfies readonly Fn[];
export const PRIM0 = 16;
// Each table holds every member of its union, so no op, type or role can encode as a missing (zero) code.
type Every<U, T extends readonly unknown[]> = [Exclude<U, T[number]>] extends [never] ? true : never;
const _every: [Every<Fn, typeof PRIMS>, Every<Ty, typeof TYS>, Every<TableSource["role"], typeof ROLES>] = [true, true, true];
void _every;

export const swizzleCode = (sel: string): number => [...sel].reduce((c, ch, k) => c | ("xyzw".indexOf(ch) << (2 * k)), sel.length << 8);
export const swizzleSel = (code: number): string => Array.from({ length: code >> 8 }, (_, k) => "xyzw"[(code >> (2 * k)) & 3]).join("");

/** One job, span by span (N-API borrows each typed array). */
export type Job = {
  readonly program: Program;
  readonly nodes: Uint32Array; readonly args: Uint32Array; readonly lits: Float64Array; readonly rows: Float32Array;
  readonly blobs: Uint8Array; readonly syms: Uint8Array; readonly symAt: Uint32Array;
  readonly sites: Uint32Array; readonly siteOf: Uint32Array; readonly decls: Uint32Array; readonly roots: Uint32Array;
};
const SPANS = ["nodes", "args", "lits", "rows", "blobs", "syms", "symAt", "sites", "siteOf", "decls", "roots"] as const;

/** The job as one byte string: the header, then each span as its byte length and its bytes padded to 8. What is hashed and dumped. */
export function pack(job: Job): Uint8Array {
  const parts = SPANS.map((s) => new Uint8Array(job[s].buffer, job[s].byteOffset, job[s].byteLength));
  const out = new Uint8Array(16 + parts.reduce((n, p) => n + 8 + ((p.length + 7) & ~7), 0));
  const w = new DataView(out.buffer);
  [MAGIC, WIRE, job.program, SPANS.length].forEach((v, k) => w.setUint32(4 * k, v, true));
  let at = 16;
  for (const p of parts) { w.setUint32(at, p.length, true); out.set(p, at + 8); at += 8 + ((p.length + 7) & ~7); }
  return out;
}

/** Symbol `k` of a job. */
export const symOf = (job: Job, k: number): string => new TextDecoder().decode(job.syms.subarray(job.symAt[k], job.symAt[k + 1]));

/** The decls of a job, parsed: each with its words and roots. */
export type DeclRec = { readonly tag: Decl; readonly words: Uint32Array; readonly roots: Uint32Array };
export function declsOf(job: Job): { programRoots: Uint32Array; decls: DeclRec[] } {
  const d = job.decls, decls: DeclRec[] = [];
  let at = 1, root = d[0]!;
  while (at < d.length) {
    const tag = d[at]! as Decl, nr = d[at + 1]!, nw = d[at + 2]!;
    decls.push({ tag, words: d.subarray(at + 3, at + 3 + nw), roots: job.roots.subarray(root, root + nr) });
    at += 3 + nw; root += nr;
  }
  return { programRoots: job.roots.subarray(0, d[0]!), decls };
}
