// The one native call (plan 2026-11-19 §3.5): a recorded job crosses into Rust as one Uint8Array, borrowed in place,
// and an answer comes back as JSON. Every failure is a NativeError carrying the engine's typed Error and its exit code
// (src/cli/verdict.ts maps it in one line). A missing, unloadable or stale addon is `environment` (exit 6) and names
// the build command that fixes it. Runs under Bun and Node 22 alike (process.dlopen, no Bun-only API).

import { fileURLToPath } from "node:url";
import type { Precision } from "../host/manifest.ts";

/** The addon this checkout expects (rust/crates/loft-node STAMP): bump both with any change to run's arguments or answer. */
const STAMP = "loft-node abi 5 wire 2";
const BUILD = "tools/dev.sh build loft-node --fast";

export type Program = "Scene" | "Solve" | "Evaluate" | "Check";
/** What a Scene job built (loft-core `Scene`). `unfilled` counts tables nothing fills (a recurrence's: the wire does not
 * carry recurrences yet); `draws` is 0 until the library records its drawables as Draw decls (lane 3.1's TS half). */
export type SceneReport = {
  readonly roots: number; readonly functions: number; readonly wgsl_bytes: number; readonly resources: number; readonly unfilled: number; readonly kernels: number;
  readonly storage_buffers: number; readonly dispatches: number; readonly draws: number; readonly bundle_bytes: number; readonly build: string;
  /** Under `precision: "mixed"`: "Valid", or Naga's refusal of the module under `alias lf = f16;` (a mixed load is then
   * refused by the host). Under "f32": "NotAsked". */
  readonly mixed: "Valid" | "NotAsked" | { readonly Refused: string };
};
/** Rust's own time per phase, ns (loft-core `Phases`). `solve` and `prove` are a Solve or Check job's. */
export type Phases = { readonly decode: number; readonly plan: number; readonly emit: number; readonly f16: number; readonly bundle: number; readonly solve: number; readonly prove: number };
/** loft-core `check::Verdict` as serde writes it. */
export type Verdict = {
  readonly truth: "Holds" | "Fails" | { readonly Undecided: unknown }; readonly margin: number; readonly bounds: readonly [number, number];
  readonly witness: readonly (readonly [number, number])[]; readonly work: { readonly boxes: number }; readonly scope: "Proved" | { readonly Sampled: { readonly n: number } };
};
export type Site = { readonly file: string; readonly line: number; readonly col: number };
/** A Solve job leaves every hold "Unproved" (never a pass); a Check job proves each (loft-core `solve::Proof`). */
export type Proof = "Unproved" | { readonly Checked: Verdict };
export type SolveReport = {
  readonly label: string; readonly curves: readonly (readonly [string, readonly number[]])[]; readonly holds: readonly (readonly [Site, Proof])[];
  readonly rounds: number; readonly steps: number; readonly tape_ops: number;
};
/** The program driver's answer (loft-core `Report`). */
export type Report = {
  readonly program: Program; readonly nodes: number; readonly decls: number; readonly scene: SceneReport | null;
  readonly checks: readonly { readonly label: string; readonly verdict: Verdict }[]; readonly solves: readonly SolveReport[]; readonly phases: Phases;
};
type Kind = "usage" | "load" | "scene" | "budget" | "environment" | "internal";

/** A typed refusal from the engine, or from loading it. `error` is the Rust `Error` as serde JSON (null for load faults). */
export class NativeError extends Error {
  override readonly name = "NativeError";
  readonly kind: Kind;
  readonly code: number;
  readonly error: unknown;
  constructor(kind: Kind, code: number, message: string, error: unknown) {
    super(message);
    this.kind = kind;
    this.code = code;
    this.error = error;
  }
}

/** One job's ceilings in work units (loft-ir `Limits`): over one, the job ends as `budget` (exit 7) at the same count on
 * every machine. `UNLIMITED` leaves a meter to the engine's own quotas. */
export type Limits = { readonly nodes: bigint; readonly tapeOps: bigint; readonly boxes: bigint; readonly gpuNanos: bigint };
export const UNLIMITED = 2n ** 64n - 1n;

/** How to build (loft-core `Build`). `naga`: "load" leaves validation to the host's module creation (the default build;
 * a bad module is refused at load, without the author's line); "emit" runs Naga at emit and refuses at the author's
 * line (`loft check`, `loft build --naga`, the lab tools). `precision: "mixed"` adds the f16 check (`SceneReport.mixed`). */
export type Build = { readonly naga: "load" | "emit"; readonly precision: Precision };
export const DEFAULT_BUILD: Build = { naga: "load", precision: "f32" };

/** `out`: where a Scene's bundle file is written; omitted, it is built and hashed but not written. */
type Native = { run(job: Uint8Array, limits: Limits, out?: string, build?: Build): Report };
type Addon = { run(job: Uint8Array, limits: BigUint64Array, out: Uint8Array, build: Uint8Array): string; stamp: unknown };
type Answer = { Ok: Report } | { Err: { kind: Kind; code: number; message: string; error: unknown } };

const environment = (message: string): NativeError => new NativeError("environment", 6, message, null);

/** Loads the addon at `path` and checks its stamp. */
export function open(path: string): Native {
  const m = { exports: {} as Partial<Addon> };
  try {
    process.dlopen(m, path);
  } catch (e) {
    throw environment(`the Loft engine addon did not load from ${path}: ${(e as Error).message}. Fix: ${BUILD}`);
  }
  const addon = m.exports;
  if (addon.stamp !== STAMP || typeof addon.run !== "function") {
    throw environment(`the Loft engine addon at ${path} is stale: it says ${JSON.stringify(addon.stamp)}, this checkout expects "${STAMP}". Fix: ${BUILD}`);
  }
  const run = addon.run;
  return {
    run(job, l, out, build = DEFAULT_BUILD) {
      // loft-node `build_of`: [naga, precision], naga 0 load / 1 emit, precision 0 f32 / 1 mixed.
      const bytes = Uint8Array.of(build.naga === "emit" ? 1 : 0, build.precision === "mixed" ? 1 : 0);
      const a = JSON.parse(run(job, BigUint64Array.of(l.nodes, l.tapeOps, l.boxes, l.gpuNanos), new TextEncoder().encode(out ?? ""), bytes)) as Answer;
      if ("Ok" in a) return a.Ok;
      throw new NativeError(a.Err.kind, a.Err.code, a.Err.message, a.Err.error);
    },
  };
}

let loaded: Native | undefined;
/** The addon of this checkout's lab build, or the file `LOFT_NATIVE` names (another profile, another platform's build). */
export function native(): Native {
  const lib = process.platform === "darwin" ? "libloft_node.dylib" : "libloft_node.so";
  // fileURLToPath, not URL.pathname: a checkout path with a space or non-ASCII byte is percent-encoded in .pathname.
  return (loaded ??= open(process.env.LOFT_NATIVE ?? fileURLToPath(new URL(`../../rust/target/lab/${lib}`, import.meta.url))));
}
