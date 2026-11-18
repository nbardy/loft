import { expect, test } from "bun:test";
import scene from "../examples/mixed_precision.ts";
import { compile } from "../src/compile/index.ts";
import { headlessDevice } from "../src/host/headless.ts";
import { loadBundle } from "../src/host/host.ts";
import { lower } from "../src/host/lower.ts";

test("mixed files bind matching palette bytes and shaders, with exact f32 fallback pixels", async () => {
  const a = compile(scene);
  const f32 = lower(a);
  const mixed = lower(a, "mixed");
  const variant = mixed.manifest.precision!;
  const entries = Object.entries(variant.buffers);
  expect(entries).toHaveLength(1);
  const [id, packed] = entries[0]!;
  expect(packed.size).toBe(2048);
  expect(f32.manifest.buffers[Number(id)]!.size).toBe(4096);
  expect(mixed.blobs.get(packed.init)!.byteLength).toBe(packed.size);

  const capable = await headlessDevice();
  const portable = await (await navigator.gpu.requestAdapter())!.requestDevice();
  const view = { camera: a.cameras.three!, width: 256, height: 160, time: 1 };
  const draw = async (device: GPUDevice, bundle: typeof f32) => {
    const loaded = await loadBundle(device, bundle, { kind: "none" });
    try { return { precision: loaded.precision, pixels: await loaded.frame(view) }; }
    finally { loaded.destroy(); }
  };
  try {
    const base = await draw(portable, f32);
    const fallback = await draw(portable, mixed);
    expect(base.precision).toBe("f32");
    expect(fallback.precision).toBe("f32-fallback");
    expect(Buffer.compare(fallback.pixels, base.pixels)).toBe(0);
    if (capable.features.has("shader-f16")) {
      const half = await draw(capable, mixed);
      expect(half.precision).toBe("mixed");
      let max = 0;
      for (const [i, value] of base.pixels.entries()) max = Math.max(max, Math.abs(value - half.pixels[i]!));
      expect(max).toBeGreaterThan(0);
      expect(max).toBeLessThanOrEqual(2);
    }
  } finally {
    portable.destroy();
    capable.destroy();
  }
});
