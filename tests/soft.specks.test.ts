import { beforeAll, expect, test } from "bun:test";
import type { Camera } from "../src/artefact.ts";
import { createHost, type GpuHost } from "../src/host/host.ts";
import { lower } from "../src/host/lower.ts";
import { headlessDevice } from "../src/host/headless.ts";
import { add, blob, compile, draw, input, lamp, length, lt, medium, mul, nothing, pass, scene, select, sub, time, toon, vec3 } from "../src/index.ts";

let host: GpuHost;
beforeAll(async () => { host = createHost(await headlessDevice()); });
const W = 160, H = 96;
const camera: Camera = { kind: "orthographic", eye: [0, 0, 8], target: [0, 0, 0], height: 3.2 };
const view = { camera, width: W, height: H, time: 0 };
const style = toon({ outline: 0, sky: { horizon: [0, 0, 0], zenith: [0, 0, 0] } });

// A one-sample hard circle blinks or loses its energy as the centre crosses a pixel boundary.
test("sub-pixel grains preserve their area-weighted energy through a pixel of motion", async () => {
  const grain = draw.points("speck", 1, () => ({ at: vec3(mul(time, 1 / 30), mul(time, 1 / 30), 0), radius: 0.020, colour: [1, 1, 1], opacity: 1 }));
  const s = scene({ ground: nothing, things: [grain], style, sun: [0, 0.12, -1] });
  const loaded = await host.load(compile(s));
  try {
    const energy: number[] = [];
    for (const time of [0, 0.25, 0.5, 0.75, 1]) {
      const image = await loaded.frame({ ...view, time });
      let l = 0;
      for (let p = 0; p < image.length; p += 4) l += image[p]!;
      energy.push(l);
    }
    // One tent with projected radius 0.6 px integrates to 0.6² * 255 = 91.8 at unit light.
    expect(Math.min(...energy)).toBeGreaterThan(83);
    expect(Math.max(...energy)).toBeLessThan(101);
    expect(Math.max(...energy) - Math.min(...energy)).toBeLessThanOrEqual(5);
  } finally { loaded.destroy(); }
});

// Sampling white and black at the same centres isolates transmission from the coloured in-scattering.
test("the cloud veils grain contrast by depth, its tint remains, and an opaque body hides grains", async () => {
  const clay = medium("clayTest", { puffs: [blob({ centre: [0, 0, 0], radii: [3, 1, 0.6], density: 1.6, colour: [0.55, 0.15, 0.06] })] });
  const veil = draw.density("veil", clay, {
    bounds: { min: [-8, -3.2, -2.4], max: [8, 3.2, 2.4] }, sun: [0.8, 0.8, 0.8], sky: 0,
    forward: 0, shadowing: 0, detail: { kind: "none" }, haze: 0,
  });
  const boulder = draw.field("occluder", (p) => sub(length(sub(p, [1.8, 0, 2.9])), 0.24), {
    bounds: { kind: "box", min: [1.5, -0.3, 2.6], max: [2.1, 0.3, 3.2] }, steps: 40, outline: "silhouette", schedule: "trace",
    paint: () => vec3(0.2, 0.2, 0.2),
  });
  const bright = input("testPigment", "f32", [1]), visible = input("testVisibility", "f32", [1]);
  const fromBack = pass({ name: "sprayTestBack", size: [W, H], camera: { kind: "look", eye: [0, 0, -8], target: [0, 0, 0], lens: { halfHeight: 1.6 } } });
  const red = (pixels: Uint8Array, x: number): number => pixels[(47 * W + x) * 4]!;
  const contrasts: number[][] = [];
  for (const count of [4, 32768]) {
    const grains = draw.points("grains", count, (i) => ({
      at: vec3(add(-1.8, mul(i, 1.2)), 0, select(lt(i, 1), 2.6, select(lt(i, 2), 0, -2.6))),
      radius: 0.14, colour: vec3(bright, bright, bright), opacity: select(lt(i, 4), visible, 0),
    }), { veil });
    // Only the lamp can light the grains here.
    const s = scene({ ground: nothing, things: [boulder, veil, grains], style: { ...style, ink: () => fromBack.at([0.5, 0.5]).xyz },
      sun: { toward: [0, 1, 0], amount: [0, 0, 0] },
      lights: [lamp([0, 0, -4], { colour: [0.95, 0.95, 0.95], reach: 50 })] });
    const artefact = compile(s);
    if (count > 4) {
      // If the secondary pass reuses fs, its camera sees attenuation sampled from the opposite side of the cloud.
      const entries = lower(artefact).manifest.renderPipelines.filter((p) => p.label.startsWith("specks.grains ")).map((p) => p.fragment?.entryPoint);
      expect(entries.sort()).toEqual(["fs", "fs_scene_pass"]);
    }
    const loaded = await host.load(artefact);
    try {
      const white = await loaded.frame(view);
      const black = await loaded.frame({ ...view, inputs: { testPigment: 0 } });
      const bare = await loaded.frame({ ...view, inputs: { testVisibility: 0 } });
      const lum = [26, 62, 98, 134].map((x) => red(white, x) - red(black, x));
      expect(lum[0]!).toBeGreaterThan(160);
      expect(lum[1]! / lum[0]!).toBeWithin(0.38, 0.52);
      expect(lum[2]! / lum[0]!).toBeWithin(0.15, 0.25);
      expect(Math.abs(lum[3]!)).toBeLessThanOrEqual(1);
      // A black grain behind the cloud should match the cloud over black sky, including its red radiance.
      expect(red(bare, 98)).toBeGreaterThan(45);
      expect(Math.abs(red(black, 98) - red(bare, 98))).toBeLessThanOrEqual(4);
      contrasts.push(lum);
    } finally { loaded.destroy(); }
  }
  expect(Math.max(...contrasts[0]!.map((a, i) => Math.abs(a - contrasts[1]![i]!)))).toBeLessThanOrEqual(5);
});

