// The bake of a compiled artefact's recurrences (src/artefact.ts `Recurrence`), on the device that loads it: each
// recurrence's step run from its start to its last step on the GPU, the states to keep written by the steps themselves,
// then read back into the artefact's static data. That data is the truth: `lower` writes it into the files (so a file
// loader never bakes), the frame draws it, and the CPU checks read it. Run once per artefact; a baked one is skipped.
//
// Imports no compiler module and no bun-only one (the browser host uses it).

import { POSE_WORKGROUP, type Artefact, type BakeRecord, type Recurrence } from "../artefact.ts";
import { EnvironmentError } from "../errors.ts";

/** Bake every recurrence of `a` not yet baked, in the artefact's order (each after those it reads). */
export async function bakeRecurrences(device: GPUDevice, a: Artefact): Promise<void> {
  for (const r of a.recurrences ?? []) if (r.baked.done === undefined) await bakeOne(device, a, r);
}

const STORAGE = 0x80, COPY_SRC = 0x4, COPY_DST = 0x8, MAP_READ = 0x1, COMPUTE = 0x4;

async function bakeOne(device: GPUDevice, a: Artefact, r: Recurrence): Promise<void> {
  const t0 = performance.now();
  const stateBytes = (r.entries + 1) * r.lanes * 16;
  const made: GPUBuffer[] = [];
  const buffer = (label: string, size: number, usage: number): GPUBuffer => { const b = device.createBuffer({ label, size, usage }); made.push(b); return b; };
  device.pushErrorScope("validation");
  let scoped = true;
  const refused = async (): Promise<void> => {
    scoped = false;
    const error = await device.popErrorScope();
    if (error) throw new EnvironmentError(`bake "${r.name}": WebGPU refused the bake:\n${error.message}`, true);
  };
  try {
    const pair = [buffer(`bake ${r.name} A`, stateBytes, STORAGE), buffer(`bake ${r.name} B`, stateBytes, STORAGE)] as const;
    const kept = buffer(`bake ${r.name} kept`, r.data.byteLength, STORAGE | COPY_SRC);
    const read = buffer(`bake ${r.name} readback`, r.data.byteLength, MAP_READ | COPY_DST);
    // The static buffers the step reads, as the artefact holds them now (the recurrences it reads already baked in).
    const statics = r.bake.statics.map((name) => {
      const s = (a.statics ?? []).find((x) => x.name === name);
      if (s === undefined) throw new Error(`bake "${r.name}": its step reads the static buffer "${name}", which the artefact lacks`);
      const b = buffer(`bake ${r.name} ${name}`, s.data.byteLength, STORAGE | COPY_DST);
      device.queue.writeBuffer(b, 0, s.data as Float32Array<ArrayBuffer>);
      return b;
    });
    const layout = device.createBindGroupLayout({
      label: `bake ${r.name}`,
      entries: [
        ...[0, 1, 2].map((binding): GPUBindGroupLayoutEntry => ({ binding, visibility: COMPUTE, buffer: { type: "storage" } })),
        ...statics.map((_, k): GPUBindGroupLayoutEntry => ({ binding: 3 + k, visibility: COMPUTE, buffer: { type: "read-only-storage" } })),
      ],
    });
    const module = device.createShaderModule({ label: `bake ${r.name}`, code: r.bake.wgsl });
    const pipelineLayout = device.createPipelineLayout({ label: `bake ${r.name}`, bindGroupLayouts: [layout] });
    const [start, step] = await Promise.all(["recur_start", "recur_step"].map((entryPoint) => device.createComputePipelineAsync({ label: `bake ${r.name} ${entryPoint}`, layout: pipelineLayout, compute: { module, entryPoint } })))
      .catch((e: unknown) => { throw new EnvironmentError(`bake "${r.name}": a pipeline failed to build:\n${e instanceof Error ? e.message : String(e)}`, true); });
    const group = (src: GPUBuffer, dst: GPUBuffer): GPUBindGroup => device.createBindGroup({
      layout,
      entries: [src, dst, kept, ...statics].map((b, binding) => ({ binding, resource: { buffer: b } })),
    });
    // Ping-pong: state a lives in A when a is even, in B when it is odd (start writes state 0 into A).
    const intoA = group(pair[1], pair[0]), intoB = group(pair[0], pair[1]);
    const setup = performance.now() - t0;

    const t1 = performance.now();
    const workgroups = Math.ceil(r.entries / POSE_WORKGROUP);
    const encoder = device.createCommandEncoder({ label: `bake ${r.name}` });
    const timed = device.features.has("timestamp-query");
    const stamps = timed ? device.createQuerySet({ type: "timestamp", count: 2 }) : undefined;
    const stampBuf = timed ? buffer(`bake ${r.name} stamps`, 16, 0x200 | COPY_SRC) : undefined; // QUERY_RESOLVE
    const stampRead = timed ? buffer(`bake ${r.name} stamps read`, 16, MAP_READ | COPY_DST) : undefined;
    const pass = encoder.beginComputePass({ label: `bake ${r.name}`, ...(stamps === undefined ? {} : { timestampWrites: { querySet: stamps, beginningOfPassWriteIndex: 0, endOfPassWriteIndex: 1 } }) });
    pass.setPipeline(start);
    pass.setBindGroup(0, intoA);
    pass.dispatchWorkgroups(workgroups);
    pass.setPipeline(step);
    // Dispatches in one pass are ordered: each sees what the one before it wrote. Its step number is in the state
    // itself (the clock entry, src/compile/recur.ts), not written between dispatches.
    for (let a = 0; a < r.total; a++) {
      pass.setBindGroup(0, a % 2 === 0 ? intoB : intoA);
      pass.dispatchWorkgroups(workgroups);
    }
    pass.end();
    if (stamps !== undefined) { encoder.resolveQuerySet(stamps, 0, 2, stampBuf!, 0); encoder.copyBufferToBuffer(stampBuf!, 0, stampRead!, 0, 16); }
    device.queue.submit([encoder.finish()]);
    await device.queue.onSubmittedWorkDone();
    const ms = performance.now() - t1;
    const t2 = performance.now();
    const copy = device.createCommandEncoder({ label: `bake ${r.name} readback` });
    copy.copyBufferToBuffer(kept, 0, read, 0, r.data.byteLength);
    device.queue.submit([copy.finish()]);
    await refused();
    await read.mapAsync(GPUMapMode.READ);
    r.data.set(new Float32Array(read.getMappedRange()));
    read.unmap();
    const readback = performance.now() - t2;
    let gpu: BakeRecord["gpu"] = { kind: "untimed" };
    if (stampRead !== undefined) {
      await stampRead.mapAsync(GPUMapMode.READ);
      const ts = new BigUint64Array(stampRead.getMappedRange().slice(0));
      stampRead.unmap();
      gpu = { kind: "timed", ms: Number(ts[1]! - ts[0]!) / 1e6 };
      stamps!.destroy();
    }
    r.baked.mark({ ms, readback, setup, steps: r.total, bytes: r.data.byteLength, gpu });
  } catch (e) {
    // A validation error the GPU raised says more than what it made fail.
    if (scoped) await refused();
    throw e;
  } finally {
    for (const b of made) b.destroy();
  }
}
