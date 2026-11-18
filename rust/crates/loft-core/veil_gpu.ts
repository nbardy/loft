// Lane 3.1's GPU check (lean Rust plan §8.2): specks inside a draw.volume are dimmed by the medium in front of them.
// Renders the specks module that `draws::dump_veiled_specks` writes, once as lowered and once with `veil` = 1, on Dawn,
// and compares each speck's dimming with the transmittance integrated on the CPU (4,000 midpoint steps).
//   LR31_DUMP=out/lr31/specks.wgsl tools/dev.sh test loft-core -E 'test(dump_veiled)' && bun rust/crates/loft-core/veil_gpu.ts out/lr31/specks.wgsl
import { headlessDevice } from "../../../src/host/headless.ts";

const text = await Bun.file(process.argv[2]!).text();
const W = 512, N = 60;
const device = await headlessDevice();
// An orthographic view down -z from z = 4: x in [-2, 2], y in [-1, 3], depth 0..1 over z 4..-4 (column-major).
const clip = [0.5, 0, 0, 0, 0, 0.5, 0, 0, 0, 0, -0.125, 0, 0, -0.5, 0.5, 1];
const world = [2, 0, 0, 0, 0, 2, 0, 0, 0, 0, -8, 0, 0, 1, 4, 1];
const view = new Float32Array([...clip, ...world, W, W, 1 / W, 1 / W, 0, 1, 0, 0]);
const buf = (data: Float32Array) => {
  const b = device.createBuffer({ size: data.byteLength, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
  device.queue.writeBuffer(b, 0, data);
  return b;
};
const viewBuf = buf(view);

async function render(src: string): Promise<Uint8Array> {
  device.pushErrorScope("validation"); // bun-webgpu 0.1.7 has no getCompilationInfo
  const module = device.createShaderModule({ code: `alias lf = f32;\n${src}` });
  const pipeline = device.createRenderPipeline({
    layout: "auto",
    vertex: { module, entryPoint: "vs" },
    fragment: { module, entryPoint: "fs", targets: [{ format: "rgba8unorm" }] },
    primitive: { topology: "triangle-list" },
  });
  const target = device.createTexture({ size: [W, W], format: "rgba8unorm", usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC });
  const read = device.createBuffer({ size: W * W * 4, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
  const enc = device.createCommandEncoder();
  const pass = enc.beginRenderPass({ colorAttachments: [{ view: target.createView(), loadOp: "clear", storeOp: "store", clearValue: [0, 0, 0, 0] }] });
  pass.setPipeline(pipeline);
  pass.setBindGroup(0, device.createBindGroup({ layout: pipeline.getBindGroupLayout(0), entries: [{ binding: 1, resource: { buffer: viewBuf } }] })); // the specks read no time: auto layout drops the frame
  const refused = await device.popErrorScope();
  if (refused) throw new Error(refused.message);
  pass.draw(6, N);
  pass.end();
  enc.copyTextureToBuffer({ texture: target }, { buffer: read, bytesPerRow: W * 4 }, [W, W]);
  device.queue.submit([enc.finish()]);
  await read.mapAsync(GPUMapMode.READ);
  return new Uint8Array(read.getMappedRange().slice(0));
}

const veiled = await render(text);
const plain = await render(text.replace(/^fn veil\(.*$/m, "fn veil(p: vec3<f32>, back: vec3<f32>, reach: f32) -> f32 { return 1.0; }"));
const density = (x: number, y: number, z: number) => Math.max(1.5 * (1.2 - Math.hypot(x, y - 1, z)), 0);
let worst = 0, sum = 0, inside = 0, minShare = 1;
for (let i = 0; i < N; i++) {
  const a = 0.10472 * i, x = Math.cos(a), y = 1 + Math.sin(a), z = 0.5 * Math.sin(3 * a);
  const steps = 4000, dt = (2 - z) / steps; // toward the eye (+z) to the medium's far side z = 2
  let tau = 0;
  for (let k = 0; k < steps; k++) tau += density(x, y, z + (k + 0.5) * dt) * dt;
  const want = Math.exp(-tau);
  const px = Math.round((x / 4 + 0.5) * W - 0.5), py = Math.round((0.5 - (y - 1) / 4) * W - 0.5);
  const at = (py * W + px) * 4;
  const got = veiled[at]! / Math.max(plain[at]!, 1);
  worst = Math.max(worst, Math.abs(got - want));
  if (process.env.LR31_VERBOSE) console.log(i, px, py, veiled[at], plain[at], got.toFixed(3), want.toFixed(3));
  sum += 1 - got;
  if (want < 0.999) inside++;
  minShare = Math.min(minShare, got);
}
console.log(JSON.stringify({ specks: N, veiled_by_medium: inside, mean_dimming: +(sum / N).toFixed(4), darkest: +minShare.toFixed(4), worst_abs_vs_cpu: +worst.toFixed(4) }));
process.exit(worst <= 3 / 255 + 0.01 && inside > 0 ? 0 : 1);
