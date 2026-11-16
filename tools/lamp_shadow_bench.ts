// A lamp's shadow map (DESIGN.md 17) against castRay on the crab (examples/bosses/reefback_titan.ts, base variant),
// lit by a lamp at [3, 14, 2.5] over it: the crab unlit by it (none), the lamp unshadowed (lamp), castRay's soft ray
// (width 0.05 per unit of t, as tools/cast_bench.ts on build/cast-grid), the map with the matching size (0.65: 0.05 x
// the lamp's distance of about 13) and the map hard (size 0). One process, frames interleaved run by run (`frameGpu`:
// pose stages plus draws and passes), paired against `none`; the load average before and after. With an output
// directory it writes each run's frame as WebP and prints the share of pixels whose luminance (0..1) differs by more
// than 0.1 between castRay and the map.
//
//   bun tools/lamp_shadow_bench.ts [reps] [view] [out-dir]
import { createHash } from "node:crypto";
import { loadavg } from "node:os";
import { chosenAt } from "../src/cli/variant.ts";
import { writeWebp } from "../src/cli/image.ts";
import { compileScene } from "../src/compile/index.ts";
import { createHost } from "../src/host/host.ts";
import { headlessDevice } from "../src/host/headless.ts";
import { castRay, div, f, lamp, mul, smoothstep, type Light } from "../src/index.ts";

const reps = Number(process.argv[2] ?? 40);
const viewName = process.argv[3] ?? "game";
const outDir = process.argv[4];
const W = 1280, H = 800, AT = [3, 14, 2.5] as const, COLOUR = [1.2, 1.0, 0.8] as const, REACH = 12;

type Run = "none" | "lamp" | "castRay" | "map" | "mapHard";
function lightOf(run: Run): Light | undefined {
  switch (run) {
    case "none": return undefined;
    case "lamp": return lamp([...AT], { colour: [...COLOUR], reach: REACH });
    case "castRay": {
      const bulb = lamp([...AT], { colour: [...COLOUR], reach: REACH });
      const width = (t: ReturnType<typeof f>) => mul(t, 0.05);
      return (p, xi) => {
        const a = bulb(p, xi);
        const hit = castRay({ o: p, d: a.toward, t0: f(0.05), t1: a.distance as ReturnType<typeof f>, width });
        return { ...a, amount: mul(a.amount, smoothstep(0, 1, div(hit.gap, width(hit.t)))) };
      };
    }
    case "map": return lamp([...AT], { colour: [...COLOUR], reach: REACH, size: 0.65, shadow: true });
    case "mapHard": return lamp([...AT], { colour: [...COLOUR], reach: REACH, shadow: true });
  }
}

const runs: Run[] = ["none", "lamp", "castRay", "map", "mapHard"];
const device = await headlessDevice();
const loaded: { run: Run; frame: any; view: any }[] = [];
const pictures = new Map<Run, Uint8Array>();
for (const run of runs) {
  const { scene } = await chosenAt("examples/bosses/reefback_titan.ts", { kind: "default" });
  const light = lightOf(run);
  const lit = light === undefined ? scene : { ...scene, lighting: { ...scene.lighting, lights: [...scene.lighting.lights, light] } };
  const t0 = performance.now();
  const a = compileScene(lit).artefact;
  const l = await createHost(device).load(a);
  const view = { camera: a.cameras[viewName], width: W, height: H, time: 1 };
  const px = await l.frame(view);
  pictures.set(run, px);
  console.log(`${run.padEnd(8)} compile+load ${(performance.now() - t0).toFixed(0)} ms  rgba ${createHash("sha256").update(px).digest("hex").slice(0, 16)}`);
  if (outDir !== undefined) writeWebp(`${outDir}/crab_${run}.webp`, { width: W, height: H, rgba: px });
  loaded.push({ run, frame: l, view });
}
console.log(`load average before: ${loadavg().map((x) => x.toFixed(1)).join(" ")}`);
const ms = runs.map(() => [] as number[]);
for (let r = 0; r < reps; r++) for (const [k, x] of loaded.entries()) ms[k]!.push((await x.frame.frameGpu(x.view, 1)).ms);
console.log(`load average after:  ${loadavg().map((x) => x.toFixed(1)).join(" ")}`);
const q = (xs: number[], p: number) => { const s = [...xs].sort((a, b) => a - b); return s[Math.min(s.length - 1, Math.floor(p * s.length))]!; };
console.log(`GPU ms per frame over ${reps} interleaved frames (${viewName}, ${W}x${H}):   median   q25    q75    best`);
for (const [k, x] of loaded.entries()) console.log(`  ${x.run.padEnd(8)} ${[0.5, 0.25, 0.75, 0].map((p) => q(ms[k]!, p).toFixed(2).padStart(6)).join(" ")}`);
for (let k = 1; k < runs.length; k++) {
  for (const against of [0, 1]) {
    if (k === against) continue;
    const d = ms[k]!.map((x, r) => x - ms[against]![r]!);
    console.log(`  ${runs[k]!.padEnd(8)} - ${runs[against]!.padEnd(5)} paired: median ${q(d, 0.5).toFixed(2)}  q25 ${q(d, 0.25).toFixed(2)}  q75 ${q(d, 0.75).toFixed(2)}`);
  }
}
// Luminance differences (Rec. 709 weights on the 8-bit sRGB values, 0..1).
const lum = (p: Uint8Array, i: number) => (0.2126 * p[i]! + 0.7152 * p[i + 1]! + 0.0722 * p[i + 2]!) / 255;
const share = (a: Run, b: Run) => {
  const x = pictures.get(a)!, y = pictures.get(b)!;
  let n = 0;
  for (let i = 0; i < x.length; i += 4) if (Math.abs(lum(x, i) - lum(y, i)) > 0.1) n++;
  return (100 * n) / (x.length / 4);
};
for (const [a, b] of [["castRay", "map"], ["castRay", "lamp"], ["map", "lamp"], ["map", "mapHard"]] as [Run, Run][]) {
  console.log(`  pixels whose luminance differs by > 0.1, ${a} vs ${b}: ${share(a, b).toFixed(2)}%`);
}
for (const x of loaded) x.frame.destroy();
process.exit(0);
