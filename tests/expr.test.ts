// The two backends must agree: the same expression evaluated in JavaScript and on the GPU gives the same numbers.
// If they drift, a creature posed on the CPU and drawn on the GPU would disagree with itself.
import { expect, test } from "bun:test";
import { setupGlobals } from "bun-webgpu";
import {
  add, atan2, clamp, compileJs, cos, cross, CURVE_N, emitWgsl, length, lt, mix, mod, mul, normalize, param, pow, row, Row,
  select, sin, smoothstep, step, sub, time, variable, vec3, type Bind, type Expr,
} from "../src/expr.ts";

const u = variable("u", "f32");
const ramp = new Row(Float32Array.from({ length: CURVE_N * 4 }, (_, i) => Math.sin(i * 0.37) * 2), "test ramp");
const girth = param("test.girth", 1.3);
const cases: Expr[] = [
  add(mul(sin(mul(u, 7)), girth), cos(time)),
  mix(vec3(1, 2, 3), vec3(u, mul(u, u), 0.5), smoothstep(0.2, 0.8, u)),
  normalize(cross(vec3(u, 1, 0.3), vec3(0.2, u, 1))).mul(length(vec3(u, 2, 3))),
  mod(sub(mul(u, 10), 3.7), 1.3),
  select(lt(u, 0.5), pow(u, 2.2), atan2(u, 0.3)),
  row(ramp, u).xyz.mul(step(0.25, u)),
  clamp(vec3(u, -1, 4), 0, vec3(0.6, 0.6, 2)),
];

test("JS and WGSL backends agree", async () => {
  await setupGlobals();
  const adapter = await navigator.gpu.requestAdapter();
  const device = await adapter!.requestDevice();
  // 0.52, not 0.5: at 0.5 the `mod` case sits exactly on its discontinuity, where f32 and f64 legitimately land on opposite sides.
  const us = [0.03, 0.31, 0.52, 0.77, 0.98], t = 1.7;
  const bind: Bind = {
    variable: (name) => name,
    input: (spec) => (spec.name === "time" ? "cfg.x" : (() => { throw new Error(spec.name); })()),
    param: () => "cfg.y",
    row: (_r, x) => `curve(${x})`,
  };
  const { lines, out } = emitWgsl(cases.map((c) => (c.ty === "vec3" ? c : vec3(c as Expr<"f32">, 0, 0))), bind);
  const code = `
    @group(0) @binding(0) var<storage, read> rows: array<vec4<f32>>;
    @group(0) @binding(1) var<storage, read_write> result: array<vec4<f32>>;
    @group(0) @binding(2) var<uniform> cfg: vec4<f32>;
    fn curve(x: f32) -> vec4<f32> {
      let s = clamp(x, 0.0, 1.0) * f32(${CURVE_N - 1});
      let i = min(u32(floor(s)), ${CURVE_N - 2}u);
      return mix(rows[i], rows[i + 1u], s - f32(i));
    }
    @compute @workgroup_size(1) fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
      let u = array<f32, ${us.length}>(${us.join(", ")})[gid.x];
      ${lines.join("\n      ")}
      ${out.map((o, i) => `result[gid.x * ${cases.length}u + ${i}u] = vec4<f32>(${o}, 0.0);`).join("\n      ")}
    }`;
  device.pushErrorScope("validation");
  const pipeline = await device.createComputePipelineAsync({ layout: "auto", compute: { module: device.createShaderModule({ code }), entryPoint: "main" } });
  const error = await device.popErrorScope();
  expect(error?.message ?? null).toBeNull();

  const rows = device.createBuffer({ size: ramp.samples.byteLength, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
  device.queue.writeBuffer(rows, 0, ramp.samples);
  const bytes = us.length * cases.length * 16;
  const result = device.createBuffer({ size: bytes, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC });
  const read = device.createBuffer({ size: bytes, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST });
  const cfg = device.createBuffer({ size: 16, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
  device.queue.writeBuffer(cfg, 0, new Float32Array([t, 1.3, 0, 0]));
  const group = device.createBindGroup({ layout: pipeline.getBindGroupLayout(0), entries: [rows, result, cfg].map((buffer, binding) => ({ binding, resource: { buffer } })) });
  const encoder = device.createCommandEncoder();
  const pass = encoder.beginComputePass();
  pass.setPipeline(pipeline); pass.setBindGroup(0, group); pass.dispatchWorkgroups(us.length); pass.end();
  encoder.copyBufferToBuffer(result, 0, read, 0, bytes);
  device.queue.submit([encoder.finish()]);
  await read.mapAsync(GPUMapMode.READ);
  const gpu = new Float32Array(read.getMappedRange().slice(0));

  const run = compileJs(cases);
  let worst = 0;
  us.forEach((uv, k) => {
    const cpu = run({ vars: { u: uv }, inputs: { time: t }, params: {} });
    cases.forEach((c, i) => {
      const want = typeof cpu[i] === "number" ? [cpu[i] as number, 0, 0] : (cpu[i] as number[]);
      for (let j = 0; j < 3; j++) {
        const got = gpu[(k * cases.length + i) * 4 + j];
        const err = Math.abs(got - want[j]) / Math.max(1, Math.abs(want[j]));
        if (err > 1e-4) console.log(`case ${i} u=${uv} comp ${j}: gpu ${got} cpu ${want[j]}`);
        worst = Math.max(worst, err);
      }
    });
  });
  // f32 on the GPU against f64 on the CPU: 1e-4 relative is the floor for single precision through sin/pow/atan2.
  expect(worst).toBeLessThan(1e-4);
});
