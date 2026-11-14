// Still mode (docs/proposals/headroom.md 5.4): a picture that averages many samples of one instant, for `loft render`,
// `frames` and `clip` with `--samples N` (and `--shutter s`, `--aperture r`). The TypeScript host and the command line
// only: the file format, the loader and every normal frame are unchanged.
//
// Sample k of N is today's frame at its own time, its own sub-pixel offset and its own point on the lens, all functions
// of (t, k) alone: no state, nothing carried from one frame to the next, the same bytes from a fresh process.
//   time    t + s (k + 0.5) / N: exact motion blur with no motion vectors, since the scene is a function of time.
//   pixel   the projection moved inside the pixel by the k-th point of the (2, 3) Halton sequence (index k + 1).
//   lens    the eye moved to the k-th point of a disc of radius r (Vogel's spiral: radius sqrt((k + 0.5) / N), angle k
//           times the golden angle), the frustum sheared so the plane through the camera's target stays where it was
//           (an off-axis thin lens, Haeberli and Akeley 1990): the target's plane is sharp, nearer and farther blur.
// Each sample is resolved as today (rgba8unorm, display sRGB, as the 4x multisample resolve averages) and added into
// an f32 accumulator by a compute pass; the average is written where the scene's picture goes and the style's post
// passes run once over it. Samples are drawn N - 1 down to 0, so the post passes read sample 0's depth.
// Each sample plans its level of detail afresh (no hysteresis memory), so a still does not depend on frames before it.

import type { Camera, FrameStats, View } from "../artefact.ts";
import { cameraMatrices, invert, type Mat4 } from "./camera.ts";
import type { Plan } from "./detail.ts";
import { FRAME_FLOATS } from "./lower.ts";
import { COLOR_FORMAT, type RenderPassDesc } from "./manifest.ts";

/** How a frame is drawn: today's frame, or a still of `samples` samples over `shutter` seconds through a lens of radius `aperture`. */
export type StillSpec =
  | { readonly kind: "frame" }
  | { readonly kind: "still"; readonly samples: number; readonly shutter: number; readonly aperture: number };

/** The k-th element of the van der Corput sequence in base b (Halton's coordinate). */
function radicalInverse(k: number, b: number): number {
  let f = 1, r = 0;
  for (let i = k; i > 0; i = Math.floor(i / b)) { f /= b; r += f * (i % b); }
  return r;
}

const GOLDEN_ANGLE = Math.PI * (3 - Math.sqrt(5));

/** Sample k of n: its time after t, its pixel offset (pixels, x right, y down) and its point on the unit lens disc. */
export function sampleOf(k: number, n: number, s: Extract<StillSpec, { kind: "still" }>): { readonly dt: number; readonly pixel: readonly [number, number]; readonly lens: readonly [number, number] } {
  const r = Math.sqrt((k + 0.5) / n), a = k * GOLDEN_ANGLE;
  return {
    dt: (s.shutter * (k + 0.5)) / n,
    pixel: [radicalInverse(k + 1, 2) - 0.5, radicalInverse(k + 1, 3) - 0.5],
    lens: [r * Math.cos(a), r * Math.sin(a)],
  };
}

/**
 * The k-th point of the R2 sequence (Roberts 2018), the sample a light's `xi` takes in a still. MERGE(light): when the
 * light proposal lands, a still compiles the scene with `xi` given to lights as this point (a host input, say
 * `loft_xi`, set per sample in `stillFrame` below where the sample's view is made), rotated per pixel in the shader by a
 * hash of the pixel (Cranley-Patterson: fract(r2(k) + hash2(pixel))). A normal frame never holds that branch.
 */
export function r2(k: number): readonly [number, number] {
  const g = 1.32471795724474602596;
  return [(0.5 + k / g) % 1, (0.5 + k / (g * g)) % 1];
}

/**
 * The 48-float frame uniform of sample k: the view's camera with its eye on the lens and the frustum sheared to keep
 * the target's plane, then offset inside the pixel (src/host/host.ts `frameUniform` is the unmoved case).
 */
