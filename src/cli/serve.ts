// `loft serve`: one long-lived process that owns the GPU device and draws requests, one at a time. A scene is compiled
// in a child process that has no GPU (`loft build` with LOFT_NO_GPU=1, wrapped in the caller's `--confine` command);
// only the built directory (docs/artefact_format.md) crosses into this process, which never runs scene code: it reads
// files, as `render --artefact` does, and checks after every request that no compiler module was loaded.
//
// Protocol: JSON lines. Each line on stdin is one request; each line on stdout is one response, tagged with the
// request's id, in the order requests finish. The first line out, unasked, is {"ready": true, "adapter": ...}. Requests
// compile concurrently (up to --compiles at once) and draw one at a time. Anything else the process prints goes to
// stderr. At the end of stdin the server finishes what it has and exits 0. docs/handoff_training.md "loft serve".
//
//   {"id": "a", "scene": "train/x.ts", "camera": "fixed", "times": [0, 1.5], "size": "512x512", "out": "out/a"}
//   {"id": "b", "artefact": "out/files/x", "camera": {"kind": "perspective", "eye": [4, 4, 8], "target": [0, 0.7, 0], "fovY": 0.62}, ...}
//
//   {"id": "a", "ok": true, "frames": [{"time": 0, "file": "out/a/0.rgba", "sha256": "..."}, ...], "ms": {...}, "cache": {...}}
//   {"id": "a", "ok": false, "exit": 4, "line": "loft: scene: ...", "detail": [...]}
//
// A failed request is answered and the server goes on; `exit` and `line` are what `loft render` would have ended with
// (docs/errors.md). A lost device is answered with exit 6 and ends the server with exit 6: start a new one.
import { createHash } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import type { Camera, View } from "../artefact.ts";
import { EnvironmentError, LoadError } from "../errors.ts";
import { cachingDevice, type CachingDevice } from "../host/cache.ts";
import { readBundle } from "../host/files.ts";
import { headlessDevice } from "../host/headless.ts";
import { loadBundle, NotDataError } from "../host/host.ts";
import type { Bundle } from "../host/manifest.ts";
import { parseSize, UsageError, type Size } from "./args.ts";
import { FALLBACK_CAMERAS } from "./cameras.ts";
import { compilerModulesLoaded } from "./run_files.ts";
import { ending, headed, type Verdict } from "./verdict.ts";

export type ServeCommand = {
  readonly kind: "serve";
  /** Compiles running at once (each a child process); draws are always one at a time. */
  readonly compiles: number;
  /** The command a compile runs inside (a sandbox), prefixed to `bun src/cli/main.ts build ...`; empty for none. */
  readonly confine: readonly string[];
  /** A module whose default export is called with each artefact before it reaches the GPU; it throws to refuse one. */
  readonly check: string | undefined;
};

type Source = { readonly kind: "scene"; readonly path: string } | { readonly kind: "artefact"; readonly dir: string };
type CameraAsk = { readonly kind: "named"; readonly name: string } | { readonly kind: "given"; readonly camera: Camera };
type Request = { readonly id: string | number; readonly source: Source; readonly camera: CameraAsk; readonly times: readonly number[]; readonly size: Size; readonly out: string };

type Answer =
  | { readonly id: string | number | null; readonly ok: true; readonly frames: readonly { readonly time: number; readonly file: string; readonly sha256: string }[];
      readonly ms: { readonly compile?: number; readonly load: number; readonly frames: number }; readonly cache: ReturnType<CachingDevice["counts"]> }
  | { readonly id: string | number | null; readonly ok: false; readonly exit: number; readonly line: string; readonly detail: readonly string[]; readonly deviceLost?: true };

const MAIN = new URL("./main.ts", import.meta.url).pathname;

// ---- the request, classified once ------------------------------------------------------------------------------

const isVec3 = (v: unknown): v is [number, number, number] => Array.isArray(v) && v.length === 3 && v.every((x) => typeof x === "number" && Number.isFinite(x));

