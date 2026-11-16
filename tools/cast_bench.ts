// castRay's cost on the largest example: the crab (examples/bosses/reefback_titan.ts, base variant) lit by a lamp over
// it at [3, 14, 2.5] whose light function casts one shadow ray per lit fragment, against the same crab unlit, and
// against other checkouts' castRay (`base`, or a directory: e.g. `git archive <ref> | tar -x -C out/base_cast`,
// node_modules linked). Each scene is compiled by its own checkout's modules, so several castRay versions run in one
// process, interleaved frame by frame (`frameGpu`:
// the pose stages, where a per-frame table is built, plus the draws). Prints each frame's rgba hash, then per scene
// the median, quartiles and best of the per-frame GPU times, and the load average before and after.
//
//   bun tools/cast_bench.ts [reps] [base-dir] [view] [runs]
// runs: comma-separated <checkout>:<light>, checkout here | base | a directory, light none | lamp | hard | soft
// (default here:none,base:soft,here:soft,base:hard,here:hard). Paired differences are against the first run.
import { createHash } from "node:crypto";
import { loadavg } from "node:os";

const reps = Number(process.argv[2] ?? 60);
const baseDir = process.argv[3] ?? "out/base_cast";
const viewName = process.argv[4] ?? "game";
const which = (process.argv[5] ?? "here:none,base:soft,here:soft,base:hard,here:hard").split(",").map((r) => r.split(":") as [string, Light]);
const W = 1280, H = 800;

type Mods = { cast: any; index: any; compile: any; variant: any; host: any; headless: any };
async function mods(root: string): Promise<Mods> {
  const at = (p: string) => import(new URL(p, `file://${root}/`).href);
  return { index: await at("src/index.ts"), compile: await at("src/compile/index.ts"), variant: await at("src/cli/variant.ts"), host: await at("src/host/host.ts"), headless: await at("src/host/headless.ts"), cast: undefined };
}

/** The crab from `root`, lit by a lamp over it whose light casts its own soft shadow (a width of 0.05 per unit of t). */
type Light = "none" | "lamp" | "hard" | "soft";
async function crab(m: Mods, root: string, light: Light): Promise<any> {
  const { scene } = await m.variant.chosenAt(`${root}/examples/bosses/reefback_titan.ts`, { kind: "default" });
  if (light === "none") return scene;
  const { castRay, lamp, f, mul, smoothstep, div } = m.index;
  const bulb = lamp([3, 14, 2.5], { colour: [1.2, 1.0, 0.8], reach: 12 });
  // hard: a ray of a hair's width (examples/light/area_lamp.ts); soft: a width of 0.05 per unit of t (a lamp's penumbra).
  const width = (t: any) => (light === "hard" ? f(0.002) : mul(t, 0.05));
  const shaded = (p: any, xi: any) => {
    const a = bulb(p, xi);
    if (light === "lamp") return a;
    const hit = castRay({ o: p, d: a.toward, t0: f(0.05), t1: a.distance, width });
    return { ...a, amount: mul(a.amount, smoothstep(0, 1, div(hit.gap, width(hit.t)))) };
  };
  return { ...scene, lighting: { ...scene.lighting, lights: [...scene.lighting.lights, shaded] } };
}

const here = process.cwd(), base = `${here}/${baseDir}`;
const rootOf = (where: string): string => (where === "here" ? here : where === "base" ? base : `${here}/${where}`);
const loadedMods = new Map<string, Mods>();
for (const [where] of which) if (!loadedMods.has(rootOf(where))) loadedMods.set(rootOf(where), await mods(rootOf(where)));
const device = await (await mods(here)).headless.headlessDevice();
const runs: { name: string; loaded: any }[] = [];
for (const [where, light] of which) {
  const name = `${where}:${light}`, root = rootOf(where), m = loadedMods.get(root)!;
  const t0 = performance.now();
  const a = m.compile.compileScene(await crab(m, root, light)).artefact;
  const loaded = await m.host.createHost(device).load(a);
  const view = { camera: a.cameras[viewName], width: W, height: H, time: 0 };
  const px = await loaded.frame(view);
  console.log(`${name.padEnd(16)} compile+load ${(performance.now() - t0).toFixed(0)} ms  rgba ${createHash("sha256").update(px).digest("hex").slice(0, 16)}`);
  runs.push({ name, loaded: Object.assign(loaded, { view }) });
}
console.log(`load average before: ${loadavg().map((x) => x.toFixed(1)).join(" ")}`);
const ms = runs.map(() => [] as number[]);
for (let r = 0; r < reps; r++) for (const [k, run] of runs.entries()) ms[k]!.push((await run.loaded.frameGpu(run.loaded.view, 1)).ms);
console.log(`load average after:  ${loadavg().map((x) => x.toFixed(1)).join(" ")}`);
const q = (xs: number[], p: number) => { const s = [...xs].sort((a, b) => a - b); return s[Math.min(s.length - 1, Math.floor(p * s.length))]!; };
console.log(`GPU ms per frame over ${reps} interleaved frames (${viewName}, ${W}x${H}):   median   q25    q75    best`);
for (const [k, run] of runs.entries()) console.log(`  ${run.name.padEnd(16)} ${[0.5, 0.25, 0.75, 0].map((p) => q(ms[k]!, p).toFixed(2).padStart(6)).join(" ")}`);
// Paired differences: each round's frame less the first run's frame of the same round.
for (let k = 1; k < runs.length; k++) {
  const d = ms[k]!.map((x, r) => x - ms[0]![r]!);
  console.log(`  ${runs[k]!.name.padEnd(16)} - ${runs[0]!.name}, paired: median ${q(d, 0.5).toFixed(2)}  q25 ${q(d, 0.25).toFixed(2)}  q75 ${q(d, 0.75).toFixed(2)}`);
}
for (const run of runs) run.loaded.destroy();
process.exit(0);