export function sampleUniform(view: View, k: number, n: number, s: Extract<StillSpec, { kind: "still" }>): Float32Array<ArrayBuffer> {
  const { pixel, lens } = sampleOf(k, n, s);
  const c = view.camera;
  // The command line refuses this with the view's name (src/cli/still.ts); here it is a caller's mistake.
  if (s.aperture > 0 && c.kind !== "perspective") throw new Error(`still: an aperture needs a perspective camera; this one is ${c.kind}`);
  const base = cameraMatrices(c, view.width / view.height);
  const ref = Math.abs(base.forward[1]) > 0.999999 ? [0, 0, -Math.sign(base.forward[1])] : [0, 1, 0];
  const f = base.forward;
  const right = unit([f[1] * ref[2]! - f[2] * ref[1]!, f[2] * ref[0]! - f[0] * ref[2]!, f[0] * ref[1]! - f[1] * ref[0]!]);
  const up = [right[1] * f[2] - right[2] * f[1], right[2] * f[0] - right[0] * f[2], right[0] * f[1] - right[1] * f[0]];
  const shift = [0, 1, 2].map((i) => s.aperture * (lens[0] * right[i]! + lens[1] * up[i]!));
  const moved: Camera = { ...c, eye: add(c.eye, shift), target: add(c.target, shift) };
  const m = cameraMatrices(moved, view.width / view.height);
  const vp: Mat4 = Float64Array.from(m.viewProj);
  // Shear: a point of the target's plane (clip w = d) moved by -shift in view space lands back where it was, and the
  // whole picture moves by the pixel offset (NDC y is up, pixels y down). Added as multiples of clip w's row.
  // The projection's x and y scales are the lengths of view_proj's rows 0 and 1 (each a unit row of the view's rotation
  // times the scale, as `pixel_world` reads row 1), not their first elements.
  const d = Math.hypot(...[0, 1, 2].map((i) => c.target[i]! - c.eye[i]!));
  const sx = Math.hypot(base.viewProj[0]!, base.viewProj[4]!, base.viewProj[8]!), sy = Math.hypot(base.viewProj[1]!, base.viewProj[5]!, base.viewProj[9]!);
  const ax = (s.aperture * lens[0] * sx) / d + (2 * pixel[0]) / view.width;
  const ay = (s.aperture * lens[1] * sy) / d - (2 * pixel[1]) / view.height;
  for (let col = 0; col < 4; col++) {
    vp[col * 4]! += ax * vp[col * 4 + 3]!;
    vp[col * 4 + 1]! += ay * vp[col * 4 + 3]!;
  }
  const u = new Float32Array(FRAME_FLOATS);
  u.set(vp, 0);
  u.set(invert(vp), 16);
  u.set([...m.eye, m.perspective], 32);
  u.set([...m.forward, 0], 36);
  u.set([view.width, view.height, 1 / view.width, 1 / view.height], 40);
  u.set([view.time, 0, 0, 0], 44);
  return u;
}
const unit = (v: readonly number[]): number[] => { const l = Math.hypot(...v); return v.map((x) => x / l); };
const add = (a: readonly [number, number, number], b: readonly number[]): [number, number, number] => [a[0] + b[0]!, a[1] + b[1]!, a[2] + b[2]!];

/** What a still needs of the host that draws it (src/host/host.ts makes one per frame). */
export type StillKit = {
  readonly device: GPUDevice;
  readonly passes: readonly RenderPassDesc[];
  /** The view's texture views by manifest id, for this view's size. */
  readonly views: readonly GPUTextureView[];
  readonly textures: readonly GPUTexture[];
  /** The frame's output (the readback texture). */
  readonly out: GPUTexture;
  /** Pose the view with this frame uniform, wait for its level-of-detail bounds, plan with no memory. */
  sample(view: View, frame: Float32Array<ArrayBuffer>): Promise<Plan>;
  /** Record passes [from, to) of the frame into the encoder. */
  record(encoder: GPUCommandEncoder, out: GPUTextureView, plan: Plan, from: number, to: number): void;
};

const ACCUMULATE_WGSL = /* wgsl */ `
@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> sum: array<vec4<f32>>;
@compute @workgroup_size(8, 8) fn add(@builtin(global_invocation_id) g: vec3<u32>) {
  let size = textureDimensions(picture);
  if (g.x >= size.x || g.y >= size.y) { return; }
  let i = g.y * size.x + g.x;
  sum[i] = sum[i] + textureLoad(picture, vec2<i32>(g.xy), 0);
}`;
const AVERAGE_WGSL = /* wgsl */ `
@group(0) @binding(0) var<storage, read> sum: array<vec4<f32>>;
@group(0) @binding(1) var<uniform> shape: vec4<f32>;  // width, 1 / samples
@vertex fn vs(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
  let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
  return vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
}
@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
  return sum[u32(p.y) * u32(shape.x) + u32(p.x)] * shape.y;
}`;

const pipelines = new WeakMap<GPUDevice, { readonly accumulate: GPUComputePipeline; readonly average: GPURenderPipeline }>();
function pipelinesOf(device: GPUDevice) {
  const known = pipelines.get(device);
  if (known) return known;
  const made = {
    accumulate: device.createComputePipeline({ label: "loft still accumulate", layout: "auto", compute: { module: device.createShaderModule({ label: "loft still accumulate", code: ACCUMULATE_WGSL }), entryPoint: "add" } }),
    average: device.createRenderPipeline({
      label: "loft still average", layout: "auto",
      vertex: { module: device.createShaderModule({ label: "loft still average", code: AVERAGE_WGSL }), entryPoint: "vs" },
      fragment: { module: device.createShaderModule({ label: "loft still average (fs)", code: AVERAGE_WGSL }), entryPoint: "fs", targets: [{ format: COLOR_FORMAT }] },
      primitive: { topology: "triangle-list" },
    }),
  };
  pipelines.set(device, made);
  return made;
}

