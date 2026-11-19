// What a frame is a function of, as one record and one hash (docs/handoff_training.md "Identity"). The contract: the
// same `id` gives a byte-identical frame. Every producer (`loft serve`, `render`, `frames`, `clip`) builds the record
// here, from the bundle it drew and the device it drew on, so two paths to one frame give one id.
//
// The three parts:
//   build   the program the GPU ran (a hash of the lowered bundle: manifest, WGSL chunks, data blobs) and the library
//           that ran it (a hash of this checkout's src/, which holds the loader). The compiler's whole effect on pixels
//           is in the bundle, so variant, style and the scene's source are in `program` and need no field of their own.
//   frame   the view: camera (resolved, not its name), time, size, every param and input value (defaults filled in, so
//           an omitted value and its default agree), the precision actually used, and how the frame was drawn.
//   device  runtime, adapter and driver.
//
// This file is on the file loader's import closure (tools/check_loader_closure.ts): it imports types and node only.
import { createHash } from "node:crypto";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import type { Camera, View } from "./artefact.ts";
import type { Bundle, FramePrecision, Manifest } from "./host/manifest.ts";
import type { StillSpec } from "./host/still.ts";

/**
 * The program the GPU ran. `files`: the bundle is all of it. `js-tables`: a scene compiled in this process also fills
 * the named tables from JavaScript each frame, which the bundle does not hold, so `program` does not cover them and the
 * id is only as good as the scene file (a built directory never has such tables: `loft build` refuses them).
 */
export type Program =
  | { readonly kind: "files"; readonly sha256: string }
  | { readonly kind: "js-tables"; readonly sha256: string; readonly tables: readonly string[] };

/**
 * The camera a frame was drawn with: a TS host camera (resolved), or, drawn from a Bundle V1 file by loft-draw, one of
 * the bundle's cameras by name (the file's hash and the time fix where it is: it is evaluated on the GPU) or a host
 * look camera.
 */
export type FrameCamera =
  | Camera
  | { readonly kind: "named"; readonly name: string }
  | { readonly kind: "look"; readonly eye: readonly number[]; readonly target: readonly number[]; readonly up: readonly number[]; readonly fovY: number };

export type Identity = {
  readonly build: { readonly program: Program; readonly library: string; readonly tier: "compiled"; readonly precision: "f32" | "mixed" };
  readonly frame: {
    /** A stepped frame also depends on the exact input prefix at its retained checkpoint and presented tick. */
    readonly steps?: readonly { checkpoint: number; prefix: readonly number[]; tick: number; fraction: number }[];
    readonly camera: FrameCamera; readonly time: number; readonly size: readonly [number, number];
    readonly params: Readonly<Record<string, number>>; readonly inputs: Readonly<Record<string, readonly number[]>>;
    readonly precision: FramePrecision;
    /** `frame` (one draw) or a still (`--samples`: accumulated samples, shutter, aperture). */
    readonly drawn: StillSpec;
    readonly construction: boolean;
  };
  readonly device: { readonly runtime: string; readonly adapter: string; readonly driver: string };
};

/** The record's hash: sha256 of its canonical JSON, hex. */
export function identityId(i: Identity): string {
  return createHash("sha256").update(canonical(i)).digest("hex");
}

/**
 * JSON with keys sorted at every level, and -0 written as `-0` (JSON.stringify writes it as 0, but a scene can read
 * the sign, and serve keeps it: structured cloning carries it to the GPU child). Non-finite numbers are refused.
 */
export function canonical(v: unknown): string {
  if (typeof v === "number") {
    if (!Number.isFinite(v)) throw new Error(`identity: ${v} is not a finite number`);
    return Object.is(v, -0) ? "-0" : JSON.stringify(v);
  }
  if (v === null || typeof v === "string" || typeof v === "boolean") return JSON.stringify(v);
  if (Array.isArray(v)) return `[${v.map(canonical).join(",")}]`;
  if (typeof v === "object") {
    // An undefined field is absent, as in JSON.
    const keys = Object.keys(v).filter((k) => (v as Record<string, unknown>)[k] !== undefined).sort();
    return `{${keys.map((k) => `${JSON.stringify(k)}:${canonical((v as Record<string, unknown>)[k])}`).join(",")}}`;
  }
  throw new Error(`identity: a ${typeof v} has no canonical form`);
}

