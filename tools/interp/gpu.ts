export const size = 512;
const format = "rgba32float";
export function layout(device: GPUDevice) {
  const group = (types: GPUBufferBindingType[]) => device.createBindGroupLayout({
    entries: types.map((type, binding) => ({ binding, visibility: GPUShaderStage.FRAGMENT, buffer: { type } })),
  });
  const frame = group(["uniform", "read-only-storage"]), program = group(["read-only-storage", "read-only-storage"]);
  const compiled = device.createPipelineLayout({ bindGroupLayouts: [frame] });
  const interpreted = device.createPipelineLayout({ bindGroupLayouts: [frame, program] });
  return { frame, program, compiled, interpreted };
}

export async function pipeline(d: GPUDevice, layout: GPUPipelineLayout, source: string) {
  const start = performance.now();
  d.pushErrorScope("validation");
  const module = d.createShaderModule({ code: source });
  const moduleError = await d.popErrorScope();
  if (moduleError) throw new Error(moduleError.message);
  d.pushErrorScope("validation");
  const result = await d.createRenderPipelineAsync({ layout, vertex: { module, entryPoint: "vs" },
    fragment: { module, entryPoint: "fs", targets: [{ format }] } });
  const error = await d.popErrorScope();
  if (error) throw new Error(error.message);
  return { result, ms: performance.now() - start };
}

export function upload(d: GPUDevice, data: Float32Array<ArrayBuffer> | Uint32Array<ArrayBuffer>, usage: number) {
  const b = d.createBuffer({ size: Math.max(16, data.byteLength), usage: usage | GPUBufferUsage.COPY_DST });
  if (data.byteLength) d.queue.writeBuffer(b, 0, data);
  return b;
}
export function group(d: GPUDevice, layout: GPUBindGroupLayout, buffers: GPUBuffer[]) {
  return d.createBindGroup({ layout, entries: buffers.map((buffer, binding) => ({ binding, resource: { buffer } })) });
}

export type Draw = { pipeline: GPURenderPipeline; groups: GPUBindGroup[] };
export function instrument(d: GPUDevice) {
  const target = d.createTexture({ size: [size, size], format,
    usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC });
  const view = target.createView();
  const query = d.features.has("timestamp-query") ? d.createQuerySet({ type: "timestamp", count: 2 }) : undefined;
  const resolve = query && d.createBuffer({ size: 16, usage: GPUBufferUsage.QUERY_RESOLVE | GPUBufferUsage.COPY_SRC });
  const stamps = d.createBuffer({ size: 16, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST });
  const pixels = d.createBuffer({ size: size * size * 16, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST });
  function encode(e: GPUCommandEncoder, draw: Draw, timestampWrites?: GPURenderPassTimestampWrites) {
    const pass = e.beginRenderPass({ colorAttachments: [{ view, clearValue: [0,0,0,0], loadOp: "clear", storeOp: "store" }], timestampWrites });
    pass.setPipeline(draw.pipeline);
    draw.groups.forEach((g, k) => pass.setBindGroup(k, g));
    pass.draw(3);
    pass.end();
  }
  async function time(draw: Draw, count: number) {
    const e = d.createCommandEncoder();
    for (let i = 0; i < count; ++i) {
      const edge = i === 0 || i === count - 1;
      encode(e, draw, query && edge ? { querySet: query,
        ...(i === 0 && { beginningOfPassWriteIndex: 0 }), ...(i === count - 1 && { endOfPassWriteIndex: 1 }) } : undefined);
    }
    if (query && resolve) {
      e.resolveQuerySet(query, 0, 2, resolve, 0);
      e.copyBufferToBuffer(resolve, 0, stamps, 0, 16);
    }
    const commands = e.finish(), start = performance.now();
    d.queue.submit([commands]);
    if (!query) {
      await d.queue.onSubmittedWorkDone();
      return (performance.now() - start) / count;
    }
    await stamps.mapAsync(GPUMapMode.READ);
    const t = new BigUint64Array(stamps.getMappedRange());
    const ms = Number(t[1] - t[0]) / 1e6;
    stamps.unmap();
    if (!(ms >= 0)) throw new Error("invalid GPU timestamps");
    return ms / count;
  }
  async function read(draw: Draw) {
    const e = d.createCommandEncoder();
    encode(e, draw);
    e.copyTextureToBuffer({ texture: target }, { buffer: pixels, bytesPerRow: size * 16, rowsPerImage: size }, [size, size]);
    d.queue.submit([e.finish()]);
    await pixels.mapAsync(GPUMapMode.READ);
    const data = new Float32Array(pixels.getMappedRange().slice(0));
    pixels.unmap();
    return data;
  }
  return { time, read, clock: query ? "gpu" : "queue-wall" };
}