/**
 * Draws a still into `kit.out`: every sample's scene passes, each resolved picture added into the accumulator, the
 * average where the scene's picture goes, then the post passes once. Returns the first sample's plan's statistics.
 */
export async function stillFrame(kit: StillKit, view: View, s: Extract<StillSpec, { kind: "still" }>): Promise<FrameStats> {
  const { device, passes } = kit;
  if (!(Number.isInteger(s.samples) && s.samples >= 1)) throw new Error(`still: ${s.samples} samples is not a whole number of at least 1`);
  // The scene's last pass is the last one that resolves (a pass's own colour passes resolve too, but are drawn
  // before the view); everything after it is a post pass. Its resolve target is where the scene's picture goes.
  let last = -1;
  passes.forEach((q, i) => { if (q.color?.resolveTarget !== undefined) last = i; });
  const into = passes[last]?.color?.resolveTarget;
  if (into === undefined) throw new Error("still: the frame has no pass that resolves the scene's picture");
  const { width, height } = view;
  const p = pipelinesOf(device);
  const sum = device.createBuffer({ label: "loft still sum", size: width * height * 16, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
  const shape = device.createBuffer({ label: "loft still shape", size: 16, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
  device.queue.writeBuffer(shape, 0, new Float32Array([width, 1 / s.samples, 0, 0]));
  // Where a sample's picture lands: an intermediate the post passes read, or (with no post) a texture of our own.
  const own = into.kind === "output" ? device.createTexture({ label: "loft still sample", size: { width, height }, format: COLOR_FORMAT, usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING }) : undefined;
  const picture = own ?? kit.textures[(into as { readonly texture: number }).texture]!;
  const pictureView = own?.createView() ?? kit.views[(into as { readonly texture: number }).texture]!;
  const outView = kit.out.createView();
  const accumulate = device.createBindGroup({ layout: p.accumulate.getBindGroupLayout(0), entries: [{ binding: 0, resource: picture.createView() }, { binding: 1, resource: { buffer: sum } }] });
  const average = device.createBindGroup({ layout: p.average.getBindGroupLayout(0), entries: [{ binding: 0, resource: { buffer: sum } }, { binding: 1, resource: { buffer: shape } }] });
  let first: Plan | undefined;
  try {
    for (let k = s.samples - 1; k >= 0; k--) {
      const { dt } = sampleOf(k, s.samples, s);
      // MERGE(light): the sample's light point, r2(k), joins the view here as an input once lights take `xi`.
      const sampled: View = { ...view, time: view.time + dt };
      const plan = await kit.sample(sampled, sampleUniform(sampled, k, s.samples, s));
      if (k === 0) first = plan;
      const encoder = device.createCommandEncoder({ label: `loft still sample ${k}` });
      if (k === s.samples - 1) encoder.clearBuffer(sum);
      kit.record(encoder, own === undefined ? outView : pictureView, plan, 0, last + 1);
      const pass = encoder.beginComputePass({ label: "loft still accumulate" });
      pass.setPipeline(p.accumulate);
      pass.setBindGroup(0, accumulate);
      pass.dispatchWorkgroups(Math.ceil(width / 8), Math.ceil(height / 8));
      pass.end();
      device.queue.submit([encoder.finish()]);
    }
    const encoder = device.createCommandEncoder({ label: "loft still average" });
    const avg = encoder.beginRenderPass({ label: "loft still average", colorAttachments: [{ view: own === undefined ? pictureView : outView, loadOp: "clear", storeOp: "store", clearValue: { r: 0, g: 0, b: 0, a: 1 } }] });
    avg.setPipeline(p.average);
    avg.setBindGroup(0, average);
    avg.draw(3);
    avg.end();
    // The post passes, once, over the average (they read sample 0's depth: it was drawn last).
    kit.record(encoder, outView, first!, last + 1, passes.length);
    device.queue.submit([encoder.finish()]);
    await device.queue.onSubmittedWorkDone();
  } finally {
    sum.destroy(); shape.destroy(); own?.destroy();
  }
  let triangles = 0, draws = 0;
  for (const runs of first!) for (const r of runs) { triangles += (r.vertices / 3) * r.instances; draws++; }
  return { triangles: Math.round(triangles), draws };
}
