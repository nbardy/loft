// GPU time per draw and per fixed stage, from timestamp queries (`GpuLoaded.profile`).
//
// Timestamps are written per pass, so the profiling frame puts every stage in its own pass: each draw in a pass that
// loads and stores the 4x colour and depth, the depth resolve, the multisample resolve, each post pass and the readback
// copy. It is slower than a real frame and is only for measuring.
//
// Two facts about Dawn on Metal (Apple's tile GPUs), measured while writing this, shape the arithmetic:
// - Passes overlap: a pass's begin can precede the previous pass's end (the next vertex work runs while the previous
//   fragment work drains). So a stage's cost is its COMPLETION DELTA, end(k) - end(k - 1): the time it added to the
//   frame. The deltas sum to the frame's GPU time.
// - A pass with no draw is coalesced (its timestamps repeat the previous pass's), so the resolve passes and the
//   brackets round the readback copy each issue one draw whose triangle lies outside the clip volume.
// - Timestamps are quantised to 2^16 ns (65.5 us; Dawn's "timestamp_quantization", which bun-webgpu cannot turn
//   off), so one frame's delta for a small draw is 0 or 65.5 us. A minimum over frames would report 0. Since a pass's
//   boundaries fall at varying phases of that grid, the MEAN of the deltas is unbiased; the machine's load only ever
//   adds time. So frames are taken in blocks, each block's deltas averaged, and the best (least) block mean reported.
// A pass of its own costs loading and storing the 4x attachments; that cost is measured as its own stage ("pass
// overhead": a loaded and stored pass with the empty draw) and `net` subtracts it from each draw.

import { COLOR_FORMAT, DEPTH_FORMAT, POSE_ONE_PASS, SAMPLES, SCENE_DEPTH_FORMAT, drawOne, recordPose, type Plan, type Scene, type Schedule } from "./host.ts";

export type ProfileDraw = {
  readonly label: string;
  /** Completion delta of its pass, best frame. */
  readonly ms: number;
  /** `ms` less the pass overhead (never below 0). */
  readonly net: number;
  /** Triangles it drew and draw calls it issued (what the plan chose). */
  readonly triangles: number;
  readonly instances: number;
  readonly calls: number;
};
export type ProfileStage = { readonly label: string; readonly ms: number };
export type Profile = {
  readonly draws: readonly ProfileDraw[];
  /** clear, depth resolve, multisample resolve, each post pass, readback copy. */
  readonly stages: readonly ProfileStage[];
  /** The pose stages: each in a pass of its own, in order, then all of them in one pass (as a frame runs them, labelled POSE_ONE_PASS). */
  readonly poseStages: readonly ProfileStage[];
  /** A pass that loads and stores the 4x attachments and draws nothing visible. */
  readonly passOverhead: number;
  /** CPU: frame state, `cpu` tables, uploads and the pose stages' submit, and the level-of-detail plan; best frame. */
  readonly pose: number;
  /** Sum of every stage's delta, best block. */
  readonly total: number;
  readonly frames: number;
};

const NOOP_WGSL = /* wgsl */ `
@vertex fn vs(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> { return vec4<f32>(2.0, 2.0, f32(vi), 1.0); }
@fragment fn fs() -> @location(0) vec4<f32> { return vec4<f32>(0.0); }`;

type Noops = { readonly x4Depth: GPURenderPipeline; readonly x4: GPURenderPipeline; readonly x1: GPURenderPipeline };
function noops(device: GPUDevice): Noops {
  const module = device.createShaderModule({ label: "loft profile noop", code: NOOP_WGSL });
  const layout = device.createPipelineLayout({ bindGroupLayouts: [] });
  const make = (count: number, depth: boolean): GPURenderPipeline => device.createRenderPipeline({
    label: "loft profile noop", layout,
    vertex: { module, entryPoint: "vs" },
    fragment: { module, entryPoint: "fs", targets: [{ format: COLOR_FORMAT, writeMask: 0 }] },
    primitive: { topology: "triangle-list" },
    ...(depth ? { depthStencil: { format: DEPTH_FORMAT, depthWriteEnabled: false, depthCompare: "always" as const } } : {}),
    multisample: { count },
  });
  return { x4Depth: make(SAMPLES, true), x4: make(SAMPLES, false), x1: make(1, false) };
}

