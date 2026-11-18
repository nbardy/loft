// loft CLI. Dispatch only: each command is one function below.
import { createHash } from "node:crypto";
import { loopTime, OPEN, type Loop } from "../artefact.ts";
import type { Artefact, View } from "../artefact.ts";
import { baked } from "./state.ts";
import type { Bundle, Precision } from "../host/manifest.ts";
import type { GpuLoaded } from "../host/host.ts";
import type { StillSpec } from "../host/still.ts";
import { buildOf, deviceOf, frameOf, identityId, type Identity } from "../identity.ts";
import { parseArgs, USAGE, UsageError, type Command } from "./args.ts";
import { resolveCamera } from "./cameras.ts";
import { contactSheet, writeWebp, type Frame } from "./image.ts";
import { checkInputs, viewInputs, type Sweep } from "./inputs.ts";
import { loadArtefact } from "./load.ts";
import { openVideo } from "./video.ts";
import { exitWith, headed, verdictOf } from "./verdict.ts";
import { EnvironmentError } from "../errors.ts";
import { passNotes } from "./pass_notes.ts";
import { drawFrame } from "./still.ts";

const ms = (t0: number) => `${(performance.now() - t0).toFixed(1)} ms`;

/** What the CLI reads of an artefact, whichever way it came: compiled here, or a built directory. */
type Info = Pick<Artefact, "cameras" | "inputs" | "stats">;

/**
 * A loaded scene and what its frames' identities need (src/identity.ts): the bundle drawn and the device. A scene
 * compiled here is lowered once, here, so its bundle is the one drawn (createHost's `load` would lower it again).
 */
type Opened = { readonly info: Info; readonly loaded: GpuLoaded; readonly bundle: Bundle; readonly identity: Omit<Identity, "frame"> };

async function open(cmd: Extract<Command, { kind: "render" | "frames" | "clip" }>, sweep: Sweep): Promise<Opened> {
  let t = performance.now();
  switch (cmd.source.kind) {
    case "artefact": {
      // Without the compiler: the loader reads the directory alone (src/cli/run_files.ts's closure).
      const [{ headlessDevice }, { loadArtefactFiles }] = await Promise.all([import("../host/headless.ts"), import("../host/files.ts")]);
      const device = await headlessDevice();
      const { bundle, loaded } = await loadArtefactFiles(device, cmd.source.dir, cmd.precision);
      const m = bundle.manifest;
      checkInputs(m, cmd.inputs, sweep);
      console.log(`files    ${ms(t)}  read and loaded ${cmd.source.dir}  (${fillCounts(m.tables)}; ${m.modules.length} modules, ${m.renderPipelines.length + m.computePipelines.length} pipelines)`);
      console.log(`precision ${loaded.precision}`);
      return identified({ info: m, loaded, bundle }, device);
    }
    case "file": case "fixture": {
      const artefact = await loadArtefact(cmd.source, cmd.still);
      checkInputs(artefact, cmd.inputs, sweep);
      console.log(`compile  ${ms(t)}  (at most ${artefact.stats.maxTriangles} triangles, ${artefact.stats.draws} draws)`);
      for (const line of passNotes(artefact)) console.log(line);
      t = performance.now();
      const { loaded, bundle, device } = await loadedWithRetries(artefact, cmd.precision);
      console.log(`load     ${ms(t)}  precision ${loaded.precision}`);
      for (const r of artefact.recurrences ?? []) console.log(baked(r));
      return identified({ info: artefact, loaded, bundle }, device);
    }
  }
}

/** Prints the build and device parts once; each frame's line adds its `id` (`timed`). */
function identified(o: Omit<Opened, "identity">, device: GPUDevice): Opened {
  const identity = { build: buildOf(o.bundle), device: deviceOf(device) };
  const p = identity.build.program;
  const js = p.kind === "js-tables" ? `  (+ tables filled by JavaScript, not in the program hash: ${p.tables.join(", ")})` : "";
  console.log(`identity program ${p.sha256.slice(0, 16)}  library ${identity.build.library.slice(0, 16)}  device ${identity.device.adapter}${js}`);
  return { ...o, identity };
}

