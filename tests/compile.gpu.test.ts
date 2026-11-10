// The compiler's output on a real GPU device (bun-webgpu / Dawn): every shader builds, and every WGSL function a
// drawable can call agrees with its CPU twin over the same posed tables.
import { expect, test } from "bun:test";
import { PRELUDE_WGSL, SCENE_DEPTH_WGSL } from "../src/artefact.ts";
import serpent from "../examples/serpent.ts";
import {
  add, compileScene, evaluate, keys, mul, param, scatter, scene, sea, strand, through, time, toon, variable, vec3, wave,
  type Drawable, type Expr,
} from "../src/index.ts";
import { FRAGMENT, gpu, layouts, probe, relErr, stateAt, VERTEX } from "./compile.harness.ts";

// Catches: any WGSL the compiler generates that Dawn rejects (syntax, types, uniformity of derivatives, override use),
// and any draw whose state or constants cannot make a pipeline, before a host ever loads the artefact.
test("every serpent shader compiles and every draw makes a pipeline under the contract's layouts", async () => {
  const dev = await gpu();
  const a = compileScene(serpent).artefact;
  const [g0, g1] = layouts(dev, a, VERTEX | FRAGMENT);
  const depth = dev.createBindGroupLayout({ entries: [{ binding: 0, visibility: VERTEX | FRAGMENT, texture: { sampleType: "unfilterable-float" } }] });
  const errors: string[] = [];
  for (const d of a.draws) {
    dev.pushErrorScope("validation");
    const extra = d.sceneDepth === true ? SCENE_DEPTH_WGSL : "";
    const module = dev.createShaderModule({ code: PRELUDE_WGSL + extra + a.sceneWgsl + a.shaders[d.shader] });
    const layout = dev.createPipelineLayout({ bindGroupLayouts: d.sceneDepth === true ? [g0, g1, depth] : [g0, g1] });
    const blend = d.blend === "alpha" ? { color: { srcFactor: "src-alpha", dstFactor: "one-minus-src-alpha" }, alpha: { srcFactor: "one", dstFactor: "one-minus-src-alpha" } } as const : undefined;
    try {
      await dev.createRenderPipelineAsync({
        layout,
        vertex: { module, entryPoint: "vs", constants: d.constants },
        fragment: { module, entryPoint: "fs", constants: d.constants, targets: [{ format: "rgba8unorm", blend }] },
        primitive: { topology: "triangle-list", frontFace: "ccw", cullMode: d.cull },
        depthStencil: { format: "depth32float", depthWriteEnabled: d.depthWrite, depthCompare: "less" },
        multisample: { count: 4 },
      });
    } catch (e) {
      errors.push(`${d.label}: ${e instanceof Error ? e.message : String(e)}`);
    }
    const err = await dev.popErrorScope();
    if (err) errors.push(`${d.label}: ${err.message}`);
  }
  expect(errors).toEqual([]);
});

// A strand exercising everything the chart depends on: a moving spine (time, a param, a wave), a twist, both tip
// kinds, a keyed aspect, a boxy section, and few stations (so interpolation between them matters).
const probeGirth = keys([[0, 0.4], [0.5, 0.9], [1, 0.3]]);
const body = strand({
  name: "probe",
  spine: (m) => add(through(m, [[-4, 0.5, 0], vec3(-1, add(1.2, mul(time, 0.3)), 0.8), vec3(1.5, param("probe.lift", 0.7), -0.6), [4, 2.5, 0.2]]),
    wave(m, { axis: [0, 0, 1], waves: 1.3, rate: 0.6, amplitude: 0.5 })),
  stations: 12,
  girth: (m) => mul(probeGirth(m).x, param("probe.girth", 1.1)),
  section: { aspect: (m) => keys([[0, 1.4], [1, 0.6]])(m).x, squareness: 3 },
  twist: (m) => mul(m, 1.3),
  tips: [{ round: 0.1 }, { point: 0.2 }],
  paint: [1, 0, 0],
});
const swell = sea({ swell: [{ direction: [1, 0], length: 10, height: 0.2, speed: 1 }] });

