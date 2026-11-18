// `loft serve`: one warm child owns the GPU device and draws requests, one at a time. A scene compiles in a separate
// GPU-less child (`loft build` with LOFT_NO_GPU=1, wrapped in the caller's `--confine` command). The GPU child reads
// the built directory as `render --artefact` does. Neither it nor the server loads the compiler or scene code.
//
// Protocol: JSON lines. Each line on stdin is one request; each line on stdout is one response, tagged with the
// request's id, in the order requests finish. The first line out, unasked, is {"ready": true, "adapter": ...}. Requests
// compile concurrently (up to --compiles at once) and draw one at a time. Anything else the process prints goes to
// stderr. At the end of stdin the server finishes what it has and exits 0. docs/handoff_training.md "loft serve".
//
//   {"id": "a", "scene": "train/x.ts", "camera": "fixed", "times": [0, 1.5], "size": "512x512", "out": "out/a"}
//   {"id": "b", "artefact": "out/files/x", "camera": {"kind": "perspective", "eye": [4, 4, 8], "target": [0, 0.7, 0], "fovY": 0.62}, ...,
//    "params": {"stormwing.span": 1.4}, "inputs": {"strike": 1, "ship.at": [14, 2]}}
//
//   {"id": "a", "ok": true, "identity": {"build": ..., "frame": ..., "device": ...}, "frames": [{"time": 0, "file": "out/a/0.rgba", "sha256": "...", "id": "..."}, ...], "ms": {...}, "cache": {...}}
//   {"id": "a", "ok": false, "exit": 4, "line": "loft: scene: ...", "detail": [...]}
//
// `identity` is src/identity.ts's record without the frame's time; frame k's `id` is the hash of that record with
// `frame.time` set to its time. The same `id` is the same RGBA (docs/handoff_training.md "Identity").
//
// A failed request is answered and the server goes on; `exit` and `line` are what `loft render` would have ended with
// (docs/errors.md), plus 7 (`budget`) for a limit of this server's: a compile or draw deadline, a compile CPU limit,
// or more than --pending requests at once. An unexpected device loss answers every request in flight with exit 6,
// then ends the server with exit 6: start a new one.
//
// Ownership: a scene builds in its own private directory (under $TMPDIR, removed after its answer), and no two
// requests in flight may name overlapping `out` directories (the later one is refused), so frames written to `out`
// are the request's own. An artefact is checked (names, links, sizes against --limits) before its files are read.
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { constants, tmpdir } from "node:os";
import { join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import type { Camera, View } from "../artefact.ts";
import { EnvironmentError, LoadError, SceneError } from "../errors.ts";
import { cachingDevice, type CachingDevice } from "../host/cache.ts";
import { readBundle, type ArtefactLimits } from "../host/files.ts";
import { headlessDevice } from "../host/headless.ts";
import { loadBundle, NotDataError } from "../host/host.ts";
import type { Bundle, FramePrecision, Precision } from "../host/manifest.ts";
import { parsePrecision, parseSize, UsageError, type Size } from "./args.ts";
import { FALLBACK_CAMERAS } from "./cameras.ts";
import { checkInputs, type InputValues } from "./inputs.ts";
import { compilerModulesLoaded } from "./run_files.ts";
import { selectPrecision } from "../host/precision.ts";
import { buildOf, canonical, deviceOf, frameOf, identityId, type Identity } from "../identity.ts";
import { ending, exitWith, headed, type Verdict } from "./verdict.ts";

export type ServeCommand = {
  readonly kind: "serve";
  /** Compiles running at once (each a child process); draws are always one at a time. */
  readonly compiles: number;
  /** The command a compile runs inside (a sandbox), prefixed to `bun src/cli/main.ts build ...`; empty for none. */
  readonly confine: readonly string[];
  /** A module whose default export is called with each artefact before it reaches the GPU; it throws to refuse one. */
  readonly check: string | undefined;
  /** Requests admitted and not yet answered, at most; one more is refused at once as `budget`. */
  readonly pending: number;
  /** A compile child still running after this many seconds is killed and answered as `budget`. */
  readonly compileSeconds: number;
  /** Time per draw slot, including artefact loading and all its frames. Expiry kills the GPU child and answers `budget`. */
  readonly drawSeconds: number;
  /** What an artefact (given, or built here) may hold; checked before its files are read. */
  readonly limits: ArtefactLimits;
};

type Source = { readonly kind: "scene"; readonly path: string } | { readonly kind: "artefact"; readonly dir: string };
type CameraAsk = { readonly kind: "named"; readonly name: string } | { readonly kind: "given"; readonly camera: Camera };
type Request = {
  readonly id: string | number; readonly source: Source; readonly camera: CameraAsk; readonly times: readonly number[]; readonly size: Size; readonly out: string;
  readonly precision: Precision | undefined;
  readonly params: Readonly<Record<string, number>>; readonly inputs: InputValues;
};

/** An answer's identity: every part but the frame's time, which each frame carries. */
type Shared = Omit<Identity, "frame"> & { readonly frame: Omit<Identity["frame"], "time"> };

type Answer =
  | { readonly id: string | number | null; readonly ok: true; readonly identity: Shared; readonly frames: readonly { readonly time: number; readonly file: string; readonly sha256: string; readonly id: string }[];
      readonly ms: { readonly compile?: number; readonly load: number; readonly frames: number }; readonly cache?: ReturnType<CachingDevice["counts"]> }
  | { readonly id: string | number | null; readonly ok: false; readonly exit: number; readonly line: string; readonly detail: readonly string[] };

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
  const known = ["id", "scene", "artefact", "camera", "times", "size", "out", "params", "inputs", "precision"];
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
  return { id, source, camera: cameraOf(r.camera), times: r.times as number[], size: parseSize(r.size), out: path("out", r.out), precision: parsePrecision(r.precision), params: paramsOf(r.params), inputs: inputsOf(r.inputs) };
}