/** A frame's id (src/identity.ts), short, as printed beside its rgba hash. */
function frameId(o: Opened, view: View, still: StillSpec): string {
  return identityId({ ...o.identity, frame: frameOf(o.bundle.manifest, view, o.loaded.precision, still) }).slice(0, 16);
}

/**
 * The artefact on a fresh device, tried up to three times: a shader or pipeline that fails to build from WGSL the
 * compiler wrote is the environment's (until 2026-11-12 it was bun-webgpu freeing descriptor strings early, now
 * patched: docs/reliable_render.md), and a fresh device is what can make it pass.
 */
async function loadedWithRetries(artefact: Artefact, precision?: Precision): Promise<{ loaded: GpuLoaded; bundle: Bundle; device: GPUDevice }> {
  const [{ headlessDevice }, { bakeRecurrences }, { lower }, { loadBundle }] = await Promise.all([
    import("../host/headless.ts"), import("../host/bake.ts"), import("../host/lower.ts"), import("../host/host.ts"),
  ]);
  const seen: string[] = [];
  for (let attempt = 1; ; attempt++) {
    try {
      // What createHost's `load` does (src/host/host.ts), keeping the bundle: bake the recurrences on this device, lower, load.
      const device = await headlessDevice();
      await bakeRecurrences(device, artefact);
      const bundle = lower(artefact, precision);
      return { loaded: await loadBundle(device, bundle, { kind: "js", uploads: (state) => artefact.cpuPose(state) }, precision), bundle, device };
    } catch (e) {
      if (!(e instanceof EnvironmentError && e.transient)) throw e;
      seen.push(headed(e.message)[0]);
      console.error(`loft: load attempt ${attempt} failed in the GPU (not the scene): ${seen.at(-1)}`);
      if (attempt === 3) {
        const same = seen.every((m) => m === seen[0]);
        throw new EnvironmentError(`${e.message}\nthe GPU refused the compiled scene on 3 fresh devices${same ? ", the same way each time: likely a bug in Loft's compiler, worth reporting" : ", differently each time"}`, false);
      }
    }
  }
}

/** The loop a drawn thing declares: a compiled scene's own; a built directory's manifest carries none yet, so its clock runs on. */
function loopOf(info: object): Loop {
  const l = (info as { readonly loop?: Loop }).loop;
  return l === undefined ? OPEN : l;
}

/** "n map, n scan, n cpu (not data)": how the per-frame tables are filled. */
function fillCounts(tables: readonly { readonly fill: { readonly kind: "map" | "scan" | "cpu" | "replay" } }[]): string {
  const n = { map: 0, scan: 0, cpu: 0, replay: 0 };
  for (const t of tables) n[t.fill.kind]++;
  return `tables ${n.map} map, ${n.scan} scan, ${n.cpu} cpu (not data)${n.replay > 0 ? `, ${n.replay} replayed` : ""}`;
}

/** Draws one frame and prints its line: time, what was drawn, the RGBA's hash and the frame's id. */
async function timed(o: Opened, label: string, view: View, still: StillSpec): Promise<Uint8Array> {
  const t = performance.now();
  const r = await drawFrame(o.loaded, view, still);
  // The hash names the exact RGBA, so two ways of drawing a frame can be compared without keeping a lossless picture.
  const sha = createHash("sha256").update(r.pixels).digest("hex").slice(0, 16);
  console.log(`frame    ${ms(t)}  ${label}  (${r.drawn.triangles} triangles drawn, ${r.drawn.draws} draw calls)  rgba ${sha}  id ${frameId(o, view, still)}`);
  return r.pixels;
}

/** A files render must not have loaded the compiler: checked at the end, from what this process actually loaded. */
async function witness(cmd: Extract<Command, { kind: "render" | "frames" | "clip" }>): Promise<void> {
  if (cmd.source.kind !== "artefact") return;
  const loadedNow = (await import("./run_files.ts")).compilerModulesLoaded();
  if (loadedNow.length > 0) throw new Error(`--artefact: the compiler was loaded: ${loadedNow.join(", ")}`);
  console.log("compiler modules loaded: 0");
}