// The bug it catches: a volume veil that integrates the wrong span (the whole box, or nothing in front of the grain),
// or at the wrong rate. A black, homogeneous slab of extinction sigma between z = -2 and 2: white minus black grains
// isolates transmission, exp(-sigma * path inside the slab to the grain): 1 in front, 1/2 at the centre, 1/4 behind.
test("a draw.volume veil dims grains by its transmittance to each grain, marched or from its grid", async () => {
  const sigma = Math.log(2) / 2;
  const slab = draw.volume("slab", () => sigma, { bounds: { min: [-8, -3, -2], max: [8, 3, 2] }, steps: 64, albedo: [0, 0, 0] });
  const bright = input("testVolumePigment", "f32", [1]);
  for (const count of [3, 32768]) {
    const grains = draw.points("grains", count, (i) => ({
      at: vec3(add(-1.8, mul(i, 1.2)), 0, select(lt(i, 1), 2.6, select(lt(i, 2), 0, -2.6))),
      radius: 0.14, colour: vec3(bright, bright, bright), opacity: select(lt(i, 3), 1, 0),
    }), { veil: slab });
    // The sun alone lights the grains, the same for each (a lamp's falloff would differ between them).
    const s = scene({ ground: nothing, things: [slab, grains], style, sun: { toward: [0, 1, 0], amount: [1.8, 1.8, 1.8] } });
    const loaded = await host.load(compile(s));
    try {
      const white = await loaded.frame(view);
      const black = await loaded.frame({ ...view, inputs: { testVolumePigment: 0 } });
      const lum = [26, 62, 98].map((x) => white[(47 * W + x) * 4]! - black[(47 * W + x) * 4]!);
      console.info(`volume veil, ${count} grains: contrast ${lum.join(", ")}; shares ${(lum[1]! / lum[0]!).toFixed(3)}, ${(lum[2]! / lum[0]!).toFixed(3)} (predicted 0.500, 0.250)`);
      expect(lum[0]!).toBeGreaterThan(160);
      expect(lum[1]! / lum[0]!).toBeWithin(0.47, 0.53);
      expect(lum[2]! / lum[0]!).toBeWithin(0.22, 0.28);
    } finally { loaded.destroy(); }
  }
});
