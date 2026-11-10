// loft CLI. Dispatch only: each command is one function below.
import type { Loaded } from "../artefact.ts";
import { parseArgs, USAGE, UsageError, type Command } from "./args.ts";
import { resolveCamera } from "./cameras.ts";
import { contactSheet, writeWebp, type Frame } from "./image.ts";
import { headlessHost, loadArtefact } from "./load.ts";
import { openVideo } from "./video.ts";

const ms = (t0: number) => `${(performance.now() - t0).toFixed(1)} ms`;

async function open(cmd: Exclude<Command, { kind: "dev" }>) {
  let t = performance.now();
  const artefact = await loadArtefact(cmd.source);
  console.log(`compile  ${ms(t)}  (${artefact.stats.triangles} triangles, ${artefact.stats.draws} draws)`);
  t = performance.now();
  const loaded: Loaded = await (await headlessHost()).load(artefact);
  console.log(`load     ${ms(t)}`);
  return { artefact, loaded };
}

async function timed(label: string, draw: () => Promise<Uint8Array>): Promise<Uint8Array> {
  const t = performance.now();
  const rgba = await draw();
  console.log(`frame    ${ms(t)}  ${label}`);
  return rgba;
}

async function render(cmd: Extract<Command, { kind: "render" }>) {
  const { artefact, loaded } = await open(cmd);
  const frames: Frame[] = [];
  for (const name of cmd.views) {
    const camera = resolveCamera(artefact, name);
    const rgba = await timed(name, () => loaded.frame({ camera, ...cmd.size, time: cmd.time, params: cmd.params, construction: cmd.construction }));
    frames.push({ ...cmd.size, rgba });
  }
  loaded.destroy();
  const sheet = contactSheet(frames);
  writeWebp(cmd.out, sheet);
  console.log(`wrote ${cmd.out}  ${sheet.width}x${sheet.height}  [${cmd.views.join(", ")}]`);
}

async function frames(cmd: Extract<Command, { kind: "frames" }>) {
  const { artefact, loaded } = await open(cmd);
  const camera = resolveCamera(artefact, cmd.view);
  const out: Frame[] = [];
  for (const time of cmd.times) {
    const rgba = await timed(`t=${time}`, () => loaded.frame({ camera, ...cmd.size, time, params: cmd.params }));
    out.push({ ...cmd.size, rgba });
  }
  loaded.destroy();
  const sheet = contactSheet(out);
  writeWebp(cmd.out, sheet);
  console.log(`wrote ${cmd.out}  ${sheet.width}x${sheet.height}  times [${cmd.times.join(", ")}]`);
}

async function clip(cmd: Extract<Command, { kind: "clip" }>) {
  const { artefact, loaded } = await open(cmd);
  const camera = resolveCamera(artefact, cmd.view);
  const count = Math.round(cmd.seconds * cmd.fps);
  const video = openVideo(cmd.out, cmd.size.width, cmd.size.height, cmd.fps);
  const t0 = performance.now();
  for (let i = 0; i < count; i++) {
    const rgba = await loaded.frame({ camera, ...cmd.size, time: i / cmd.fps, params: cmd.params });
    await video.write(rgba);
  }
  await video.finish();
  loaded.destroy();
  console.log(`frame    ${((performance.now() - t0) / count).toFixed(1)} ms mean over ${count} frames (render + encode)`);
  console.log(`wrote ${cmd.out}  ${count} frames at ${cmd.fps} fps`);
}

async function main(argv: readonly string[]) {
  const cmd = parseArgs(argv);
  switch (cmd.kind) {
    case "render": return render(cmd);
    case "frames": return frames(cmd);
    case "clip": return clip(cmd);
    case "dev": return (await import("./dev.ts")).dev(cmd);
  }
}

try {
  await main(process.argv.slice(2));
} catch (e) {
  if (e instanceof UsageError) {
    console.error(`loft: ${e.message}\n${USAGE}`);
    process.exit(2);
  }
  console.error(`loft: ${e instanceof Error ? (e.stack ?? e.message) : String(e)}`);
  process.exit(1);
}