/** `{name: value}` as given, or `{}` when absent; anything but a plain object is refused. */
function namedOf(field: string, v: unknown): Record<string, unknown> {
  if (v === undefined) return {};
  if (typeof v !== "object" || v === null || Array.isArray(v)) throw new UsageError(`request: "${field}" must be an object of name: value, given ${JSON.stringify(v)}`);
  return v as Record<string, unknown>;
}

function paramsOf(v: unknown): Record<string, number> {
  return Object.fromEntries(Object.entries(namedOf("params", v)).map(([name, x]) => {
    if (typeof x !== "number" || !Number.isFinite(x)) throw new UsageError(`request: params "${name}": ${JSON.stringify(x)} is not a number`);
    return [name, x];
  }));
}

/** An input is a number, or a list of numbers for a vector input (as `--input name=x,y,z`). */
function inputsOf(v: unknown): InputValues {
  return Object.fromEntries(Object.entries(namedOf("inputs", v)).map(([name, x]) => {
    const xs = typeof x === "number" ? [x] : x;
    if (!Array.isArray(xs) || xs.length === 0 || !xs.every((n) => typeof n === "number" && Number.isFinite(n))) throw new UsageError(`request: inputs "${name}": ${JSON.stringify(x)} is not a number or a list of numbers`);
    return [name, xs as number[]];
  }));
}

/** The request's overrides against the artefact, before a draw: a typo fails by name. */
function checkParams(params: Request["params"], m: Bundle["manifest"]): void {
  for (const name of Object.keys(params)) {
    if (!m.params.some((p) => p.name === name)) throw new UsageError(`params "${name}": the scene declares no such param (it has: ${m.params.map((p) => p.name).join(", ") || "none"}). Fix: drop it from "params", or declare it: param("${name}", 1)`);
  }
}

// ---- verdicts --------------------------------------------------------------------------------------------------

