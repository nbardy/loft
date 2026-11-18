// Non-aligned volume rows, a load dependency, a shortened final checkpoint, and a partial frame write. The color
// channels report independent numbers, so a plausible-looking image cannot hide a scheduling or copy bug.
import { expect, test } from "bun:test";
import { always, atLeast, compute, drawSpec, emitShaderFn, fetch, ge, input, nothing, param, scene, solve, spline, tabulate, time, toon, vec2, vec4, type Drawable } from "../src/index.ts";
import { runChecks } from "../src/check.ts";
import { compile } from "../src/compile/index.ts";
import { SceneError } from "../src/errors.ts";
import { SolveError } from "../src/geom/solve.ts";
import { headlessDevice } from "../src/host/headless.ts";
import { bakeRawBuild, loadBundle } from "../src/host/host.ts";
import { lower } from "../src/host/lower.ts";

const volume = compute("testVolume", {
  when: { kind: "build" }, entry: "main", dispatch: [3, 2, 3],
  writes: { sdf: { kind: "texture", size: [3, 2, 3], format: "r32float" } },
  wgsl: `
@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
  textureStore(sdf, vec3<i32>(id), vec4<f32>(f32(id.x + 2u * id.y + 3u * id.z) / 16.0));
}`,
});

const baked = compute("testBaked", {
  when: { kind: "build" }, entry: "main", dispatch: [1], reads: { volume: volume.sdf },
  writes: { point: { kind: "buffer", entries: 1, lanes: 1 } },
  wgsl: "@compute @workgroup_size(1) fn main() { point[0] = textureLoad(volume, vec3<i32>(2, 1, 2), 0); }",
});

const seed = compute("testLoad", {
  when: { kind: "load" }, entry: "main", dispatch: [1], reads: { point: baked.point },
  writes: { value: { kind: "buffer", entries: 1, lanes: 1 } },
  wgsl: "@compute @workgroup_size(1) fn main() { value[0] = point[0]; }",
});

const steps = compute("testSteps", {
  when: { kind: "steps", start: "init", count: 5, keep: 2, rate: 4 }, entry: "main", dispatch: [1],
  reads: { seed: seed.value },
  writes: {
    count: { kind: "buffer", entries: 1, lanes: 1 },
    image: { kind: "texture", size: [1, 2], format: "r32float" },
    brick: { kind: "texture", size: [1, 1, 2], format: "r32float" },
  },
  wgsl: `
@compute @workgroup_size(1) fn init() {
  count[0] = seed[0];
  textureStore(image, vec2<i32>(0, 1), vec4<f32>(0.25));
  textureStore(brick, vec3<i32>(0, 0, 1), vec4<f32>(0.0625));
}
@compute @workgroup_size(1) fn main() {
  count[0] = prev_count[0] + vec4<f32>(0.0625, 0, 0, 0);
  let p = vec2<i32>(0, 1);
  textureStore(image, p, textureLoad(prev_image, p, 0) + vec4<f32>(0.0625));
  let q = vec3<i32>(0, 0, 1);
  textureStore(brick, q, textureLoad(prev_brick, q, 0) + vec4<f32>(0.0625));
}`,
});

const gate = input("gate", "f32", [0]);
const scale = param("scale", 1);
const frame = compute("testFrame", {
  when: { kind: "frame" }, entry: "main", dispatch: [1], reads: { count: steps.count, image: steps.image },
  values: { gate, scale },
  writes: { colour: { kind: "texture", size: [1, 1], format: "rgba8unorm" }, factor: { kind: "buffer", entries: 1, lanes: 1 } },
  wgsl: `
@compute @workgroup_size(1) fn main() {
  if (gate() > 0.5) {
    textureStore(colour, vec2<i32>(0), vec4<f32>(count(0u, 0u, loft_time()).x, image(vec2<i32>(0, 1), loft_time()).x, 0, 1));
    factor[0] = vec4<f32>(scale());
  }
}`,
});

