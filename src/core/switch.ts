// Which compiler core runs a kernel: the TypeScript one (the default) or the Rust one (`LOFT_CORE=rust`).
//
// No bun-only module here: share.ts, which the browser host imports, asks this file. The Rust kernels are registered
// by src/core/native.ts, which the CLI imports when `LOFT_CORE=rust` (src/cli/main.ts). A kernel the Rust core cannot
// take is a named refusal: printed once per reason on stderr, counted, and then run by the TypeScript twin. There is
// no other way back to TypeScript.

/** The kernels the Rust core can run, each with a TypeScript twin of the same bytes. */
export type Kernel = "share" | "solve";

/** Why a kernel stayed on TypeScript under `LOFT_CORE=rust`. */
export type Refusal = { readonly kernel: Kernel; readonly reason: string };

type Env = { readonly process?: { readonly env?: Record<string, string | undefined> } };
const asked = (globalThis as Env).process?.env?.LOFT_CORE;

/** The core asked for: `rust` only when `LOFT_CORE=rust`; any other value but unset or `ts` is refused at start. */
export const CORE: "ts" | "rust" = asked === undefined || asked === "" || asked === "ts" ? "ts" : asked === "rust" ? "rust" : (() => {
  throw new Error(`LOFT_CORE="${asked}": expected "ts" (the default) or "rust"`);
})();

/** The Rust kernels, set by native.ts. */
export const rustKernels: { share?: unknown; solve?: unknown } = {};

const reported = new Set<string>();
/** Every refusal of this process, in order (tools and tests read it; the line on stderr is for people). */
export const refusals: Refusal[] = [];

/** Records that `kernel` runs on TypeScript this time, and why. */
export function refuse(kernel: Kernel, reason: string): void {
  refusals.push({ kernel, reason });
  const line = `loft core: ${kernel} stays on TypeScript: ${reason}`;
  if (reported.has(line)) return;
  reported.add(line);
  console.error(line);
}

/** The Rust implementation of `kernel` when `LOFT_CORE=rust`; undefined (with a reported refusal) when it is not loaded. */
export function rustKernel<K extends Kernel>(kernel: K): unknown {
  if (CORE !== "rust") return undefined;
  const k = rustKernels[kernel];
  if (k === undefined) refuse(kernel, "the Rust core is not loaded in this entry point (src/core/native.ts)");
  return k;
}