/**
 * A failure in this process, classified. Nothing here runs scene code, so a scene's typed refusals cannot arise; the
 * classes that can are named. verdict.ts `verdictOf` would import the library to test for the rest, which this process
 * must not load.
 */
function verdictHere(e: unknown): Verdict {
  if (!(e instanceof Error)) return { kind: "internal", message: String(e), stack: "" };
  if (e instanceof Budget) return { kind: "budget", message: e.message };
  if (e instanceof UsageError) return { kind: "usage", message: e.message };
  if (e instanceof LoadError) return { kind: "load", message: e.message };
  if (e instanceof SceneError) return { kind: "scene", message: e.message, site: undefined };
  if (e instanceof NotDataError) return { kind: "scene", message: e.message, site: undefined };
  if (e instanceof EnvironmentError) return { kind: "environment", message: e.message };
  return { kind: "internal", message: `${e.name === "Error" ? "" : `${e.name}: `}${e.message}`, stack: e.stack ?? "" };
}

function failed(id: string | number | null, v: Verdict): Extract<Answer, { ok: false }> {
  const e = ending(v);
  return { id, ok: false, exit: e.code, line: e.line, detail: v.kind === "internal" ? [...e.detail, ...v.stack.split("\n")] : e.detail };
}

/** The verdict a `loft build` child ended with: its exit code and its last `loft:` line, passed on as they are. */
class ChildVerdict extends Error {
  constructor(readonly response: Omit<Extract<Answer, { ok: false }>, "id">) { super(response.line); }
}

/** A limit of this server's ran out (a deadline, a CPU limit, the pending bound): verdict `budget`, exit 7. */
class Budget extends Error {}

// ---- compile: a child process, no GPU --------------------------------------------------------------------------

async function compile(cmd: ServeCommand, scene: string, dir: string, precision: Precision, children: Set<{ kill(signal?: number | NodeJS.Signals): void }>): Promise<void> {
  const child = Bun.spawn([...cmd.confine, process.execPath, MAIN, "build", scene, "--out", dir, "--precision", precision], {
    env: { ...process.env, LOFT_NO_GPU: "1" }, stdin: "ignore", stdout: "ignore", stderr: "pipe",
  });
  children.add(child);
  // The deadline kills the direct child. A --confine wrapper should exec the build (or forward the signal), or the
  // build outlives it; the answer is `budget` either way.
  let late = false;
  const timer = setTimeout(() => { late = true; child.kill("SIGKILL"); }, cmd.compileSeconds * 1000);
  let stderr: string, code: number;
  try { [stderr, code] = await Promise.all([new Response(child.stderr).text(), child.exited]); } finally { clearTimeout(timer); children.delete(child); }
  if (code === 0 && !late) return;
  const lines = stderr.trimEnd().split("\n");
  const last = lines.at(-1) ?? "";
  if (late) throw new Budget(`compile of ${scene}: still running after ${cmd.compileSeconds} s (--compile-seconds), killed`);
  // A wrapper that does not exec reports a signal as 128 + its number, as a shell does.
  const signal = child.signalCode ?? (code > 128 ? Object.entries(constants.signals).find(([, n]) => n === code - 128)?.[0] : undefined);
  if (signal === "SIGXCPU") throw new Budget(`compile of ${scene}: killed by SIGXCPU, the CPU-time limit --confine set`);
  // Any other signal, or an end without Loft's last line, has no verdict of its own; exit 1 is the taxonomy's
  // "unknown" (neither the scene's fault nor the machine's, for certain).
  if (signal !== undefined || !/^loft: /.test(last) || code < 1 || code > 7) {
    throw new ChildVerdict({ ok: false, exit: 1, line: `loft: internal (a bug in Loft, or a scene mistake no constructor catches yet): the compile ended ${signal !== undefined ? `by signal ${signal}` : `with exit ${code}`} and no verdict`, detail: lines.slice(-20) });
  }
  throw new ChildVerdict({ ok: false, exit: code, line: last, detail: lines.slice(0, -1).slice(-20) });
}

