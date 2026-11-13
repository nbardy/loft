// Where the time of one `loft frames` goes, stage by stage, in one process: the library's import, the scene module's
// import, the compile, the device, WGSL modules, pipelines, each frame (pose, encode, GPU plus readback) and the PNG
// and WebP encode. The device's create calls are wrapped to time shader modules and pipelines apart.
//
//   bun tools/render_stages.ts <scene.ts> [--view three] [--size 512x512] [--times 0,0.37,0.91,1.53] [--again]
//
// --again loads and draws the same scene a second time on the same device (what a batch of scenes in one process
// pays after the first: the warm module cache, and Dawn's in-process pipeline cache). Prints JSON on the last line.
export {};
const t0 = performance.now();
const argv = process.argv.slice(2);
const flag = (name: string, def: string) => { const i = argv.indexOf(name); return i < 0 ? def : argv[i + 1]!; };
const path = argv[0]!;
const view = flag("--view", "three");
const [width, height] = flag("--size", "512x512").split("x").map(Number) as [number, number];
const times = flag("--times", "0,0.37,0.91,1.53").split(",").map(Number);
const again = argv.includes("--again");

const ms: Record<string, number> = {};
const lap = async <T>(name: string, f: () => Promise<T> | T): Promise<T> => {
  const t = performance.now();
  const r = await f();
  ms[name] = (ms[name] ?? 0) + performance.now() - t;
  return r;
};

const lib = await lap("import library", () => import("../src/index.ts"));
const { chosenAt } = await lap("import library", () => import("../src/cli/variant.ts"));
const { resolveCamera } = await import("../src/cli/cameras.ts");
const { loopTime, OPEN } = await import("../src/artefact.ts");
const { contactSheet, encodePng } = await import("../src/cli/image.ts");
const { headlessDevice } = await lap("import library", () => import("../src/host/headless.ts"));
const { createHost } = await lap("import library", () => import("../src/host/index.ts"));
const { scene } = await lap("import scene", () => chosenAt(path, { kind: "default" }));
const artefact = await lap("compile", () => lib.compile(scene as Parameters<typeof lib.compile>[0]));
const device = await lap("device", () => headlessDevice());

// Wrap the create calls the host makes at load, to split shader modules from pipelines.
const timed = <K extends "createShaderModule" | "createRenderPipelineAsync" | "createComputePipelineAsync">(k: K, label: string) => {
  const orig = (device[k] as (...a: unknown[]) => unknown).bind(device);
  (device as unknown as Record<string, unknown>)[k] = (...a: unknown[]) => {
    const t = performance.now();
    const r = orig(...a);
    if (r instanceof Promise) return r.finally(() => { ms[label] = (ms[label] ?? 0) + performance.now() - t; });
    ms[label] = (ms[label] ?? 0) + performance.now() - t;
    return r;
  };
};
timed("createShaderModule", "wgsl modules");
timed("createRenderPipelineAsync", "render pipelines");
timed("createComputePipelineAsync", "compute pipelines");

async function drawOnce(prefix: string) {
  const loaded = await lap(`${prefix}load (all)`, () => createHost(device).load(artefact));
  const camera = resolveCamera(artefact, view);
  const frames = [];
  for (const t of times) {
    const r = await lap(`${prefix}frames (wall)`, () => loaded.frameTimed({ camera, width, height, time: loopTime(artefact.loop ?? OPEN, t) }));
    ms[`${prefix}frame pose`] = (ms[`${prefix}frame pose`] ?? 0) + r.ms.pose;
    ms[`${prefix}frame encode`] = (ms[`${prefix}frame encode`] ?? 0) + r.ms.encode;
    ms[`${prefix}frame gpu+readback`] = (ms[`${prefix}frame gpu+readback`] ?? 0) + r.ms.readback;
    frames.push({ width, height, rgba: r.pixels });
  }
  loaded.destroy();
  return frames;
}

const frames = await drawOnce("");
const sheet = contactSheet(frames);
const png = await lap("png encode (sheet)", () => encodePng(sheet));
const tmp = `out/render_stages_${process.pid}.png`;
await Bun.write(tmp, png);
await lap("cwebp", () => { Bun.spawnSync(["cwebp", "-quiet", "-q", "90", tmp, "-o", `${tmp}.webp`]); });
await lap("png encode (4 frames)", () => { for (const f of frames) encodePng(f); });
if (again) {
  for (const k of ["wgsl modules", "render pipelines", "compute pipelines"]) ms[`first ${k}`] = ms[k]!, ms[k] = 0;
  await drawOnce("again ");
}
ms["in-process total"] = performance.now() - t0;
const round = Object.fromEntries(Object.entries(ms).map(([k, v]) => [k, +v.toFixed(1)]));
console.log(`${artefact.stats.maxTriangles} triangles, ${artefact.stats.draws} draws, ${artefact.shaders ? Object.keys(artefact.shaders).length : 0} shaders`);
console.log(JSON.stringify({ scene: path, triangles: artefact.stats.maxTriangles, draws: artefact.stats.draws, ms: round }));
process.exit(0);
