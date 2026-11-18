import { expect, test } from "bun:test";
import { compile, depthOfField, draw, filmGrain, mix, nothing, scene, spacing, surface, toon, vec3, type PostSpec } from "../src/index.ts";
import { headlessDevice } from "../src/host/headless.ts";
import { createHost } from "../src/host/index.ts";

const camera = { kind: "perspective", eye: [0, 0, 0], target: [0, 0, -4], fovY: 0.6 } as const;
const width = 320, height = 720;
const fy = height / (2 * Math.tan(camera.fovY / 2));
const view = { camera, width, height, time: 0 };

/** Flat, grayscale test cards with edges at exact pixel boundaries in the perspective view. */
const card = (name: string, z: number, l: number, t: number, r: number, b: number, grey: number) => {
  const x = (p: number) => (p - width / 2) * z / fy;
  const y = (p: number) => (height / 2 - p) * z / fy;
  const face = surface((u, v) => vec3(mix(x(l), x(r), u), mix(y(b), y(t), v), -z), { u: ["free", "free"], v: "free" });
  return draw.surface(name, face, { paint: () => vec3(grey), sides: "two", outline: "none", grid: { u: spacing.even(1), v: spacing.even(1) } });
};

const chart = (things: ReturnType<typeof card>[], post: readonly PostSpec[]) => compile(scene({
  ground: nothing, things, sun: [0, 1, 0], style: { ...toon(), shade: ({ base }) => base, post },
}));

const red = (bytes: Uint8Array, x: number, y: number) => bytes[(y * width + x) * 4]!;

// Catches the wrong depth linearisation, radius/diameter mixups, using the receiver's radius for foreground
// scatter, and leaking a defocused layer onto a sharp occluder (or vice versa).
test("thin-lens blur has the predicted radius and respects focus and occlusion", async () => {
  const optics = { focus: 4, focalLength: 0.11, fStop: 1 };
  const mid = width / 2;
  const a = chart([
    card("far_dark", 8, 0, 0, mid, height, 0.125),
    card("far_light", 8, mid, 0, width, height, 0.875),
    card("focus", 4, 0, 0, mid, 200, 0.25),
    card("far_backing", 7, 0, 200, width, 280, 0.25),
    card("far_occluder", 6, 0, 200, mid, 280, 0.75),
    card("focus_backing", 4, 0, 480, width, height, 0.25),
    card("near", 8 / 3, 0, 480, mid, height, 0.75),
  ], [depthOfField(optics)]);
  const loaded = await createHost(await headlessDevice()).load(a);
  const image = await loaded.frame(view).finally(() => loaded.destroy());
  // A uniform circular aperture has a projected line-spread variance of radius² / 4. Differences of the
  // rendered step are that line spread; estimate its variance with the measured centre to allow subpixel error.
  const spread = Array.from({ length: 26 }, (_, i) => red(image, mid - 12 + i, 360) - red(image, mid - 13 + i, 360));
  const mass = spread.reduce((sum, w) => sum + w, 0);
  const mean = spread.reduce((sum, w, i) => sum + w * i, 0) / mass;
  const radius = 2 * Math.sqrt(spread.reduce((sum, w, i) => sum + w * (i - mean) ** 2, 0) / mass);
  const expected = fy * optics.focalLength / (2 * optics.fStop) * (1 / optics.focus - 1 / 8);
  expect(Math.abs(radius - expected)).toBeLessThan(0.8);
  expect(red(image, mid - 1, 100)).toBe(64); // sharp up to the edge
  expect(red(image, mid, 100)).toBe(223); // far gather excludes the nearer card
  expect(red(image, mid - 2, 240)).toBeWithin(144, 180); // defocused far occluder, over farther backing
  expect(red(image, mid + 1, 240)).toBeWithin(80, 120);
  expect(red(image, mid - 3, 600)).toBeWithin(149, 181); // near disc reveals some of the backing
  expect(red(image, mid + 2, 600)).toBeWithin(84, 116); // near disc overlaps the sharp backing
  expect(red(image, mid + 9, 600)).toBe(64); // no scatter past the foreground CoC
});

// Catches a time-independent seed, a stateful seed, and noise applied equally to shadows and midtones.
test("film grain repeats by frame time and peaks in the midtones", async () => {
  const a = chart([
    card("black", 4, 0, 0, 80, height, 0),
    card("grey", 4, 80, 0, 240, height, 0.5),
    card("white", 4, 240, 0, width, height, 1),
  ], [filmGrain({ amount: 0.08 })]);
  const loaded = await createHost(await headlessDevice()).load(a);
  try {
    const first = await loaded.frame(view);
    const next = await loaded.frame({ ...view, time: 1 / 30 });
    const repeat = await loaded.frame(view);
    expect(Buffer.compare(repeat, first)).toBe(0);
    expect(Buffer.compare(next, first)).not.toBe(0);
    const greys = Array.from({ length: 150 }, (_, i) => red(first, i + 85, 360));
    const rms = Math.sqrt(greys.reduce((sum, x) => sum + (x - 128) ** 2, 0) / greys.length);
    expect(rms).toBeWithin(5, 7); // 0.08 * 255 / sqrt(12) = 5.89 byte levels
    for (let x = 0; x < 80; x++) {
      expect(red(first, x, 360)).toBe(0);
      expect(red(first, x + 240, 360)).toBe(255);
    }
  } finally {
    loaded.destroy();
  }
});