// Catches: the WGSL chart and its CPU twin disagreeing: a `station_at` that interpolates differently from its
// JavaScript mirror, a quaternion convention flipped on one side, a slot or curve row bound to the wrong place, or a
// frame axis (x, y, z) assembled differently. Body coordinates include rho inside the skin and v past 1 (it wraps).
test("the WGSL chart functions agree with their CPU twins to 1e-4", async () => {
  const c = compileScene(scene({ sea: swell, things: [body], style: toon(), sun: [0.3, 0.8, 0.2] }));
  const chart = c.chart(body);
  const state = stateAt(c.artefact, 1.37);
  const xs: number[][] = [];
  for (const m of [0, 0.003, 0.05, 0.21, 0.5, 0.77, 0.94, 0.999, 1]) for (const v of [0.25, 0.4, 0.61, 0.9, 1.13]) for (const rho of [0, 0.6, 1]) xs.push([m, v, rho]);
  const fields = ["pos", "normal", "x", "y", "z"] as const;
  const code = `fn probe(i: u32, x: vec4<f32>) -> vec4<f32> {
  let b = ${chart.wgsl}(x.x, x.y, x.z);
  let k = i % 5u;
  return vec4<f32>(select(select(select(select(b.z, b.y, k == 3u), b.x, k == 2u), b.normal, k == 1u), b.pos, k == 0u), 0.0);
}`;
  const gpuOut = await probe(c.artefact, state, "", code, xs.flatMap((x) => fields.map(() => x)));
  const twin = chart.cpu(c.frame(state));
  let worstPos = 0, worstDir = 0;
  xs.forEach(([m, v, rho], i) => {
    const b = twin(m, v, rho);
    fields.forEach((f, k) => {
      const e = relErr(gpuOut[i * 5 + k], b[f]);
      if (f === "pos") worstPos = Math.max(worstPos, e); else worstDir = Math.max(worstDir, e);
    });
  });
  expect(worstPos).toBeLessThan(1e-4);
  // Normals near a closing tip come from a finite difference of radii (cancellation in f32), so 1e-3 there.
  expect(worstDir).toBeLessThan(1e-3);
});

// Catches: a declared table landing at the wrong offset in its packed buffer, an accessor reading the wrong binding,
// a fill not run (or run before the table it reads), or CPU-evaluated expressions read out of order. Two tables share
// one buffer; the second reads the first, and both carry values a shader can only get through the table.
test("a drawable-declared table reaches the shader with the values its fill wrote", async () => {
  const lift = param("probe.lift", 0.7);
  let first: { wgsl: string } | undefined, second: { wgsl: string } | undefined;
  const thing: Drawable = {
    name: "tabled",
    draws(ctx) {
      const cpu = ctx.evaluate([mul(time, 3), add(lift, 1)]);
      const a = ctx.table({ name: "first", vec4s: 3, fill: (frame, out) => {
        const [t3, l1] = frame.values(cpu) as number[];
        for (let i = 0; i < 3; i++) out.set([t3 + i, l1, i * 10, -1 - i], i * 4);
      } });
      const b = ctx.table({ name: "second", vec4s: 2, fill: (frame, out) => {
        const src = frame.table(a);
        out.set([src[0] * 2, src[4] * 2, src[8] * 2, 7], 0);
        out.set([1, 2, 3, 4], 4);
      } });
      first = a; second = b;
      return { shaders: {}, draws: [] };
    },
  };
  const c = compileScene(scene({ sea: swell, things: [body, thing], style: toon(), sun: [0, 1, 0] }));
  const state = stateAt(c.artefact, 2.5, { params: { "probe.lift": 1.25 } });
  const code = `fn probe(i: u32, x: vec4<f32>) -> vec4<f32> { return select(${second!.wgsl}(i - 3u), ${first!.wgsl}(i), i < 3u); }`;
  const got = await probe(c.artefact, state, "", code, [[0], [0], [0], [0], [0]]);
  expect(got).toEqual([[7.5, 2.25, 0, -1], [8.5, 2.25, 10, -2], [9.5, 2.25, 20, -3], [15, 17, 19, 7], [1, 2, 3, 4]]);
});