// ---- draw: the warm GPU child ----------------------------------------------------------------------------------

type Gpu = { readonly cache: CachingDevice; readonly lost: () => string | undefined; readonly adapter: GPUAdapterInfo | undefined; readonly device: Identity["device"] };
type Draw = { readonly req: Request; readonly dir: string; readonly limits: ArtefactLimits };
type DrawMessage =
  | { readonly kind: "ready"; readonly adapter: Pick<GPUAdapterInfo, "vendor" | "architecture" | "device" | "description"> | null }
  | { readonly kind: "answer"; readonly answer: Answer }
  | { readonly kind: "stop"; readonly verdict: Verdict };

/** Two `out` directories overlap when they are one, or one holds the other. */
const overlaps = (a: string, b: string) => a === b || a.startsWith(b + sep) || b.startsWith(a + sep);

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

/** Keeps the deadline outside Dawn, including synchronous native calls. Only artefact paths and request data cross IPC. */
function startDrawWorker(check: string | undefined, stopped: (v: Verdict) => void) {
  type Boot = Exclude<DrawMessage, { kind: "answer" }>;
  let boot!: (message: Boot) => void;
  const ready = new Promise<Boot>((resolve) => { boot = resolve; });
  let answer: ((value: Answer) => void) | undefined;
  let starting = true, retired = false;
  const stop = (verdict: Verdict) => {
    if (retired) return;
    retired = true;
    if (starting) boot({ kind: "stop", verdict });
    else stopped(verdict);
  };
  const child = Bun.spawn([process.execPath, import.meta.path, ...(check === undefined ? [] : [resolve(check)])], {
    // Structured cloning preserves signed zero in scene inputs and times.
    stdin: "ignore", stdout: "ignore", stderr: "inherit", serialization: "advanced",
    ipc(message: DrawMessage) {
      if (retired) return;
      switch (message.kind) {
        case "ready": starting = false; boot(message); break;
        case "answer": {
          const finish = answer;
          answer = undefined;
          finish?.(message.answer);
          break;
        }
        case "stop": stop(message.verdict); break;
      }
    },
    onExit(child, code, signal) {
      stop({ kind: "environment", message: `GPU process ${child.pid} ended (${signal ?? `exit ${code}`}); the server ends: start a new one` });
    },
  });
  return {
    ready,
    draw: (work: Draw) => new Promise<Answer>((resolve) => { answer = resolve; child.send(work); }),
    async kill() {
      retired = true;
      child.kill("SIGKILL");
      await child.exited;
    },
  };
}

/** `loft-draw serve` (rust/crates/loft-draw), its protocol version, and where `tools/dev.sh build loft-draw --fast` puts it. */
const LOFT_DRAW = fileURLToPath(new URL("../../rust/target/lab/loft-draw", import.meta.url));
const DRAW_WIRE = 1;
/** The child's lines (rust/crates/loft-draw/src/serve.rs): `ready` once, then one reply per request, in order. */
type DrawReady = { readonly wire: number; readonly adapter: Extract<DrawMessage, { kind: "ready" }>["adapter"]; readonly device: Identity["device"] };
type DrawReply =
  | { readonly ok: true; readonly precision: FramePrecision; readonly frames: readonly { readonly file: string; readonly sha256: string }[]; readonly ms: { readonly load: number; readonly frames: number } }
  | { readonly ok: false; readonly verdict: Verdict };

/**
 * The draw child as `loft-draw serve`, with startDrawWorker's interface. This process reads and checks the artefact
 * (limits, --check, names, camera) and builds each frame's identity; the child loads, draws and writes `out/k.rgba`.
 * Only resolved request data crosses the pipe, as canonical JSON (it keeps -0).
 */