/** One timed pass. `kind` says which list its delta goes to. */
type Stage = { readonly label: string; readonly kind: "pose" | "draw" | "stage" | "overhead" | "bracket"; readonly index: number };

export type ProfileInput = {
  readonly device: GPUDevice;
  readonly scene: Scene;
  readonly schedule: Schedule;
  readonly size: { readonly width: number; readonly height: number };
  /** Pose a frame (its own submit, waiting for its level-of-detail bounds) and return its plan and CPU milliseconds. */
  readonly upload: () => Promise<{ readonly plan: Plan; readonly poseMs: number }>;
  readonly frames: number;
};

export async function profileFrames(p: ProfileInput): Promise<Profile> {
  const { device, scene, schedule, size } = p;
  if (!device.features.has("timestamp-query")) throw new Error("profile: the device was created without the \"timestamp-query\" feature");
  const n = noops(device);
  const tex = (label: string, format: GPUTextureFormat, sampleCount: number, usage: number) => device.createTexture({ label, size, format, sampleCount, usage });
  const RA = GPUTextureUsage.RENDER_ATTACHMENT, TB = GPUTextureUsage.TEXTURE_BINDING;
  const colour = tex("profile colour x4", COLOR_FORMAT, SAMPLES, RA);
  const depth = tex("profile depth x4", DEPTH_FORMAT, SAMPLES, RA | TB);
  const sceneDepth = tex("profile scene depth", SCENE_DEPTH_FORMAT, 1, RA | TB);
  const out = tex("profile output", COLOR_FORMAT, 1, RA | GPUTextureUsage.COPY_SRC);
  const tiny = device.createTexture({ label: "profile bracket", size: { width: 1, height: 1 }, format: COLOR_FORMAT, usage: RA });
  const bytesPerRow = Math.ceil((size.width * 4) / 256) * 256;
  const readback = device.createBuffer({ label: "profile readback", size: bytesPerRow * size.height, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST });
  const posts = schedule.kind === "split" ? schedule.post.length : 0;
  const intermediates = Array.from({ length: Math.min(posts, 2) }, (_, i) => tex(`profile post ${i}`, COLOR_FORMAT, 1, RA | TB));
  const views = { colour: colour.createView(), depth: depth.createView(), sceneDepth: sceneDepth.createView(), out: out.createView(), tiny: tiny.createView() };
  const groups = schedule.kind === "split" ? {
    resolve: device.createBindGroup({ layout: schedule.resolveLayout, entries: [{ binding: 0, resource: views.depth }] }),
    depth: device.createBindGroup({ layout: schedule.depthLayout, entries: [{ binding: 0, resource: views.sceneDepth }] }),
    post: intermediates.map((t) => device.createBindGroup({
      layout: schedule.postLayout,
      entries: [{ binding: 0, resource: views.sceneDepth }, { binding: 1, resource: t.createView() }, { binding: 2, resource: schedule.sampler }],
    })),
  } : undefined;
  const outputOf = (k: number): GPUTextureView => (k === posts ? views.out : intermediates[k % 2]!.createView());

  const maxStages = 9 + scene.pose.length + scene.opaque.length + scene.transparent.length + posts;
  const querySet = device.createQuerySet({ type: "timestamp", count: 2 * maxStages });
  const resolveBuf = device.createBuffer({ size: 16 * maxStages, usage: GPUBufferUsage.QUERY_RESOLVE | GPUBufferUsage.COPY_SRC });
  const readTs = device.createBuffer({ size: 16 * maxStages, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST });

  const block = Math.max(1, Math.floor(p.frames / 6));
  const sums = new Map<string, number>(), best = new Map<string, number>();
  let bestPose = Infinity, bestTotal = Infinity, sumTotal = 0;
  let shape: { readonly stages: readonly Stage[]; readonly plan: Plan } | undefined;
  for (let f = 0; f < p.frames; f++) {
    const { plan, poseMs } = await p.upload();
    bestPose = Math.min(bestPose, poseMs);
    const stages: Stage[] = [];
    const encoder = device.createCommandEncoder({ label: "loft profile" });
    const timed = (label: string, kind: Stage["kind"]): GPURenderPassTimestampWrites => {
      const index = stages.length;
      stages.push({ label, kind, index });
      return { querySet, beginningOfPassWriteIndex: 2 * index, endOfPassWriteIndex: 2 * index + 1 };
    };
    const x4 = (label: string, kind: Stage["kind"], load: GPULoadOp): GPURenderPassEncoder => encoder.beginRenderPass({
      label, timestampWrites: timed(label, kind),
      colorAttachments: [{ view: views.colour, loadOp: load, storeOp: "store", clearValue: scene.clear }],
      depthStencilAttachment: { view: views.depth, depthClearValue: 1, depthLoadOp: load, depthStoreOp: "store" },
    });
    const noop = (pass: GPURenderPassEncoder, pipeline: GPURenderPipeline) => { pass.setPipeline(pipeline); pass.draw(3); pass.end(); };

    // The pose stages again (the upload ran them untimed to read the bounds; the same inputs give the same tables):
    // once as a frame runs them, every stage in one pass, then once more a pass per stage for the breakdown. A compute
    // pass may overlap the render passes after it, so a pose pass's time is its own duration, not a completion delta.
    if (scene.pose.length > 0) {
      recordPose(encoder, scene, (label) => timed(label, "pose"));
      recordPose(encoder, scene, undefined, timed(POSE_ONE_PASS, "pose"));
    }

    noop(x4("clear", "stage", "clear"), n.x4Depth);
    noop(x4("pass overhead", "overhead", "load"), n.x4Depth);
    for (const d of scene.opaque) {
      const pass = x4(d.draw.label, "draw", "load");
      pass.setBindGroup(0, scene.group0); pass.setBindGroup(1, scene.group1);
      drawOne(pass, d, plan);
      // A plan may issue nothing for a draw; the pass still needs a draw to be timed.
      pass.setPipeline(n.x4Depth); pass.draw(3);
      pass.end();
    }
    if (groups !== undefined && schedule.kind === "split") {
      const resolve = encoder.beginRenderPass({
        label: "depth resolve", timestampWrites: timed("depth resolve", "stage"),
        colorAttachments: [{ view: views.sceneDepth, loadOp: "clear", storeOp: "store", clearValue: { r: 1, g: 0, b: 0, a: 1 } }],
      });
      resolve.setPipeline(schedule.resolve); resolve.setBindGroup(0, groups.resolve); resolve.draw(3); resolve.end();
    }
    for (const d of scene.transparent) {
      const pass = x4(d.draw.label, "draw", "load");
      pass.setBindGroup(0, scene.group0); pass.setBindGroup(1, scene.group1);
      if (groups !== undefined) pass.setBindGroup(2, groups.depth);
      drawOne(pass, d, plan);
      pass.setPipeline(n.x4Depth); pass.draw(3);
      pass.end();
    }
    noop(encoder.beginRenderPass({
      label: "msaa resolve", timestampWrites: timed("multisample resolve", "stage"),
      colorAttachments: [{ view: views.colour, resolveTarget: outputOf(0), loadOp: "load", storeOp: "discard" }],
    }), n.x4);
    if (groups !== undefined && schedule.kind === "split") {
      schedule.post.forEach((pipeline, i) => {
        const k = i + 1;
        const pass = encoder.beginRenderPass({
          label: `post ${i}`, timestampWrites: timed(`post ${i}`, "stage"),
          colorAttachments: [{ view: outputOf(k), loadOp: "clear", storeOp: "store", clearValue: { r: 0, g: 0, b: 0, a: 1 } }],
        });
        pass.setPipeline(pipeline);
        pass.setBindGroup(0, scene.group0); pass.setBindGroup(1, scene.group1); pass.setBindGroup(2, groups.post[(k - 1) % 2]!);
        pass.draw(3); pass.end();
      });
    }
    const bracket = (label: string, kind: Stage["kind"]) => noop(encoder.beginRenderPass({
      label, timestampWrites: timed(label, kind), colorAttachments: [{ view: views.tiny, loadOp: "clear", storeOp: "store", clearValue: { r: 0, g: 0, b: 0, a: 0 } }],
    }), n.x1);
    bracket("readback start", "bracket");
    encoder.copyTextureToBuffer({ texture: out }, { buffer: readback, bytesPerRow, rowsPerImage: size.height }, size);
    bracket("readback copy", "stage");
    encoder.resolveQuerySet(querySet, 0, 2 * stages.length, resolveBuf, 0);
    encoder.copyBufferToBuffer(resolveBuf, 0, readTs, 0, 16 * stages.length);
    device.queue.submit([encoder.finish()]);
    await readTs.mapAsync(GPUMapMode.READ);
    const ts = new BigUint64Array(readTs.getMappedRange().slice(0));
    readTs.unmap();
    await readback.mapAsync(GPUMapMode.READ); readback.unmap();
    const ms = (a: bigint, b: bigint) => Number(b - a) / 1e6;
    let total = 0;
    stages.forEach((s, k) => {
      const delta = k === 0 || s.kind === "pose" ? ms(ts[2 * k]!, ts[2 * k + 1]!) : ms(ts[2 * k - 1]!, ts[2 * k + 1]!);
      total += delta;
      // Labels can repeat (two draws of one shader); keyed by position, which is the same every frame.
      const key = `${k}|${s.label}`;
      sums.set(key, (sums.get(key) ?? 0) + delta);
    });
    sumTotal += total;
    if ((f + 1) % block === 0) {
      for (const [key, v] of sums) best.set(key, Math.min(best.get(key) ?? Infinity, v / block));
      bestTotal = Math.min(bestTotal, sumTotal / block);
      sums.clear(); sumTotal = 0;
    }
    shape = { stages, plan };
  }
  for (const t of [colour, depth, sceneDepth, out, tiny, ...intermediates]) t.destroy();
  for (const b of [readback, resolveBuf, readTs]) b.destroy();
  querySet.destroy();

  const s = shape!;
  const at = (st: Stage) => best.get(`${st.index}|${st.label}`)!;
  const overhead = at(s.stages.find((st) => st.kind === "overhead")!);
  const drawStages = s.stages.filter((st) => st.kind === "draw");
  const all = [...scene.opaque, ...scene.transparent];
  const draws = drawStages.map((st, i): ProfileDraw => {
    const runs = s.plan[all[i]!.index]!;
    const triangles = runs.reduce((t, r) => t + (r.vertices / 3) * r.instances, 0);
    const instances = runs.reduce((t, r) => t + r.instances, 0);
    return { label: st.label, ms: at(st), net: Math.max(0, at(st) - overhead), triangles: Math.round(triangles), instances, calls: runs.length };
  });
  const stages = s.stages.filter((st) => st.kind === "stage").map((st) => ({ label: st.label, ms: at(st) }));
  const poseStages = s.stages.filter((st) => st.kind === "pose").map((st) => ({ label: st.label, ms: at(st) }));
  return { draws, stages, poseStages, passOverhead: overhead, pose: bestPose, total: bestTotal, frames: p.frames };
}