// Catches: scattered copies placed by the wrong instance index, at body coordinates evaluated differently on the GPU,
// on the wrong slice of the host, or with the frame's axes swapped. Each instance's root is compared with the host's
// CPU chart at the coordinates the CPU evaluates from `at(i)`.
test("scattered instances land where the CPU says", async () => {
  const bump = strand({ name: "bump", spine: (m) => through(m, [[0, 0, 0], [0, 0.3, 0]]), stations: 4, girth: 0.05, up: [1, 0, 0], paint: [1, 1, 1] });
  const count = 9;
  const at = (i: Expr<"f32">) => ({ m: add(0.1, mul(i, 0.1)), v: add(0.2, mul(i, 0.17)), rho: add(0.9, mul(time, 0.01)) });
  const sc = scatter({ name: "bumps", on: body, count, thing: bump, at, scale: (i) => add(1, i) });
  const c = compileScene(scene({ sea: swell, things: [body, sc], style: toon(), sun: [0, 1, 0] }));
  const state = stateAt(c.artefact, 0.8);
  const code = `fn probe(i: u32, x: vec4<f32>) -> vec4<f32> { return vec4<f32>(scatter_bumps_root(i).pos, 0.0); }`;
  const gpuOut = await probe(c.artefact, state, c.artefact.shaders["scatter.bumps"]!.replace(/@vertex[\s\S]*$/, ""), code, Array.from({ length: count }, () => [0]));
  const twin = c.chart(body).cpu(c.frame(state));
  for (let i = 0; i < count; i++) {
    const p = at(variable("i", "f32"));
    const val = (e: Expr<"f32">) => evaluate(e, { vars: { i }, inputs: { time: 0.8 } }) as number;
    const want = twin(val(p.m), val(p.v), val(p.rho)).pos;
    expect(relErr(gpuOut[i], want)).toBeLessThan(1e-4);
  }
});

// Catches: post passes declared by a drawable (or a style) not reaching `Artefact.post`, reaching it under a shader
// key that does not exist, or being run in the wrong order. Two passes: the first paints magenta, the second keeps
// only the red channel of the picture so far; a host frame must come out red everywhere.
test("post passes from a style and a drawable run in order on the host", async () => {
  const { createHost } = await import("../src/host/index.ts");
  const style = { ...toon(), post: [{ label: "magenta", wgsl: "@fragment fn fs(in: PostIn) -> @location(0) vec4<f32> { return vec4<f32>(1.0, 0.0, 1.0, 1.0); }" }] };
  const redOnly: Drawable = { name: "redOnly", draws: (ctx) => {
    ctx.post({ label: "red", wgsl: "@fragment fn fs(in: PostIn) -> @location(0) vec4<f32> { return vec4<f32>(post_color(in.uv).r, 0.0, 0.0, 1.0); }" });
    return { shaders: {}, draws: [] };
  } };
  const a = compileScene(scene({ sea: swell, things: [body, redOnly], style, sun: [0, 1, 0] })).artefact;
  expect(a.post?.map((p) => p.label)).toEqual(["magenta", "red"]);
  const loaded = await createHost(await gpu()).load(a);
  const px = await loaded.frame({ camera: { kind: "perspective", eye: [0, 3, 10], target: [0, 0, 0], fovY: 0.8 }, width: 16, height: 8, time: 0 });
  loaded.destroy();
  for (let i = 0; i < px.length; i += 4) expect([px[i], px[i + 1], px[i + 2]]).toEqual([255, 0, 0]);
});