function startRustDraw(checkPath: string | undefined, stopped: (v: Verdict) => void): ReturnType<typeof startDrawWorker> {
  type Boot = Exclude<DrawMessage, { kind: "answer" }>;
  let starting = true, retired = false, boot!: (message: Boot) => void;
  const ready = new Promise<Boot>((go) => { boot = go; });
  const stop = (verdict: Verdict) => {
    if (retired) return;
    retired = true;
    if (starting) boot({ kind: "stop", verdict });
    else stopped(verdict);
  };
  if (!existsSync(LOFT_DRAW)) {
    stop({ kind: "environment", message: `${LOFT_DRAW} is missing. Fix: tools/dev.sh build loft-draw --fast` });
    return { ready, draw: () => new Promise<Answer>(() => {}), kill: async () => {} };
  }
  const child = Bun.spawn([LOFT_DRAW, "serve"], {
    stdin: "pipe", stdout: "pipe", stderr: "inherit",
    onExit(child, code, signal) { stop({ kind: "environment", message: `loft-draw ${child.pid} ended (${signal ?? `exit ${code}`}); the server ends: start a new one` }); },
  });
  const reader = child.stdout.getReader(), decoder = new TextDecoder();
  let buffered = "";
  const line = async <T,>(): Promise<T> => {
    for (let at = buffered.indexOf("\n"); at < 0; at = buffered.indexOf("\n")) {
      const { value, done } = await reader.read();
      if (done) return new Promise(() => {}); // onExit answers
      buffered += decoder.decode(value, { stream: true });
    }
    const [head, rest] = [buffered.slice(0, buffered.indexOf("\n")), buffered.slice(buffered.indexOf("\n") + 1)];
    buffered = rest;
    return JSON.parse(head) as T;
  };
  let check: (b: Bundle) => void = () => {};
  let device: Identity["device"];
  void (async () => {
    try {
      if (checkPath !== undefined) check = (await import(resolve(checkPath))).default;
      if (typeof check !== "function") throw new UsageError(`--check ${checkPath}: its default export is not a function`);
      const r = await line<DrawReady>();
      if (r.wire !== DRAW_WIRE) throw new EnvironmentError(`${LOFT_DRAW} speaks wire ${r.wire}, this checkout ${DRAW_WIRE}: it is stale. Fix: tools/dev.sh build loft-draw --fast`, false);
      device = r.device;
      starting = false;
      boot({ kind: "ready", adapter: r.adapter });
    } catch (e) {
      stop(verdictHere(e));
    }
  })();
  const draw = async ({ req, dir, limits }: Draw): Promise<Answer> => {
    let bundle: Bundle, camera: Camera;
    try {
      try { bundle = readBundle(dir, limits); } catch (e) { throw new LoadError(`artefact ${dir}: ${e instanceof Error ? e.message : String(e)}`); }
      try { check(bundle); } catch (e) { throw new LoadError(`artefact ${dir}: refused by --check: ${e instanceof Error ? e.message : String(e)}`); }
      checkParams(req.params, bundle.manifest);
      checkInputs(bundle.manifest, req.inputs, { kind: "none" }, "serve");
      camera = cameraFor(req.camera, bundle);
      // The TS host refuses `precision: "mixed"` on an f32-only artefact (LoadError); the Rust loader would draw f32.
      // Refuse it here, with the same check, so the engine switch does not change the answer. The device argument does
      // not matter for the refusal: only the ask and the artefact do.
      selectPrecision(bundle, true, req.precision);
    } catch (e) {
      return failed(req.id, verdictHere(e));
    }
    child.stdin.write(canonical({ dir, camera, times: req.times, size: [req.size.width, req.size.height], params: req.params, inputs: req.inputs, precision: req.precision ?? "mixed", out: req.out }) + "\n");
    child.stdin.flush();
    const r = await line<DrawReply>();
    if (!r.ok) return failed(req.id, r.verdict);
    const build = buildOf(bundle), view = (time: number): View => ({ camera, ...req.size, time, params: req.params, inputs: req.inputs, construction: false });
    const identities = req.times.map((time): Identity => ({ build, frame: frameOf(bundle.manifest, view(time), r.precision, { kind: "frame" }), device }));
    const { time: _, ...frame } = identities[0]!.frame;
    const frames = r.frames.map((f, k) => ({ time: req.times[k]!, ...f, id: identityId(identities[k]!) }));
    return { id: req.id, ok: true, identity: { build, frame, device }, frames, ms: r.ms };
  };
  return {
    ready,
    draw,
    async kill() {
      retired = true;
      child.kill("SIGKILL");
      await child.exited;
    },
  };
}

