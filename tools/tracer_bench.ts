// Pictures and best-case cost of the traced examples (coral, nest, pearls), per camera, at 1280x800.
//
// Cost is "tracer only": the GPU's own time (timestamp queries, tools/gpu_time.ts) for a render pass holding only the
// traced draws, best of `frames` (the machine is shared: only the minimum says what the work costs, and wall-clock
// frame differences moved by milliseconds with the load). Pictures are the whole scene through the host, written as
// raw RGBA (for differencing) and WebP (for looking) under `out/tracer/<tag>/`.
//
// `bun tools/tracer_bench.ts <tag> [frames] [scene,scene] [schedule]`; schedule overrides every draw.field's
// (trace | brick | mesh | auto). Cameras: the scene's own, plus `far` (the overview `three` at twice its distance).
import { mkdirSync, writeFileSync } from "node:fs";
import type { Artefact, Camera } from "../src/artefact.ts";
import { compile, type Scene } from "../src/index.ts";
import { setupGlobals } from "bun-webgpu";
import { createHost } from "../src/host/index.ts";
import { gpuTimer } from "./gpu_time.ts";
import { writeWebp } from "../src/cli/image.ts";
import { FieldDrawable, type FieldSchedule } from "../src/draw/field.ts";
import coral from "../examples/coral.ts";
import nest from "../examples/nest.ts";
import pearls from "../examples/pearls.ts";

const tag = process.argv[2] ?? "now";
const N = Number(process.argv[3] ?? 20);
const which = (process.argv[4] ?? "coral,nest,pearls").split(",");
const schedule = process.argv[5] as FieldSchedule | undefined;
const W = 1280, H = 800;
const SCENES: Record<string, Scene> = { coral, nest, pearls };
const TRACED = new Set(["coral", "nest", "pearls"]);

/** The scene with every field drawn on the given schedule. */
const withSchedule = (s: Scene, k: FieldSchedule | undefined): Scene =>
  k === undefined ? s : { ...s, things: s.things.map((t) => (t instanceof FieldDrawable ? new FieldDrawable(t.name, t.field, { ...t.o, schedule: k }) : t)) };

const far = (c: Camera): Camera => {
  if (c.kind !== "perspective") throw new Error("far: perspective cameras only");
  const e = c.eye.map((x, i) => c.target[i]! + 2 * (x - c.target[i]!)) as unknown as Camera["eye"];
  return { ...c, eye: e };
};

await setupGlobals();
const device = await (await navigator.gpu.requestAdapter())!.requestDevice({ requiredFeatures: ["timestamp-query"] });
const host = createHost(device);
const dir = `out/tracer/${tag}`;
mkdirSync(dir, { recursive: true });
for (const name of which) {
  const t0 = performance.now();
  const a: Artefact = compile(withSchedule(SCENES[name]!, schedule));
  const compileMs = performance.now() - t0;
  const statics = (a as { statics?: readonly { data: Float32Array }[] }).statics ?? [];
  const staticMB = statics.reduce((n, s) => n + s.data.byteLength, 0) / 2 ** 20;
  console.log(`${name}: compile ${compileMs.toFixed(0)} ms, static tables ${staticMB.toFixed(2)} MB, ${a.stats.triangles} triangles`);
  const full = await host.load(a);
  const timer = await gpuTimer(device, a, (label) => TRACED.has(label), W, H);
  const cams: [string, Camera][] = [...Object.entries(a.cameras), ["far", far(a.cameras.three!)]];
  for (const [cn, camera] of cams) {
    const px = await full.frame({ camera, width: W, height: H, time: 0 });
    writeFileSync(`${dir}/${name}_${cn}.rgba`, px);
    writeWebp(`${dir}/${name}_${cn}.webp`, { width: W, height: H, rgba: px });
    console.log(`  ${cn.padEnd(6)} tracer ${(await timer.time(camera, N)).toFixed(2)} ms`);
  }
  timer.destroy();
  full.destroy();
}
process.exit(0);
