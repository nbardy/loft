// From the worktree root: bun agent_notes/2026-11-18_f16/bench.ts
import { createHash } from "node:crypto";
import { mkdirSync } from "node:fs";
import creature from "../../examples/serpent.ts";
import pbr from "../../examples/mixed_precision.ts";
import medium from "../../examples/open.ts";
import { compile } from "../../src/compile/index.ts";
import { headlessDevice } from "../../src/host/headless.ts";
import { loadBundle } from "../../src/host/host.ts";
import { lower } from "../../src/host/lower.ts";
import type { Manifest } from "../../src/host/manifest.ts";
import { selectPrecision } from "../../src/host/precision.ts";
import { textureSize } from "../../src/host/host.ts";
import { paperGrain } from "../../src/style/post.ts";

const width = 1280, height = 800, rounds = 31, out = "out/f16";
const small = { width: 384, height: 240 };
const device = await headlessDevice();
if (!device.features.has("shader-f16")) throw new Error("Benchmark requires a device with shader-f16");
mkdirSync(out, { recursive: true });
console.log(device.adapterInfo, { width, height, rounds });
const scenes = [
  ["creature", { ...creature, style: { ...creature.style, post: [paperGrain()] } }],
  ["pbr", pbr],
  ["medium", medium],
] as const;

function memory(m: Manifest) {
  const bytesPerPixel: Partial<Record<GPUTextureFormat, number>> = { rgba8unorm: 4, depth32float: 4, r32float: 4, r8unorm: 1, rgba16float: 8 };
  const buffers = m.buffers.reduce((n, b) => n + b.size, 0);
  let attachments = 0;
  for (const t of m.textures) {
    const bpp = bytesPerPixel[t.format];
    if (bpp === undefined) throw new Error(`Add bytes per pixel for ${t.format}`);
    const size = textureSize(t.size, { width, height });
    attachments += size.width * size.height * t.sampleCount * bpp;
  }
  return { buffers, attachments, outputAndReadback: width * height * 4 + Math.ceil(width * 4 / 256) * 256 * height };
}

const selected = process.argv.slice(2);
for (const [name, scene] of scenes) {
  if (selected.length > 0 && !selected.includes(name)) continue;
  const a = compile(scene);
  const bundles = [lower(a), lower(a, "mixed")] as const;
  const loaded = [];
  for (const b of bundles) loaded.push(await loadBundle(device, b, { kind: "js", uploads: (state) => a.cpuPose(state) }));
  const view = { camera: a.cameras.three!, width, height, time: 1 };
  const samples = [[], []] as [number[], number[]];
  for (let i = 0; i < rounds + 4; i++) {
    for (const k of i % 2 === 0 ? [0, 1] : [1, 0]) {
      const { ms } = await loaded[k]!.frameGpu(view, 1);
      if (i >= 4) samples[k]!.push(ms);
    }
  }
  const [f32, mixed] = [await loaded[0]!.frame(view), await loaded[1]!.frame(view)];
  const max = [0, 0, 0, 0];
  let changed = 0, squared = 0;
  for (let i = 0; i < f32.length; i += 4) {
    let different = false;
    for (let c = 0; c < 4; c++) {
      const d = Math.abs(f32[i + c]! - mixed[i + c]!);
      max[c] = Math.max(max[c]!, d);
      different ||= d !== 0;
      squared += d * d;
    }
    changed += Number(different);
  }
  const hash = (p: Uint8Array) => createHash("sha256").update(p).digest("hex");
  const timing = samples.map((xs) => {
    const sorted = xs.toSorted((a, b) => a - b);
    return { p10: sorted[3], median: sorted[15], p90: sorted[27] };
  });
  console.log(JSON.stringify({ name, timing, memory: bundles.map((b) => memory(selectPrecision(b, true).bundle.manifest)), max, changed, rms: Math.sqrt(squared / f32.length), hashes: [hash(f32), hash(mixed)] }));
  const stills = [await loaded[0]!.frame({ ...view, ...small }), await loaded[1]!.frame({ ...view, ...small })];
  const pair = new Uint8Array(small.width * 2 * small.height * 3);
  for (let y = 0; y < small.height; y++) for (let k = 0; k < 2; k++) for (let x = 0; x < small.width; x++) {
    const p = (y * small.width + x) * 4, q = (y * small.width * 2 + k * small.width + x) * 3;
    pair.set(stills[k]!.subarray(p, p + 3), q);
  }
  await Bun.write(`${out}/${name}.ppm`, new Blob([`P6\n${small.width * 2} ${small.height}\n255\n`, pair]));
  loaded.forEach((l) => l.destroy());
}
device.destroy();