async function render(cmd: Extract<Command, { kind: "render" }>) {
  const opened = await open(cmd, { kind: "none" });
  const { info: artefact, loaded } = opened;
  const frames: Frame[] = [];
  for (const name of cmd.views) {
    const camera = resolveCamera(artefact, name);
    const rgba = await timed(opened, name, { camera, ...cmd.size, time: loopTime(loopOf(artefact), cmd.time), params: cmd.params, inputs: cmd.inputs, construction: cmd.construction }, cmd.still);
    frames.push({ ...cmd.size, rgba });
  }
  loaded.destroy();
  const sheet = contactSheet(frames);
  writeWebp(cmd.out, sheet);
  console.log(`wrote ${cmd.out}  ${sheet.width}x${sheet.height}  [${cmd.views.join(", ")}]`);
  await witness(cmd);
}

async function frames(cmd: Extract<Command, { kind: "frames" }>) {
  const opened = await open(cmd, cmd.sweep);
  const { info: artefact, loaded } = opened;
  const camera = resolveCamera(artefact, cmd.view);
  const out: Frame[] = [];
  for (const [k, time] of cmd.times.entries()) {
    const inputs = viewInputs(cmd.inputs, cmd.sweep, k, cmd.times.length);
    const label = cmd.sweep.kind === "linear" ? `t=${time} ${cmd.sweep.name}=${inputs[cmd.sweep.name]!.map((x) => +x.toFixed(4)).join(",")}` : `t=${time}`;
    const rgba = await timed(opened, label, { camera, ...cmd.size, time: loopTime(loopOf(artefact), time), params: cmd.params, inputs }, cmd.still);
    out.push({ ...cmd.size, rgba });
  }
  loaded.destroy();
  const sheet = contactSheet(out);
  writeWebp(cmd.out, sheet);
  console.log(`wrote ${cmd.out}  ${sheet.width}x${sheet.height}  times [${cmd.times.join(", ")}]`);
  await witness(cmd);
}

/**
 * The shortest span a scene's baked state keeps, as (name, seconds it holds from): a compiled scene's recurrences, or a
 * built directory's replays (one that keeps every state carries no record of it in the file). None for a loop: its
 * clock comes round inside the span (the compiler refuses a looping scene whose state span does not cover it).
 */
function heldFrom(info: object): { readonly name: string; readonly until: number } | undefined {
  if (loopOf(info).kind === "every") return undefined;
  // A still state (kept from one step: settled before the scene) holds by design, never refuses.
  const rs = (info as Partial<Artefact>).recurrences?.filter((r) => r.steps > 0).map((r) => ({ name: r.name, until: r.start + r.steps / r.rate }))
    ?? (info as { readonly frame?: { readonly replay?: readonly { label: string; start: number; steps: number; rate: number }[] } }).frame?.replay?.map((r) => ({ name: r.label, until: r.start + r.steps / r.rate }))
    ?? [];
  return rs.reduce<{ name: string; until: number } | undefined>((m, r) => (m === undefined || r.until < m.until ? r : m), undefined);
}

async function clip(cmd: Extract<Command, { kind: "clip" }>) {
  const { info: artefact, loaded } = await open(cmd, cmd.sweep);
  const held = heldFrom(artefact);
  if (held !== undefined && cmd.seconds > held.until + 1e-9) {
    loaded.destroy();
    throw new UsageError(`clip: ${cmd.seconds} s, but the state "${held.name}" is kept only to ${held.until} s and holds still after it. Fix: --seconds ${held.until}, or a later \`until\` in its recur()`);
  }
  const camera = resolveCamera(artefact, cmd.view);
  const count = Math.round(cmd.seconds * cmd.fps);
  const video = openVideo(cmd.out, cmd.size.width, cmd.size.height, cmd.fps);
  const t0 = performance.now();
  for (let i = 0; i < count; i++) {
    const rgba = (await drawFrame(loaded, { camera, ...cmd.size, time: loopTime(loopOf(artefact), i / cmd.fps), params: cmd.params, inputs: viewInputs(cmd.inputs, cmd.sweep, i, count) }, cmd.still)).pixels;
    await video.write(rgba);
  }
  await video.finish();
  loaded.destroy();
  console.log(`frame    ${((performance.now() - t0) / count).toFixed(1)} ms mean over ${count} frames (render + encode)`);
  console.log(`wrote ${cmd.out}  ${count} frames at ${cmd.fps} fps`);
  await witness(cmd);
}