function cameraOf(v: unknown): CameraAsk {
  if (typeof v === "string") return { kind: "named", name: v };
  const c = v as Record<string, unknown> | null;
  if (typeof c !== "object" || c === null || !isVec3(c.eye) || !isVec3(c.target)) throw new UsageError(`camera: given ${JSON.stringify(v)}; expected a camera name, or {kind, eye: [x, y, z], target: [x, y, z], fovY | height}`);
  switch (c.kind) {
    case "perspective":
      if (!(typeof c.fovY === "number" && c.fovY > 0 && c.fovY < Math.PI)) throw new UsageError(`camera: fovY ${JSON.stringify(c.fovY)}; expected radians in (0, pi)`);
      return { kind: "given", camera: { kind: "perspective", eye: c.eye, target: c.target, fovY: c.fovY } };
    case "orthographic":
      if (!(typeof c.height === "number" && c.height > 0 && Number.isFinite(c.height))) throw new UsageError(`camera: height ${JSON.stringify(c.height)}; expected a positive number`);
      return { kind: "given", camera: { kind: "orthographic", eye: c.eye, target: c.target, height: c.height } };
    default: throw new UsageError(`camera: kind ${JSON.stringify(c.kind)}; expected "perspective" or "orthographic"`);
  }
}

/** The id of a line, if it has a usable one: a failed parse is still answered under it. */
function idOf(r: unknown): string | number | null {
  const id = (r as { id?: unknown } | null)?.id;
  return typeof id === "string" || (typeof id === "number" && Number.isFinite(id)) ? id : null;
}

function requestOf(line: string): Request {
  let r: Record<string, unknown>;
  try { r = JSON.parse(line); } catch (e) { throw new UsageError(`request: not JSON: ${e instanceof Error ? e.message : String(e)}`); }
  if (typeof r !== "object" || r === null || Array.isArray(r)) throw new UsageError("request: expected a JSON object");
  const known = ["id", "scene", "artefact", "camera", "times", "size", "out"];
  const stray = Object.keys(r).filter((k) => !known.includes(k));
  if (stray.length > 0) throw new UsageError(`request: unknown field ${stray.map((k) => `"${k}"`).join(", ")} (fields: ${known.join(", ")})`);
  const id = idOf(r);
  if (id === null) throw new UsageError("request: needs an \"id\" (a string or a number), echoed in its response");
  if ((r.scene === undefined) === (r.artefact === undefined)) throw new UsageError("request: give one of \"scene\" (a .ts file, compiled here in a child process) or \"artefact\" (a directory `loft build` wrote)");
  const path = (field: string, v: unknown): string => {
    if (typeof v !== "string" || v === "") throw new UsageError(`request: "${field}" must be a path`);
    return resolve(v);
  };
  const source: Source = r.scene !== undefined ? { kind: "scene", path: path("scene", r.scene) } : { kind: "artefact", dir: path("artefact", r.artefact) };
  if (!Array.isArray(r.times) || r.times.length === 0 || !r.times.every((t) => typeof t === "number" && Number.isFinite(t))) throw new UsageError("request: \"times\" must be a non-empty list of numbers (seconds)");
  if (typeof r.size !== "string") throw new UsageError("request: \"size\" must be WIDTHxHEIGHT, e.g. \"512x512\"");
  if (r.camera === undefined) throw new UsageError("request: needs a \"camera\": a name the scene declares (or front, side, top, three), or a camera object");
  return { id, source, camera: cameraOf(r.camera), times: r.times as number[], size: parseSize(r.size), out: path("out", r.out) };
}

// ---- verdicts --------------------------------------------------------------------------------------------------

/**
 * A failure in this process, classified. Nothing here runs scene code, so a scene's typed refusals cannot arise; the
 * classes that can are named. verdict.ts `verdictOf` would import the library to test for the rest, which this process
 * must not load.
 */
function verdictHere(e: unknown): Verdict {
  if (!(e instanceof Error)) return { kind: "internal", message: String(e), stack: "" };
  if (e instanceof UsageError) return { kind: "usage", message: e.message };
  if (e instanceof LoadError) return { kind: "load", message: e.message };
  if (e instanceof NotDataError) return { kind: "scene", message: e.message, site: undefined };
  if (e instanceof EnvironmentError) return { kind: "environment", message: e.message };
  return { kind: "internal", message: `${e.name === "Error" ? "" : `${e.name}: `}${e.message}`, stack: e.stack ?? "" };
}

