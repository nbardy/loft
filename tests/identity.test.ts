// src/identity.ts. The bug these catch: something a frame depends on that the id leaves out, so two different frames
// share an id (a producer that fills a field from the wrong place, a default not filled in, a canonical form that drops
// a value), or the same frame drawn twice getting two ids.
import { expect, test } from "bun:test";
import { createHash } from "node:crypto";
import mixedScene from "../examples/mixed_precision.ts";
import stormwing from "../examples/stormwing.ts";
import type { View } from "../src/artefact.ts";
import { drawFrame } from "../src/cli/still.ts";
import { compile } from "../src/compile/index.ts";
import { headlessDevice } from "../src/host/headless.ts";
import { loadBundle } from "../src/host/host.ts";
import { lower } from "../src/host/lower.ts";
import type { StillSpec } from "../src/host/still.ts";
import { buildOf, canonical, deviceOf, frameOf, identityId, programOf, type Identity } from "../src/identity.ts";

test("identity: every leaf of the record reaches the id, -0 included", () => {
  const sample: Identity = {
    build: { program: { kind: "js-tables", sha256: "aa", tables: ["t"] }, library: "bb", tier: "compiled", precision: "f32" },
    frame: {
      camera: { kind: "perspective", eye: [1, 2, 3], target: [0, 0, 0], fovY: 0.6 }, time: 0, size: [64, 48],
      params: { span: 1 }, inputs: { strike: [0] }, precision: "f32", drawn: { kind: "still", samples: 4, shutter: 0.5, aperture: 0 }, construction: false,
    },
    device: { runtime: "r", adapter: "a", driver: "d" },
  };
  const base = identityId(sample);
  let leaves = 0;
  const walk = (v: unknown, set: (x: unknown) => void) => {
    if (typeof v === "object" && v !== null) {
      for (const k of Object.keys(v)) walk((v as Record<string, unknown>)[k], (x) => { (v as Record<string, unknown>)[k] = x; });
      return;
    }
    leaves++;
    const changed = typeof v === "number" ? (v === 0 ? -0 : v + 1) : typeof v === "boolean" ? !v : `${String(v)}x`;
    set(changed);
    expect(identityId(sample)).not.toBe(base);
    set(v);
  };
  walk(sample, () => {});
  expect(leaves).toBe(28);
  expect(identityId(sample)).toBe(base);
  expect(canonical({ b: 1, a: [-0, 0.1] })).toBe('{"a":[-0,0.1],"b":1}');
});

test("identity: the same view twice is one id and one RGBA; each thing a frame depends on changes the id", async () => {
  const a = compile(stormwing);
  const bundle = lower(a);
  const device = await headlessDevice();
  const loaded = await loadBundle(device, bundle, { kind: "js", uploads: (s) => a.cpuPose(s) });
  try {
    const part = { build: buildOf(bundle), device: deviceOf(device) };
    const draw = async (view: View, drawn: StillSpec = { kind: "frame" }) => {
      const r = await drawFrame(loaded, view, drawn);
      return { id: identityId({ ...part, frame: frameOf(bundle.manifest, view, loaded.precision, drawn) }), rgba: createHash("sha256").update(r.pixels).digest("hex") };
    };
    const camera = { kind: "perspective", eye: [-14, 9, -13], target: [0, 1, 0], fovY: 0.6 } as const;
    const view: View = { camera, width: 64, height: 48, time: 0.7 };
    const base = await draw(view);
    expect(await draw({ ...view })).toEqual(base);
    // A default given explicitly is the frame without it.
    expect((await draw({ ...view, params: { "stormwing.span": 1 }, inputs: { strike: 0 } })).id).toBe(base.id);
    const changed = {
      time: await draw({ ...view, time: 1.1 }),
      size: await draw({ ...view, width: 48 }),
      camera: await draw({ ...view, camera: { ...camera, fovY: 0.5 } }),
      param: await draw({ ...view, params: { "stormwing.span": 1.4 } }),
      input: await draw({ ...view, inputs: { strike: 0.5 } }),
      construction: await draw({ ...view, construction: true }),
      still: await draw(view, { kind: "still", samples: 2, shutter: 0, aperture: 0 }),
    };
    for (const [what, c] of Object.entries(changed)) expect([what, c.id]).not.toEqual([what, base.id]);
    // The contract's direction: different RGBA, different id (none of these collide).
    expect(new Set(Object.values(changed).map((c) => c.id)).size).toBe(Object.keys(changed).length);
    // Another scene is another program.
    expect(programOf(lower(compile(mixedScene))).sha256).not.toBe(part.build.program.sha256);
  } finally {
    loaded.destroy();
    device.destroy();
  }
}, 120_000);

test("identity: a mixed bundle on a device without shader-f16 records f32-fallback", async () => {
  const a = compile(mixedScene);
  const mixed = lower(a, "mixed");
  const capable = await headlessDevice();
  // A device asked for no features: what a machine without shader-f16 gets.
  const portable = await (await navigator.gpu.requestAdapter())!.requestDevice();
  const view: View = { camera: a.cameras.three!, width: 64, height: 48, time: 1 };
  const frame = async (device: GPUDevice) => {
    const loaded = await loadBundle(device, mixed, { kind: "none" });
    try {
      await loaded.frame(view);
      const identity: Identity = { build: buildOf(mixed), frame: frameOf(mixed.manifest, view, loaded.precision, { kind: "frame" }), device: deviceOf(device) };
      return { identity, id: identityId(identity) };
    } finally { loaded.destroy(); }
  };
  try {
    const fallback = await frame(portable);
    expect(fallback.identity.build.precision).toBe("mixed");
    expect(fallback.identity.frame.precision).toBe("f32-fallback");
    if (capable.features.has("shader-f16")) {
      const half = await frame(capable);
      expect(half.identity.frame.precision).toBe("mixed");
      expect(half.id).not.toBe(fallback.id);
    }
  } finally {
    portable.destroy();
    capable.destroy();
  }
}, 120_000);
