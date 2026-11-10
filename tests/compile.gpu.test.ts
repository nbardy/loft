// The compiler's output on a real GPU device (bun-webgpu / Dawn).
import { beforeAll, expect, test } from "bun:test";
import { setupGlobals } from "bun-webgpu";
import { PRELUDE_WGSL, type Artefact, type FrameState } from "../src/artefact.ts";
import serpent from "../examples/serpent.ts";
import { compileJs, variable, type Env } from "../src/expr.ts";
import { stationAt, STATION_VEC4S } from "../src/geom/stations.ts";
import { FRAME_VARS } from "../src/geom/strand.ts";
import { add, compile, keys, mul, param, scene, sea, strand, through, time, toon, vec3, wave } from "../src/index.ts";

let device: GPUDevice;
beforeAll(async () => {
  await setupGlobals();
  device = await (await navigator.gpu.requestAdapter())!.requestDevice();
});

const VERTEX = 0x1, FRAGMENT = 0x2, COMPUTE = 0x4;

/** The host contract's explicit layouts (artefact.ts header): every binding of groups 0 and 1 visible everywhere. */
function layouts(a: Artefact, visibility: number, extra: GPUBindGroupLayoutEntry[] = []): GPUBindGroupLayout[] {
  const storage = (binding: number): GPUBindGroupLayoutEntry => ({ binding, visibility, buffer: { type: "read-only-storage" } });
  const g0 = device.createBindGroupLayout({ entries: [{ binding: 0, visibility, buffer: { type: "uniform" } }, storage(1), storage(2), storage(3)] });
  const g1 = device.createBindGroupLayout({ entries: a.dynamic.map((d) => storage(d.binding)) });
  return extra.length ? [g0, g1, device.createBindGroupLayout({ entries: extra })] : [g0, g1];
}