/** `loft build`: compile, bake (a scene with `recur`, on this GPU), lower, write the directory (`writeBundle` refuses `cpu` tables by name). */
async function build(cmd: Extract<Command, { kind: "build" }>) {
  let t = performance.now();
  const artefact = await loadArtefact(cmd.source);
  console.log(`compile  ${ms(t)}  ${fillCounts(artefact.tables)}`);
  for (const line of passNotes(artefact)) console.log(line);
  if (artefact.recurrences !== undefined) {
    // The files carry baked states (a file loader never bakes): bake on this machine's GPU first.
    const [{ headlessDevice }, { bakeRecurrences }] = await Promise.all([import("../host/headless.ts"), import("../host/bake.ts")]);
    await bakeRecurrences(await headlessDevice(), artefact);
    for (const r of artefact.recurrences) console.log(baked(r));
  }
  t = performance.now();
  const [{ lower }, { writeBundle }] = await Promise.all([import("../host/lower.ts"), import("../host/files.ts")]);
  const emitted = lower(artefact, cmd.precision);
  let bundle = emitted;
  if (emitted.manifest.build !== undefined && process.env.LOFT_NO_GPU !== "1") {
    const [{ headlessDevice }, { bakeRawBuild }] = await Promise.all([import("../host/headless.ts"), import("../host/host.ts")]);
    const device = await headlessDevice();
    const started = performance.now();
    try {
      const data = await bakeRawBuild(device, emitted);
      bundle = lower(artefact, cmd.precision, data);
      console.log(`compute build  ${ms(started)}  ${[...data.values()].reduce((n, bytes) => n + bytes.byteLength, 0)} bytes`);
    } finally { device.destroy(); }
  }
  const sizes = writeBundle(cmd.out, bundle);
  const m = bundle.manifest;
  const kb = (n: number) => `${(n / 1024).toFixed(1)} KB`;
  console.log(`lower    ${ms(t)}  ${m.modules.length} modules from ${m.chunks.length} chunks, ${m.renderPipelines.length} render and ${m.computePipelines.length} compute pipelines, ${m.frame.passes.length} passes`);
  console.log(`wrote ${cmd.out}  ${sizes.files} files: wgsl ${kb(sizes.wgsl)}, bin ${kb(sizes.bin)}, manifest ${kb(sizes.manifest)}`);
}

async function main(argv: readonly string[]) {
  const cmd = parseArgs(argv);
  switch (cmd.kind) {
    case "render": return render(cmd);
    case "frames": return frames(cmd);
    case "clip": return clip(cmd);
    case "dev": return (await import("./dev.ts")).dev(cmd);
    case "bake": return (await import("../bake/command.ts")).bake(cmd);
    case "build": return build(cmd);
    case "check": return (await import("./check.ts")).check(cmd);
    case "gallery": return (await import("./gallery.ts")).gallery(cmd);
    case "serve": return (await import("./serve.ts")).serve(cmd);
    case "profile": return (await import("./profile.ts")).profile(cmd);
    case "variants": return (await import("./variants.ts")).variants(cmd);
    case "help": return void console.log(USAGE);
    case "api": {
      const api = await import("./api.ts");
      try {
        return void console.log(cmd.name.kind === "all" ? api.apiFull() : api.apiOne(cmd.name.name));
      } catch (e) {
        if (e instanceof api.ApiError) throw new UsageError(`api: ${e.message}`);
        throw e;
      }
    }
    case "context": return void console.log((await import("./context.ts")).context());
  }
}

try {
  await main(process.argv.slice(2));
} catch (e) {
  exitWith(await verdictOf(e));
}