const copy = compute("testCopy", {
  when: { kind: "frame" }, entry: "main", dispatch: [1], reads: { src: frame.colour },
  writes: { colour: { kind: "texture", size: [1, 1], format: "rgba8unorm" } },
  wgsl: "@compute @workgroup_size(1) fn main() { textureStore(colour, vec2<i32>(0), textureLoad(src, vec2<i32>(0), 0)); }",
});
const pixel = copy.colour.sample([0.5, 0.5]);
const poseRead = tabulate("poseRead", () => copy.colour.sample([0.5, 0.5]).x, 2, "frame");
const read = vec4(poseRead(0), pixel.y, steps.brick.sample([0.5, 0.5, 0.75]).x, fetch(frame.factor, 0, 0).x);
const draw: Drawable = {
  name: "measured", skeletons: [], forms: [],
  draws(ctx) {
    const { libs, code } = emitShaderFn(ctx.slots, { name: "computed", args: [], out: "vec4<f32>", roots: [read], ret: ([p]) => p! });
    return {
      shaders: { measured: { libs, code: code + `
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
  return vec4<f32>(array(vec2<f32>(-1,-1), vec2<f32>(3,-1), vec2<f32>(-1,3))[i], 0.5, 1);
}
@fragment fn fs() -> @location(0) vec4<f32> { let c = computed(); return vec4<f32>(c.r, c.g, c.b * c.a, 1); }` } },
      draws: [drawSpec({ label: "measured", shader: "measured", vertices: 3 })],
    };
  },
};

const camera = { kind: "perspective", eye: [0, 0, 2], target: [0, 0, 0], fovY: 0.6 } as const;
const example = scene({ compute: [copy, frame, steps, seed, baked, volume], ground: nothing, things: [draw], style: toon(), sun: [0, 1, 1], cameras: { front: camera } });

test("raw compute round trips 3D rows, orders jobs, keeps the final step and clears each frame", async () => {
  const a = compile(example);
  const emitted = lower(a);
  const device = await headlessDevice();
  try {
    const data = await bakeRawBuild(device, emitted);
    const bytes = data.get("testVolume.sdf")!;
    expect(bytes.byteLength).toBe(3 * 2 * 3 * 4);
    expect([...new Float32Array(bytes.buffer)]).toEqual(
      Array.from({ length: 18 }, (_, i) => ((i % 3) + 2 * (Math.floor(i / 3) % 2) + 3 * Math.floor(i / 6)) / 16),
    );
    const finished = lower(a, data);
    expect(finished.manifest.build).toBeUndefined();
    expect(finished.manifest.computePipelines.every((p) => !p.label.includes("testVolume") && !p.label.includes("testBaked"))).toBe(true);
    for (const bundle of [emitted, finished]) {
      const gpu = await loadBundle(device, bundle, { kind: "none" });
      try {
        const view = { camera, height: 9, width: 11 };
        const sample = async (time: number, gate: number, scale = 1, width = view.width) =>
          [...(await gpu.frame({ ...view, width, time, params: { scale }, inputs: { gate } })).subarray(4 * (4 * width + 5), 4 * (4 * width + 6))];
        // 10/16 baked; at 0.95s the preceding kept state is step 2. At 1.25s the final step is 5.
        expect(await sample(0.95, 1)).toEqual([191, 96, 48, 255]);
        expect(await sample(1.25, 1, 0.5, 19)).toEqual([239, 143, 48, 255]);
        expect(await sample(0.1, 0)).toEqual([0, 0, 0, 255]);
        expect(await sample(0.95, 1)).toEqual([191, 96, 48, 255]);
      } finally { gpu.destroy(); }
    }
  } finally { device.destroy(); }
}, 120_000);

test("CPU checks and solve identify the exact raw compute read", () => {
  const checks = [
    always([time, 0, 1], ge(pixel.x, 0)),
    always([time, 0, 1], ge(poseRead(0), 0)),
    { name: "sampled", run: (p: Parameters<ReturnType<typeof always>["run"]>[0]) => { p.scalar(pixel.x); throw new Error("expected refusal"); } },
  ];
  for (const result of runChecks(example, checks)) {
    expect(result.holds).toBe("undecided");
    expect(result.measured).toContain('raw compute "testCopy.colour"');
  }
  try {
    solve({ over: [0, 1], vary: { x: spline(4, 1) }, hold: ({ x }) => [atLeast(x, pixel.x, { at: 0 })], prefer: { jerk: 1 } });
    throw new Error("expected refusal");
  } catch (e) {
    expect(e).toBeInstanceOf(SolveError);
    expect((e as SolveError).failure).toMatchObject({ kind: "undecided", resource: "testCopy.colour" });
  }
  expect(() => compile({ ...example, compute: [copy, frame, steps, baked, volume] })).toThrow(SceneError);
});