/**
 * The bundle's hash: its manifest as written to manifest.json, then each chunk text and blob in manifest order, each
 * length-prefixed. A scene compiled in memory and the directory `loft build` wrote of it hash the same.
 */
export function programOf(bundle: Bundle): Program {
  const m = bundle.manifest;
  const h = createHash("sha256").update("loft program 1\n");
  const part = (bytes: string | Uint8Array) => {
    const b = typeof bytes === "string" ? Buffer.from(bytes, "utf8") : bytes;
    h.update(`${b.byteLength}\n`).update(b);
  };
  part(JSON.stringify(m));
  for (const name of m.chunks) part(need(bundle.chunks.get(name), `chunk ${name}`));
  for (const { name } of m.blobs) part(need(bundle.blobs.get(name), `blob ${name}`));
  const sha256 = h.digest("hex");
  const js = m.tables.filter((t) => t.fill.kind === "cpu").map((t) => t.name);
  return js.length === 0 ? { kind: "files", sha256 } : { kind: "js-tables", sha256, tables: js };
}

function need<T>(v: T | undefined, what: string): T {
  if (v === undefined) throw new Error(`identity: the bundle's manifest names ${what}, which the bundle does not hold`);
  return v;
}

let library: string | undefined;
/**
 * A hash of every file under this checkout's src/ (paths and bytes), once per process. Not a git commit: a vendored
 * copy has no .git, and an uncommitted edit changes pixels as surely as a commit does.
 */
export function libraryOf(): string {
  if (library !== undefined) return library;
  const src = dirname(new URL(import.meta.url).pathname);
  const files: string[] = [];
  const walk = (dir: string) => {
    for (const e of readdirSync(dir, { withFileTypes: true })) {
      const p = join(dir, e.name);
      if (e.isDirectory()) walk(p);
      else if (e.isFile()) files.push(p);
    }
  };
  walk(src);
  const h = createHash("sha256").update("loft library 1\n");
  for (const f of files.map((p) => relative(src, p)).sort()) {
    const bytes = readFileSync(join(src, f));
    h.update(`${f}\n${bytes.byteLength}\n`).update(bytes);
  }
  return (library = h.digest("hex"));
}

/** The build part, for a bundle as loaded. */
export function buildOf(bundle: Bundle): Identity["build"] {
  // Formats 4 and 5 may carry the mixed overlay (src/host/precision.ts); the f32 descriptors are always there.
  return { program: programOf(bundle), library: libraryOf(), tier: "compiled", precision: bundle.manifest.precision === undefined && bundle.manifest.version !== 4 ? "f32" : "mixed" };
}

/** The device part. bun-webgpu's version is the Dawn build; Dawn's adapter description names the driver where it can. */
export function deviceOf(device: GPUDevice): Identity["device"] {
  const a = (device as GPUDevice & { adapterInfo?: GPUAdapterInfo }).adapterInfo;
  if (a === undefined) throw new Error("identity: this device reports no adapterInfo (bun-webgpu 0.1.7 does); the device part cannot be recorded");
  return {
    runtime: `bun ${Bun.version}, bun-webgpu ${bunWebgpuVersion()}`,
    adapter: [a.vendor, a.architecture, a.device].filter((s) => s !== "").join(" / "),
    driver: a.description,
  };
}

function bunWebgpuVersion(): string {
  // The package's entry is index.js at its root; its package.json sits beside it.
  const pkg = join(dirname(Bun.resolveSync("bun-webgpu", import.meta.dir)), "package.json");
  return (JSON.parse(readFileSync(pkg, "utf8")) as { version: string }).version;
}

/**
 * The frame part. Params and inputs are the values the frame used: the manifest's defaults, with the view's over them.
 * `time` is the view's own field (an input of that name is the host's, never the view's).
 */
export function frameOf(m: Pick<Manifest, "params" | "inputs">, view: View, precision: FramePrecision, drawn: StillSpec): Identity["frame"] {
  const params: Record<string, number> = {}, inputs: Record<string, readonly number[]> = {};
  for (const p of m.params) params[p.name] = view.params?.[p.name] ?? p.def;
  for (const i of m.inputs) {
    if (i.name === "time") continue;
    const v = view.inputs?.[i.name];
    inputs[i.name] = v === undefined ? i.def : typeof v === "number" ? [v] : v;
  }
  return { camera: view.camera, time: view.time, size: [view.width, view.height], params, inputs, precision, drawn, construction: view.construction ?? false };
}
