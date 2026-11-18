// The Rust core's library through bun:ffi: one dynamic library (rust/loft-core, `cargo build --release`), two calls per job
// (`loft_core_call` runs a kernel on packed bytes and keeps the result, `loft_core_take` copies it out).
//
// bun:ffi, not wasm: a job crosses the boundary twice with one buffer each way, so the call cost is nothing either way;
// the native library starts in well under a millisecond (no module compile), runs the f64 libm the JS twin runs (wasm
// would bring its own libm and differ in the last bit of sin, exp and pow), and needs no copy into a linear memory.

import { dlopen, FFIType, ptr } from "bun:ffi";
import { existsSync } from "node:fs";
import { EnvironmentError } from "../errors.ts";
import { Reader } from "./wire.ts";

const WIRE_VERSION = 1;
const ext = process.platform === "darwin" ? "dylib" : process.platform === "win32" ? "dll" : "so";
const LIB = process.env.LOFT_CORE_LIB ?? new URL(`../../rust/loft-core/target/release/${process.platform === "win32" ? "" : "lib"}loft_core.${ext}`, import.meta.url).pathname;
if (!existsSync(LIB)) throw new EnvironmentError(`LOFT_CORE=rust: no Rust core at ${LIB} (build it: cd rust/loft-core && cargo build --release)`, false);

const lib = dlopen(LIB, {
  loft_core_call: { args: [FFIType.u32, FFIType.ptr, FFIType.u64], returns: FFIType.u64 },
  loft_core_take: { args: [FFIType.ptr], returns: FFIType.void },
  loft_core_version: { args: [], returns: FFIType.u32 },
});
const version = lib.symbols.loft_core_version();
if (version !== WIRE_VERSION) throw new EnvironmentError(`LOFT_CORE=rust: ${LIB} speaks wire version ${version}, this checkout ${WIRE_VERSION} (rebuild it)`, false);

/** The kernels by number (rust/loft-core/src/lib.rs `dispatch`). */
export const KERNEL = { probe: 0, solve: 1, share: 2, verify: 3 } as const;

let dumped = 0;

/** A kernel's result: its payload, or its typed failure's payload (each kernel reads its own). */
export type Outcome = { readonly kind: "ok"; readonly r: Reader } | { readonly kind: "failed"; readonly r: Reader };

/** Runs one kernel on one packed input. Malformed bytes and panics are bugs in the core and throw. */
export function call(kernel: number, input: Uint8Array): Outcome {
  if (process.env.LOFT_CORE_DUMP !== undefined) Bun.write(`${process.env.LOFT_CORE_DUMP}/${kernel}-${dumped++}.bin`, input);
  const n = Number(lib.symbols.loft_core_call(kernel, input.byteLength === 0 ? null : ptr(input), input.byteLength));
  const out = new Uint8Array(n);
  lib.symbols.loft_core_take(ptr(out));
  const r = new Reader(out);
  const status = r.u8();
  switch (status) {
    case 0: return { kind: "ok", r };
    case 1: return { kind: "failed", r };
    case 2: throw new Error(`loft core: kernel ${kernel} could not read its input (a packer bug): ${r.str()}`);
    case 3: throw new Error(`loft core: kernel ${kernel} panicked: ${r.str()}`);
    default: throw new Error(`loft core: kernel ${kernel} returned status ${status}`);
  }
}