function failed(id: string | number | null, v: Verdict): Answer {
  const e = ending(v);
  return { id, ok: false, exit: e.code, line: e.line, detail: v.kind === "internal" ? [...e.detail, ...v.stack.split("\n")] : e.detail };
}

/** The verdict a `loft build` child ended with: its exit code and its last `loft:` line, passed on as they are. */
class ChildVerdict extends Error {
  constructor(readonly response: Omit<Extract<Answer, { ok: false }>, "id">) { super(response.line); }
}

// ---- compile: a child process, no GPU --------------------------------------------------------------------------

async function compile(cmd: ServeCommand, scene: string, dir: string): Promise<void> {
  const child = Bun.spawn([...cmd.confine, process.execPath, MAIN, "build", scene, "--out", dir], {
    env: { ...process.env, LOFT_NO_GPU: "1" }, stdin: "ignore", stdout: "ignore", stderr: "pipe",
  });
  const [stderr, code] = await Promise.all([new Response(child.stderr).text(), child.exited]);
  if (code === 0) return;
  const lines = stderr.trimEnd().split("\n");
  const last = lines.at(-1) ?? "";
  // A child that died by a signal (a confinement's CPU limit) or ended without Loft's last line has no verdict of its
  // own; exit 1 is the taxonomy's "unknown" (neither the scene's fault nor the machine's, for certain).
  if (child.signalCode !== null || !/^loft: /.test(last) || code < 1 || code > 6) {
    throw new ChildVerdict({ ok: false, exit: 1, line: `loft: internal (a bug in Loft, or a scene mistake no constructor catches yet): the compile ended ${child.signalCode !== null ? `by signal ${child.signalCode}` : `with exit ${code}`} and no verdict`, detail: lines.slice(-20) });
  }
  throw new ChildVerdict({ ok: false, exit: code, line: last, detail: lines.slice(0, -1).slice(-20) });
}

// ---- draw: this process, one at a time -------------------------------------------------------------------------

type Gpu = { readonly cache: CachingDevice; readonly lost: () => string | undefined; readonly adapter: GPUAdapterInfo | undefined };

async function freshGpu(): Promise<Gpu> {
  const device = await headlessDevice();
  let lost: string | undefined;
  void device.lost.then((info) => { lost = `device lost: ${info.reason}: ${info.message}`; });
  return { cache: cachingDevice(device), lost: () => lost, adapter: (device as GPUDevice & { adapterInfo?: GPUAdapterInfo }).adapterInfo };
}

function cameraFor(ask: CameraAsk, bundle: Bundle): Camera {
  switch (ask.kind) {
    case "given": return ask.camera;
    case "named": {
      const c = bundle.manifest.cameras[ask.name] ?? FALLBACK_CAMERAS[ask.name];
      if (c === undefined) throw new UsageError(`camera "${ask.name}": the scene names [${Object.keys(bundle.manifest.cameras).join(", ")}], defaults are [${Object.keys(FALLBACK_CAMERAS).join(", ")}]`);
      return c;
    }
  }
}

