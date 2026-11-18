import { beforeAll, expect, test } from "bun:test";
import {
  compile, draw, mul, nothing, scene, time, toon, vec3, type Camera, type Drawable, type Light, type Style, type VolumeOptions,
} from "../src/index.ts";
import { headlessDevice } from "../src/host/headless.ts";
import { createHost, type GpuHost } from "../src/host/index.ts";

let host: GpuHost;
beforeAll(async () => { host = createHost(await headlessDevice()); });

const W = 80, H = 48;
const camera: Camera = { kind: "orthographic", eye: [0, 0, 5], target: [0, 0, 0], height: 3 };
const bounds: VolumeOptions["bounds"] = { min: [-3, -2, -1], max: [3, 2, 1] };
const { volume: _toonVolume, ...toonSurfaces } = toon();
const black = () => vec3(0, 0, 0);
const style: Style = { ...toonSurfaces, sky: black, shade: black, clear: [0, 0, 0] };
const frame = { camera, width: W, height: H, time: 1 };
const at = (rgba: Uint8Array, x: number, channel: number) => rgba[(H / 2 * W + x) * 4 + channel]!;
const close = (actual: number, expected: number) => expect(Math.abs(actual - expected * 255)).toBeLessThanOrEqual(2);

// Catches incorrect clipping through an opaque surface inside the medium, a camera-inside miss, and straight-alpha
// clamping of a bright source before its fractional extinction has been applied.
test("a homogeneous emissive slab integrates only the visible path length", async () => {
  const wall = draw.mesh("wall", {
    points: [[-3, -2, 0], [0, -2, 0], [0, 2, 0], [-3, 2, 0]], triangles: [[0, 1, 2], [0, 2, 3]],
  }, { sides: "one", outline: "none", paint: black, skeletons: [] });
  const sigma = Math.log(2) / 2;
  const slab = draw.volume("slab", () => mul(sigma, time), { bounds, steps: 64, emission: [2, 0, 0.4] });
  const s = scene({ ground: nothing, things: [wall, slab], style, sun: { toward: [0, 1, 0], amount: [0, 0, 0] } });
  const loaded = await host.load(compile(s));
  try {
    const px = await loaded.frame(frame);
    const left = W / 2 - 3, right = W / 2 + 3;
    close(at(px, left, 0), 2 * (1 - Math.exp(-sigma)));
    close(at(px, right, 2), 0.4 * (1 - Math.exp(-2 * sigma)));
    const inside = await loaded.frame({ ...frame, camera: { kind: "perspective", eye: [0, 0, 0.5], target: [0, 0, 0], fovY: 0.7 } });
    close(at(inside, right, 2), 0.4 * (1 - Math.exp(-1.5 * sigma)));
    const atZero = await loaded.frame({ ...frame, time: 0 });
    expect(at(atZero, right, 2)).toBe(0);
  } finally {
    loaded.destroy();
  }
});

// The red directional light traverses the rest of the slab. The blue lamp is a half-unit toward the eye.
// Catches a sun direction used for a lamp, a shadow ray marching past its light, or using view opacity as visibility.
test("style receives distinct phase-weighted arrivals and ray visibility for the sun and a finite lamp", async () => {
  const light: Light = () => ({ toward: vec3(0, 0, 1), distance: 0.5, amount: [0, 0, 4 * Math.PI] });
  const sigma = 0.6;
  const slab = (shadow: VolumeOptions["shadow"]): Drawable => draw.volume("slab", () => sigma, {
    bounds, steps: 64, ...(shadow === undefined ? {} : { shadow }),
  });
  const volume: NonNullable<Style["volume"]> = (ctx) => ctx.lights("sum", (l) => mul(l.unshadowed, l.visibility));
  const sunY = 0.1;
  const cos = 1 / Math.hypot(1, sunY);
  const shot = async (thing: Drawable, sunZ: number, strength: number, lights: Light[], look: Style) => host.load(compile(scene({
    ground: nothing, things: [thing], style: look, sun: { toward: [0, sunY, sunZ], amount: [strength * 4 * Math.PI, 0, 0] }, lights,
  })));
  const centre = W / 2;
  const direct = await shot(slab(undefined), -1, 1, [light], { ...style, volume });
  try {
    const px = await direct.frame(frame);
    close(at(px, centre, 0), 1 - Math.exp(-1.2));
    close(at(px, centre, 2), 1 - Math.exp(-1.2));
  } finally { direct.destroy(); }
  const shadowed = await shot(slab({ kind: "march", steps: 6 }), -1, 1, [light], { ...style, volume });
  try {
    const px = await shadowed.frame(frame);
    const k = sigma * (1 / cos - 1);
    close(at(px, centre, 0), sigma * Math.exp(-1.2 / cos) * Math.expm1(2 * k) / k);
    close(at(px, centre, 2), 0.5 * (1 - Math.exp(-0.6)) + Math.exp(-0.3) * (Math.exp(-0.3) - Math.exp(-1.2)));
    expect(await shadowed.frame(frame)).toEqual(px);
  } finally { shadowed.destroy(); }

  // At g = 0.5 the forward and backward poles are about 6 and 2/9 times the isotropic phase. Reversing the
  // sign of the view vector swaps the lobes, a regression that still produces plausible fog.
  const anisotropic = draw.volume("slab", () => sigma, { bounds, steps: 64, g: 0.5 });
  for (const z of [-1, 1]) {
    const phase = 0.75 / (1.25 + z * cos) ** 1.5;
    const loaded = await shot(anisotropic, z, 0.18, [], style);
    try { close(at(await loaded.frame(frame), centre, 0), 0.18 * phase * (1 - Math.exp(-1.2))); }
    finally { loaded.destroy(); }
  }
});
