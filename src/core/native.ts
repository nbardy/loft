// The one native call (plan 2026-11-19 §3.5): a recorded job crosses into Rust as one Uint8Array, borrowed in place,
// and an answer comes back as JSON. Every failure is a NativeError carrying the engine's typed Error and its exit code
// (src/cli/verdict.ts maps it in one line). A missing, unloadable or stale addon is `environment` (exit 6) and names
// the build command that fixes it. Runs under Bun and Node 22 alike (process.dlopen, no Bun-only API).

import { fileURLToPath } from "node:url";

/** The addon this checkout expects (rust/crates/loft-node STAMP): bump both with any change to run's arguments or answer. */
const STAMP = "loft-node abi 2 wire 1";
const BUILD = "tools/dev.sh build loft-node --fast";

export type Program = "Scene" | "Solve" | "Evaluate" | "Check";
/** Wave 1's answer: the job decoded and validated. */
export type Report = { readonly program: Program; readonly nodes: number; readonly decls: number; readonly decode_ns: number };
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

export type Native = { run(job: Uint8Array, limits: Limits): Report };
type Addon = { run(job: Uint8Array, limits: BigUint64Array): string; stamp: unknown };
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
    run(job, l) {
      const a = JSON.parse(run(job, BigUint64Array.of(l.nodes, l.tapeOps, l.boxes, l.gpuNanos))) as Answer;
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
