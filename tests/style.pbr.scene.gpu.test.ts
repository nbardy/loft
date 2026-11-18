// The regular scene-to-WebGPU path: a field gets its geometry in its local shade function, and a
// time expression / host input reaches the projected environment of a single loaded artefact.
import { beforeAll, expect, test } from "bun:test";
import ambient, { spec as relief } from "../examples/materials/occlusion.ts";
import moving, { spec as disc } from "../examples/materials/moving_sun.ts";
import { compileScene } from "../src/compile/index.ts";
import { headlessDevice } from "../src/host/headless.ts";
import { createHost, type GpuHost } from "../src/host/index.ts";
import { f, movable, pbr, scene, vec3, type Scene } from "../src/index.ts";

const SIZE = 256;
let host: GpuHost;
beforeAll(async () => { host = createHost(await headlessDevice()); });

const camera = (s: Scene) => ({ camera: s.cameras.front!, width: SIZE, height: SIZE });
const load = (s: Scene) => host.load(compileScene(s).artefact);
const still = async (s: Scene) => {
  const loaded = await load(s);
  try { return await loaded.frame({ ...camera(s), time: 0 }); }
  finally { loaded.destroy(); }
};

/** Average 3 × 3 opaque pixels, away from coverage boundaries. */
function rgb(pixels: Uint8Array, x: number, y = SIZE / 2): number[] {
  return [0, 1, 2].map((ch) => {
    let sum = 0;
    for (let j = y - 1; j <= y + 1; j++) for (let i = x - 1; i <= x + 1; i++) sum += pixels[4 * (SIZE * j + i) + ch]!;
    return sum / 9;
  });
}

test("field AO darkens the groove and preserves the flat face and direct light", async () => {
  const off = () => f(1);
  const lit = await still(ambient);
  const open = await still(scene({ ...relief, style: pbr({ occlusion: off }) }));
  const groove = 76;  // x = -0.56, on the groove floor near its left wall
  const face = 128;   // x = 0, clear of either groove
  expect(rgb(lit, groove)[0]!).toBeLessThan(0.8 * rgb(open, groove)[0]!);
  expect(Math.abs(rgb(lit, face)[0]! - rgb(open, face)[0]!)).toBeLessThan(1);

  // Put the sun in front, with the atmosphere black. The same groove gets direct light and the slot may only dim sky.
  const sky = () => vec3(0, 0, 0);
  const direct = { ...relief, sun: [0, 0.45, 1] as const };
  const withAo = rgb(await still(scene({ ...direct, style: pbr({ sky }) })), groove);
  const withoutAo = rgb(await still(scene({ ...direct, style: pbr({ sky, occlusion: off }) })), groove);
  expect(withAo[0]!).toBeGreaterThan(100);
  expect(withAo).toEqual(withoutAo);
});

test("the rough disc's environment follows time and host sun inputs on the GPU", async () => {
  const left = [-0.6, 0.65, -0.65] as const, right = [0.6, 0.65, -0.65] as const;
  const fixed = rgb(await still(scene({ ...disc, sun: left })), SIZE / 2);
  const moved = scene({ ...disc, sun: movable(left) });
  const timeDriven = await load(moving);
  const inputDriven = await load(moved);
  try {
    const t0 = rgb(await timeDriven.frame({ ...camera(moving), time: 0 }), SIZE / 2);
    const t1 = rgb(await timeDriven.frame({ ...camera(moving), time: 1 }), SIZE / 2);
    const i1 = rgb(await inputDriven.frame({ ...camera(moved), time: 0, inputs: { sun: right } }), SIZE / 2);
    const i0 = rgb(await inputDriven.frame({ ...camera(moved), time: 0, inputs: { sun: left } }), SIZE / 2);
    // z > 0 normal with the sun behind, rough = 1. The direct term is zero and the sharp sky tap's weight is e^-12.
    expect(t0[0]! - t0[2]!).toBeGreaterThan(85);
    expect(t1[2]! - t1[0]!).toBeGreaterThan(85);
    for (let ch = 0; ch < 3; ch++) {
      expect(Math.abs(t0[ch]! - fixed[ch]!)).toBeLessThanOrEqual(1);
      expect(Math.abs(i0[ch]! - fixed[ch]!)).toBeLessThanOrEqual(1);
      expect(Math.abs(t1[ch]! - i1[ch]!)).toBeLessThanOrEqual(1);
    }
  } finally {
    timeDriven.destroy();
    inputDriven.destroy();
  }
});