// Catches: any WGSL the compiler generates that Dawn rejects (syntax, types, uniformity of fwidth, override use), and
// any draw whose state or constants cannot make a pipeline, before a host ever loads the artefact.
test("every serpent shader compiles and every draw makes a pipeline under the contract's layouts", async () => {
  const a = compile(serpent);
  const layout = device.createPipelineLayout({ bindGroupLayouts: layouts(a, VERTEX | FRAGMENT) });
  const errors: string[] = [];
  for (const d of a.draws) {
    device.pushErrorScope("validation");
    const module = device.createShaderModule({ code: PRELUDE_WGSL + a.sceneWgsl + a.shaders[d.shader] });
    const blend = d.blend === "alpha" ? { color: { srcFactor: "src-alpha", dstFactor: "one-minus-src-alpha" }, alpha: { srcFactor: "one", dstFactor: "one-minus-src-alpha" } } as const : undefined;
    try {
      await device.createRenderPipelineAsync({
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
    const err = await device.popErrorScope();
    if (err) errors.push(`${d.label}: ${err.message}`);
  }
  expect(errors).toEqual([]);
});

// A strand exercising everything the surface depends on: a moving spine (time, a param, a wave), a twist, both tip
// kinds, a keyed aspect, a boxy section, and few stations (so interpolation between them matters).
const probeGirth = keys([[0, 0.4], [0.5, 0.9], [1, 0.3]]);
const probe = strand({
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
const probeScene = scene({ sea: sea({ swell: [{ direction: [1, 0], length: 10, height: 0.2, speed: 1 }] }), things: [probe], style: toon(), sun: [0.3, 0.8, 0.2] });

function stateAt(a: Artefact, t: number): FrameState {
  const inputs = new Float32Array(Math.max(1, a.inputFloats));
  for (const s of a.inputs) inputs.set(s.name === "time" ? [t] : s.def, s.slot);
  return { time: t, inputs, params: Float32Array.from(a.params.map((p) => p.def)) };
}

const storageBuffer = (data: Float32Array) => {
  const b = device.createBuffer({ size: Math.max(16, data.byteLength), usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
  if (data.byteLength) device.queue.writeBuffer(b, 0, data as Float32Array<ArrayBuffer>);
  return b;
};

// Catches: the CPU pose and the GPU surface disagreeing with each other: a WGSL `station_at` that interpolates
// differently from its JavaScript mirror, a quaternion convention flipped on one side, a slot or curve row bound to the
// wrong place, or an emitted surface expression that differs from the one the CPU evaluates.
test("the vertex-stage surface equals the CPU evaluation of the same body to 1e-4", async () => {
  const a = compile(probeScene);
  const t = 1.37;
  const tables = a.pose(stateAt(a, t));
  const samples: [number, number][] = [];
  for (const m of [0, 0.003, 0.05, 0.21, 0.5, 0.77, 0.94, 0.999, 1]) for (const v of [0.25, 0.4, 0.61, 0.9, 1.13]) samples.push([m, v]);
  const code = PRELUDE_WGSL + a.sceneWgsl + a.shaders["strand.probe"] + /* wgsl */ `
@group(2) @binding(0) var<storage, read_write> result: array<vec4<f32>>;
@group(2) @binding(1) var<storage, read> probe_mv: array<vec2<f32>>;
@compute @workgroup_size(1) fn probe_main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let mv = probe_mv[gid.x];
  let s = surface_probe(mv.x, mv.y);
  result[2u * gid.x] = vec4<f32>(s.pos, 0.0);
  result[2u * gid.x + 1u] = vec4<f32>(s.normal, 0.0);
}`;
  const extra: GPUBindGroupLayoutEntry[] = [{ binding: 0, visibility: COMPUTE, buffer: { type: "storage" } }, { binding: 1, visibility: COMPUTE, buffer: { type: "read-only-storage" } }];
  const [g0, g1, g2] = layouts(a, COMPUTE, extra);
  device.pushErrorScope("validation");
  const pipeline = await device.createComputePipelineAsync({
    layout: device.createPipelineLayout({ bindGroupLayouts: [g0, g1, g2] }),
    compute: { module: device.createShaderModule({ code }), entryPoint: "probe_main" },
  });
  expect((await device.popErrorScope())?.message ?? null).toBeNull();

  const state = stateAt(a, t);
  const frameBuf = device.createBuffer({ size: 48 * 4, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
  const bytes = samples.length * 2 * 16;
  const result = device.createBuffer({ size: bytes, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC });
  const read = device.createBuffer({ size: bytes, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST });
  const group = (layout: GPUBindGroupLayout, buffers: GPUBuffer[]) => device.createBindGroup({ layout, entries: buffers.map((buffer, binding) => ({ binding, resource: { buffer } })) });
  const encoder = device.createCommandEncoder();
  const pass = encoder.beginComputePass();
  pass.setPipeline(pipeline);
  pass.setBindGroup(0, group(g0, [frameBuf, storageBuffer(state.inputs), storageBuffer(state.params), storageBuffer(a.curves)]));
  pass.setBindGroup(1, device.createBindGroup({ layout: g1, entries: a.dynamic.map((d) => ({ binding: d.binding, resource: { buffer: storageBuffer(tables[d.name]) } })) }));
  pass.setBindGroup(2, group(g2, [result, storageBuffer(Float32Array.from(samples.flat()))]));
  pass.dispatchWorkgroups(samples.length);
  pass.end();
  encoder.copyBufferToBuffer(result, 0, read, 0, bytes);
  device.queue.submit([encoder.finish()]);
  await read.mapAsync(GPUMapMode.READ);
  const gpu = new Float32Array(read.getMappedRange().slice(0));

  // The CPU side: the JavaScript mirror of station_at, then the same surface expression through the JS backend.
  const m = variable("m", "f32"), v = variable("v", "f32");
  const surf = probe.surface(FRAME_VARS, m, v);
  const run = compileJs([surf.pos, surf.normal]);
  const station = a.dynamic.find((d) => d.name === "stations")!;
  expect(station.vec4s).toBe(probe.stations * STATION_VEC4S);
  const params: Record<string, number> = Object.fromEntries(a.params.map((p) => [p.name, p.def]));
  let worstPos = 0, worstNormal = 0;
  samples.forEach(([mm, vv], i) => {
    const st = stationAt(tables.stations, 0, probe.stations, mm);
    const env: Env = { vars: { m: mm, v: vv, st_p: st.p, st_der: st.der, st_t: st.t, st_d: st.d, st_s: st.s, st_stretch: st.stretch }, inputs: { time: t }, params };
    const [pos, normal] = run(env) as [number[], number[]];
    for (let c = 0; c < 3; c++) {
      const e = Math.abs(gpu[8 * i + c] - pos[c]) / Math.max(1, Math.abs(pos[c]));
      worstPos = Math.max(worstPos, e);
      worstNormal = Math.max(worstNormal, Math.abs(gpu[8 * i + 4 + c] - normal[c]));
    }
  });
  expect(worstPos).toBeLessThan(1e-4);
  // Normals near a closing tip come from a finite difference of radii (cancellation in f32), so 1e-3 absolute.
  expect(worstNormal).toBeLessThan(1e-3);
});