export async function serve(cmd: ServeCommand) {
  // The protocol owns stdout; anything the library prints goes to stderr.
  const write = (r: Answer | object) => process.stdout.write(JSON.stringify(r) + "\n");
  console.log = console.error;
  const check: (b: Bundle) => void = cmd.check === undefined ? () => {} : (await import(resolve(cmd.check))).default;
  if (typeof check !== "function") throw new UsageError(`--check ${cmd.check}: its default export is not a function`);

  let gpu = await freshGpu();
  const a = gpu.adapter;
  write({ ready: true, adapter: a === undefined ? null : { vendor: a.vendor, architecture: a.architecture, device: a.device, description: a.description } });

  let compiling = 0;
  const waiting: (() => void)[] = [];
  const compileSlot = async <T>(body: () => Promise<T>): Promise<T> => {
    // A finishing compile hands its slot straight to the next in line, so a newcomer cannot take it in between.
    if (compiling < cmd.compiles) compiling++;
    else await new Promise<void>((go) => waiting.push(go));
    try { return await body(); } finally { const next = waiting.shift(); if (next !== undefined) next(); else compiling--; }
  };
  let drawing: Promise<unknown> = Promise.resolve();
  const drawSlot = <T>(body: () => Promise<T>): Promise<T> => {
    const turn = drawing.then(body);
    drawing = turn.catch(() => {});
    return turn;
  };

  /** Loads and draws one built directory, retrying a transient GPU refusal on a fresh device as `render` does. */
  const draw = async (req: Request, dir: string): Promise<Extract<Answer, { ok: true }>> => {
    let bundle: Bundle;
    try { bundle = readBundle(dir); } catch (e) { throw new LoadError(`artefact ${dir}: ${e instanceof Error ? e.message : String(e)}`); }
    try { check(bundle); } catch (e) { throw new LoadError(`artefact ${dir}: refused by --check: ${e instanceof Error ? e.message : String(e)}`); }
    const camera = cameraFor(req.camera, bundle);
    const t0 = performance.now();
    for (let attempt = 1; ; attempt++) {
      try {
        const loaded = await gpu.cache.settle(() => loadBundle(gpu.cache.device, bundle, { kind: "none" }));
        const t1 = performance.now();
        try {
          mkdirSync(req.out, { recursive: true });
          const frames = [];
          for (const [k, time] of req.times.entries()) {
            const view: View = { camera, ...req.size, time, params: {}, inputs: {}, construction: false };
            const pixels = await loaded.frame(view);
            const file = join(req.out, `${k}.rgba`);
            writeFileSync(file, pixels);
            frames.push({ time, file, sha256: createHash("sha256").update(pixels).digest("hex") });
          }
          return { id: req.id, ok: true, frames, ms: { load: t1 - t0, frames: performance.now() - t1 }, cache: gpu.cache.counts() };
        } finally {
          loaded.destroy();
        }
      } catch (e) {
        if (!(e instanceof EnvironmentError && e.transient) || attempt === 3 || gpu.lost() !== undefined) throw e;
        console.error(`loft serve: request ${JSON.stringify(req.id)}: load attempt ${attempt} failed in the GPU (not the scene): ${headed(e.message)[0]}`);
        const old = gpu;
        gpu = await freshGpu();
        old.cache.device.destroy();
      }
    }
  };

  /** The directory a request draws: built here in a child (timed, without the wait for a slot), or given. */
  const builtFrom = async (req: Request): Promise<{ readonly dir: string; readonly compileMs: number | undefined }> => {
    switch (req.source.kind) {
      case "artefact": return { dir: req.source.dir, compileMs: undefined };
      case "scene": {
        const scene = req.source.path, dir = join(req.out, "artefact");
        const compileMs = await compileSlot(async () => {
          const t0 = performance.now();
          await compile(cmd, scene, dir);
          return performance.now() - t0;
        });
        return { dir, compileMs };
      }
    }
  };

  const handle = async (line: string): Promise<void> => {
    let id: string | number | null = null;
    try { id = idOf(JSON.parse(line)); } catch { /* answered below as a usage error */ }
    let response: Answer;
    try {
      const req = requestOf(line);
      const built = await builtFrom(req);
      const drawn = await drawSlot(async () => {
        try { return await draw(req, built.dir); } finally {
          // Inside the slot: the cache is emptied between loads, never during one.
          if (gpu.cache.bound()) console.error("loft serve: the GPU object cache passed its bound and was emptied");
        }
      });
      response = built.compileMs === undefined ? drawn : { ...drawn, ms: { compile: built.compileMs, ...drawn.ms } };
    } catch (e) {
      response = e instanceof ChildVerdict ? { id, ...e.response } : failed(id, verdictHere(e));
    }
    const lost = gpu.lost();
    if (lost !== undefined) {
      write(failed(id, { kind: "environment", message: `${lost}; the server ends: start a new one` }));
      process.exit(6);
    }
    // Witness, from what this process actually loaded: scene code and the compiler stay in the child.
    const loadedNow = compilerModulesLoaded();
    if (loadedNow.length > 0) {
      write(failed(id, { kind: "internal", message: `loft serve loaded the compiler (${loadedNow.join(", ")}); the server ends`, stack: "" }));
      process.exit(1);
    }
    write(response);
  };

  const inflight = new Set<Promise<void>>();
  for await (const line of console) {
    if (line.trim() === "") continue;
    const p = handle(line).finally(() => inflight.delete(p));
    inflight.add(p);
  }
  await Promise.all(inflight);
  process.exit(0);
}