async function drawWorker(checkPath: string | undefined) {
  console.log = console.error;
  const send = (message: DrawMessage) => process.send!(message);
  let stopping = false;
  const stop = (verdict: Verdict) => {
    if (stopping) return;
    stopping = true;
    send({ kind: "stop", verdict });
  };
  process.on("disconnect", () => process.exit(0));
  try {
    const check: (b: Bundle) => void = checkPath === undefined ? () => {} : (await import(checkPath)).default;
    if (typeof check !== "function") throw new UsageError(`--check ${checkPath}: its default export is not a function`);

    let gpu: Gpu;
    const freshGpu = async (): Promise<Gpu> => {
      const device = await headlessDevice();
      let lost: string | undefined;
      const me: Gpu = { cache: cachingDevice(device), lost: () => lost, adapter: (device as GPUDevice & { adapterInfo?: GPUAdapterInfo }).adapterInfo, device: deviceOf(device) };
      // Only the current device's loss ends the server. A retry destroys the old one on purpose.
      void device.lost.then((info) => {
        lost = `device lost: ${info.reason}: ${info.message}`;
        if (gpu === me) stop({ kind: "environment", message: `${lost}; the server ends: start a new one` });
      });
      return me;
    };
    gpu = await freshGpu();

    /** Loads and draws one directory, retrying a transient GPU refusal on a fresh device as `render` does. */
    const draw = async ({ req, dir, limits }: Draw): Promise<Answer> => {
      let bundle: Bundle;
      try { bundle = readBundle(dir, limits); } catch (e) { throw new LoadError(`artefact ${dir}: ${e instanceof Error ? e.message : String(e)}`); }
      try { check(bundle); } catch (e) { throw new LoadError(`artefact ${dir}: refused by --check: ${e instanceof Error ? e.message : String(e)}`); }
      checkParams(req.params, bundle.manifest);
      checkInputs(bundle.manifest, req.inputs, { kind: "none" }, "serve");
      const camera = cameraFor(req.camera, bundle);
      const t0 = performance.now();
      for (let attempt = 1; ; attempt++) {
        try {
          const loaded = await gpu.cache.settle(() => loadBundle(gpu.cache.device, bundle, { kind: "none" }, req.precision));
          const t1 = performance.now();
          try {
            mkdirSync(req.out, { recursive: true });
            const frames = [];
            const build = buildOf(bundle);
            let shared: Shared | undefined;
            for (const [k, time] of req.times.entries()) {
              const view: View = { camera, ...req.size, time, params: req.params, inputs: req.inputs, construction: false };
              const pixels = await loaded.frame(view);
              const file = join(req.out, `${k}.rgba`);
              writeFileSync(file, pixels);
              const identity: Identity = { build, frame: frameOf(bundle.manifest, view, loaded.precision, { kind: "frame" }), device: gpu.device };
              const { time: _, ...frame } = identity.frame;
              shared ??= { ...identity, frame };
              frames.push({ time, file, sha256: createHash("sha256").update(pixels).digest("hex"), id: identityId(identity) });
            }
            return { id: req.id, ok: true, identity: shared!, frames, ms: { load: t1 - t0, frames: performance.now() - t1 }, cache: gpu.cache.counts() };
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

    process.on("message", async (work: Draw) => {
      let answer: Answer;
      try { answer = await draw(work); } catch (e) { answer = failed(work.req.id, verdictHere(e)); }
      if (stopping) return;
      const lost = gpu.lost();
      if (lost !== undefined) return stop({ kind: "environment", message: `${lost}; the server ends: start a new one` });
      const loaded = compilerModulesLoaded();
      if (loaded.length > 0) return stop({ kind: "internal", message: `loft serve loaded the compiler (${loaded.join(", ")}); the server ends`, stack: "" });
      if (gpu.cache.bound()) console.error("loft serve: the GPU object cache passed its bound and was emptied");
      send({ kind: "answer", answer });
    });
    const a = gpu.adapter;
    send({ kind: "ready", adapter: a === undefined ? null : { vendor: a.vendor, architecture: a.architecture, device: a.device, description: a.description } });
  } catch (e) {
    stop(verdictHere(e));
  }
}

// ---- supervise the protocol and compile children ------------------------------------------------------------

export async function serve(cmd: ServeCommand) {
  // The protocol owns stdout; anything the library prints goes to stderr.
  const write = (r: Answer | object) => process.stdout.write(JSON.stringify(r) + "\n");
  console.log = console.error;

  /** Requests admitted and not yet answered, by a token of their own (ids are the caller's and may repeat). */
  const open = new Map<object, string | number | null>();
  /** The `out` of each request in flight. */
  const outs = new Map<string, string | number>();
  const children = new Set<{ kill(signal?: number | NodeJS.Signals): void }>();
  /** Private build directories of the scenes in flight. */
  const builds = new Set<string>();
  let closing = false;
  let worker: ReturnType<typeof startDrawWorker> | undefined;
  /**
   * Ends the server: every request in flight is answered with `v` first, then the process exits with its code. A
   * device loss used to answer only the request that noticed it; the rest got end of file.
   */
  const closeWith = (v: Verdict) => {
    if (closing) return;
    closing = true;
    void worker?.kill();
    for (const c of children) c.kill("SIGKILL");
    // process.exit skips the `finally` that removes a build directory; remove the ones in flight here.
    for (const d of builds) rmSync(d, { recursive: true, force: true });
    const text = [...open.values()].map((id) => JSON.stringify(failed(id, v)) + "\n").join("");
    const code = ending(v).code;
    if (text === "") process.exit(code);
    process.stdout.write(text, () => process.exit(code));
  };

  // INTEGRATOR: classify LOFT_ENGINE once at the CLI boundary (plan §8.2) and pass it in; this is the draw child's switch.
  const engine = process.env.LOFT_ENGINE ?? "ts";
  if (engine !== "ts" && engine !== "rust") exitWith({ kind: "usage", message: `LOFT_ENGINE=${engine}: serve draws on "ts" or "rust" ("shadow" is for the corpus tools)` });
  const startDraw = engine === "rust" ? startRustDraw : startDrawWorker;
  worker = startDraw(cmd.check, closeWith);
  const boot = await worker.ready;
  if (boot.kind === "stop") {
    await worker.kill();
    exitWith(boot.verdict);
  }
  write({ ready: true, adapter: boot.adapter });

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

  const drawBounded = async (req: Request, dir: string): Promise<Answer> => {
    const deadline = performance.now() + cmd.drawSeconds * 1000;
    const budget = new Budget(`draw of request ${JSON.stringify(req.id)}: ran out of ${cmd.drawSeconds} s (--draw-seconds); request fewer or smaller frames, or raise --draw-seconds`);
    const checkTime = () => { if (performance.now() >= deadline) throw budget; };
    const current = worker ??= startDraw(cmd.check, closeWith);
    let timer: ReturnType<typeof setTimeout> | undefined;
    const timeout = new Promise<never>((_, reject) => {
      // Spawning a replacement can already have consumed a very short budget.
      timer = setTimeout(() => reject(budget), Math.max(1, Math.ceil(deadline - performance.now())));
    });
    const drawing = current.ready.then(async (boot) => {
      checkTime();
      if (boot.kind === "stop") return boot;
      const answer = await current.draw({ req, dir, limits: cmd.limits });
      return { kind: "answer", answer } as const;
    });
    try {
      const result = await Promise.race([drawing, timeout]);
      checkTime();
      if (result.kind === "answer") return result.answer;
      await current.kill();
      worker = undefined;
      return failed(req.id, result.verdict);
    } catch (e) {
      if (e === budget) {
        // Reap the child before releasing the slot and output reservation. A late frame cannot write to that path.
        await current.kill();
        worker = undefined;
      }
      throw e;
    } finally {
      clearTimeout(timer);
    }
  };

  /** Compiles (if a scene) and draws one request. A scene builds in a private directory, removed afterwards. */
  const run = async (req: Request): Promise<Answer> => {
    const source = req.source;
    switch (source.kind) {
      case "artefact": return await drawSlot(() => drawBounded(req, source.dir));
      case "scene": {
        const scene = source.path, dir = mkdtempSync(join(tmpdir(), "loft-serve-build-"));
        builds.add(dir);
        try {
          const compileMs = await compileSlot(async () => {
            const t0 = performance.now();
            await compile(cmd, scene, dir, req.precision ?? "f32", children);
            return performance.now() - t0;
          });
          const drawn = await drawSlot(() => drawBounded(req, dir));
          return drawn.ok ? { ...drawn, ms: { compile: compileMs, ...drawn.ms } } : drawn;
        } finally {
          rmSync(dir, { recursive: true, force: true });
          builds.delete(dir);
        }
      }
    }
  };

  const handle = async (line: string, token: object): Promise<void> => {
    let id: string | number | null = null;
    try { id = idOf(JSON.parse(line)); } catch { /* answered below as a usage error */ }
    open.set(token, id);
    let response: Answer;
    let held: string | undefined;
    try {
      const req = requestOf(line);
      // Reserved before the first await, so of two overlapping requests in flight the later is the one refused.
      for (const [o, other] of outs) {
        if (overlaps(o, req.out)) throw new UsageError(`request: "out" ${req.out} overlaps ${o}, the out of request ${JSON.stringify(other)}, still in flight; give each request in flight its own out`);
      }
      outs.set(held = req.out, req.id);
      response = await run(req);
    } catch (e) {
      response = e instanceof ChildVerdict ? { id, ...e.response } : failed(id, verdictHere(e));
    } finally {
      if (held !== undefined) outs.delete(held);
    }
    if (closing) return; // closeWith has answered this request
    // The supervisor also keeps the compiler out. The GPU worker checks its own modules before each answer.
    const loadedNow = compilerModulesLoaded();
    if (loadedNow.length > 0) return closeWith({ kind: "internal", message: `loft serve loaded the compiler (${loadedNow.join(", ")}); the server ends`, stack: "" });
    write(response);
    open.delete(token);
  };

  const inflight = new Set<Promise<void>>();
  for await (const line of console) {
    if (closing) break;
    if (line.trim() === "") continue;
    if (open.size >= cmd.pending) {
      let id: string | number | null = null;
      try { id = idOf(JSON.parse(line)); } catch { /* answered with no id */ }
      write(failed(id, { kind: "budget", message: `serve: ${open.size} requests already pending (--pending ${cmd.pending}); wait for an answer before sending more, or raise --pending` }));
      continue;
    }
    const token = {};
    const p = handle(line, token).finally(() => inflight.delete(p));
    inflight.add(p);
  }
  await Promise.all(inflight);
  if (!closing) {
    await worker?.kill();
    process.exit(0);
  }
}

if (import.meta.main) await drawWorker(Bun.argv[2]);
