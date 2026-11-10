// The wake: foam on the water where a body crossed the surface in the recent past (DESIGN.md 8), from the body taken
// at earlier times (`body.skeletonAt(time - k dt)`), never from state kept between frames.
//
// The pose pass evaluates the skeleton of each past slice and the sea's height at that slice's time. Where a joint's
// sphere cuts the water, its waterline is a disc of radius sqrt(r^2 - h^2); consecutive joints make a tapered capsule,
// widened as it ages. Each capsule is drawn as its own small patch of the sea surface (as shadows are), so the wake
// costs the area it covers. A capsule draws only where the body had not been more recently (the younger slice's
// capsules do not cover the pixel) and, along a chain, only where it is the deepest of its neighbours, so each pixel is
// foamed by the youngest slice that left it: a trail that fades with age, with no foam stacked on foam.

import { sub, time, variable, vec2, vec4, type TableSource } from "../expr.ts";
import { drawSpec, type CompileContext, type Drawable, type Emitted } from "../compile/context.ts";
import type { Body } from "../map/sweep.ts";
import type { Skeleton } from "../map/types.ts";

/** vec4s per capsule: (end a xz, end b xz), (radius a, radius b, age, valid), (linked to previous, linked to next, 0, 0). */
const ENTRY = 3;
/** The wake keeps every STRIDE-th station of the body. */
const STRIDE = 3;
/** Quads per side of a capsule's patch. */
const PATCH = 3;

export type WakeSpec = {
  /** Seconds of the recent past the wake remembers. */
  readonly over: number;
  /** Past slices sampled across `over`. */
  readonly samples?: number;
  /** World units per second the foam spreads as it ages. */
  readonly spread?: number;
  /** Foam colour (display sRGB). */
  readonly colour?: readonly [number, number, number];
};

class Wake implements Drawable {
  readonly skeletons = [];
  readonly surfaces = [];
  constructor(readonly name: string, readonly body: Body, readonly spec: Required<WakeSpec>) {}

  draws(ctx: CompileContext): Emitted {
    const s = this.spec, dt = s.over / s.samples;
    // Past slices are skeletons only (positions and radii, no frames), at every STRIDE-th station: all a waterline
    // needs, at a third of the cost (DESIGN.md 9). Row 0 reads the live body at the same stations.
    const n = this.body.transport.stations, joints = Math.ceil((n - 1) / STRIDE) + 1, segs = joints - 1;
    const kept = Array.from({ length: joints }, (_, j) => Math.round((j * (n - 1)) / (joints - 1)));
    const live: Skeleton = { joints, joint: (j) => this.body.skeleton.joint(kept[j]!) };
    const slices = Array.from({ length: s.samples + 1 }, (_, k) => (k === 0 ? live : this.body.skeletonAt(sub(time, k * dt), kept)));
    const X = variable("x", "f32"), Z = variable("z", "f32"), T = variable("t", "f32");
    const source: TableSource = {
      name: `wake_${this.body.name}`, lanes: 1, entries: ENTRY * (s.samples + 1) * segs, role: "data",
      prepare: (p) => {
        const chains = slices.map((sk) => p.compile(Array.from({ length: joints }, (_, k) => { const j = sk.joint(k); return vec4(j.p, j.r); })));
        const height = p.compile([ctx.sea.height(vec2(X, Z), T)]);
        return (frame, out) => {
          const now = frame.env.inputs.time as number;
          const vars = { x: 0, z: 0, t: 0 };
          chains.forEach((chain, k) => {
            vars.t = now - k * dt;
            const js = chain(frame) as number[][];
            const waterline = js.map(([x, y, z, r]) => { vars.x = x; vars.z = z; const h = y - (height(frame, vars)[0] as number); return Math.sqrt(Math.max(r * r - h * h, 0)); });
            fillRow(js, waterline, k * dt, s.spread, out, k * segs);
          });
        };
      },
    };
    const t = ctx.table(source).wgsl;
    const [fr, fg, fb] = s.colour;
    const code = /* wgsl */ `
const PATCH: u32 = ${PATCH}u;
const SEGS: u32 = ${segs}u;
const OVER: f32 = ${s.over};
struct VOut {
  @builtin(position) clip: vec4<f32>,
  @location(0) world: vec3<f32>,
  @location(1) @interpolate(flat) id: u32,
};
/// Signed distance from p to capsule e (tapered): negative inside.
fn wake_d(e: u32, p: vec2<f32>) -> f32 {
  let a = ${t}(${ENTRY}u * e);
  let b = ${t}(${ENTRY}u * e + 1u);
  let pa = p - a.xy;
  let ba = a.zw - a.xy;
  let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-8), 0.0, 1.0);
  return length(pa - ba * h) - mix(b.x, b.y, h);
}
fn wake_valid(e: u32) -> bool { return ${t}(${ENTRY}u * e + 1u).w > 0.5; }
fn wake_hash(c: vec2<f32>) -> f32 { return fract(sin(dot(c, vec2<f32>(127.1, 311.7))) * 43758.5453); }
fn wake_noise(p: vec2<f32>) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let u = f * f * (3.0 - 2.0 * f);
  return mix(mix(wake_hash(i), wake_hash(i + vec2<f32>(1.0, 0.0)), u.x), mix(wake_hash(i + vec2<f32>(0.0, 1.0)), wake_hash(i + vec2<f32>(1.0, 1.0)), u.x), u.y);
}
/// Whether a younger slice's capsule at segment j (or its neighbours) covers p: the body was there more recently.
fn wake_younger_covers(row: u32, j: u32, p: vec2<f32>) -> bool {
  let e = (row - 1u) * SEGS + j;
  var hit = wake_valid(e) && wake_d(e, p) < 0.0;
  if (j > 0u) { hit = hit || (wake_valid(e - 1u) && wake_d(e - 1u, p) < 0.0); }
  if (j + 1u < SEGS) { hit = hit || (wake_valid(e + 1u) && wake_d(e + 1u, p) < 0.0); }
  return hit;
}
@vertex fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VOut {
  var out: VOut;
  // Row 0 (the body now) is never drawn: the sea's contact foam covers it. It is read as the youngest row.
  let e = ii + SEGS;
  out.id = e;
  if (!wake_valid(e)) {
    out.clip = vec4<f32>(0.0, 0.0, 2.0, 1.0);
    out.world = vec3<f32>(0.0);
    return out;
  }
  let a = ${t}(${ENTRY}u * e);
  let b = ${t}(${ENTRY}u * e + 1u);
  let ij = vec2<u32>((vi / 6u) / PATCH, (vi / 6u) % PATCH) + quad_corner(vi % 6u);
  let uv = vec2<f32>(ij) / f32(PATCH);
  let ab = a.zw - a.xy;
  let len = length(ab);
  let along = select(vec2<f32>(1.0, 0.0), ab / len, len > 1e-6);
  let across = vec2<f32>(-along.y, along.x);
  let pad = max(b.x, b.y) + 0.1;
  let xz = a.xy + along * mix(-pad, len + pad, uv.x) + across * mix(-pad, pad, uv.y);
  out.world = vec3<f32>(xz.x, sea_height(xz), xz.y);
  out.clip = frame.view_proj * vec4<f32>(out.world, 1.0);
  return out;
}
@fragment fn fs(in: VOut) -> @location(0) vec4<f32> {
  let e = in.id;
  let row = e / SEGS;
  let j = e % SEGS;
  let p = in.world.xz;
  let px = sea_footprint(in.world);
  let d = wake_d(e, p);
  let c = ${t}(${ENTRY}u * e + 2u);
  if (d > px || wake_younger_covers(row, j, p) || (c.x > 0.5 && wake_d(e - 1u, p) < d) || (c.y > 0.5 && wake_d(e + 1u, p) <= d)) { discard; }
  let age = ${t}(${ENTRY}u * e + 1u).z / OVER;
  // Lacy foam fixed on the water: two octaves of value noise against a threshold that rises as the foam ages, with a
  // brighter line where the waterline was most recently (the capsule's rim).
  let n = 0.65 * wake_noise(p * 2.1) + 0.35 * wake_noise(p * 5.3 + vec2<f32>(3.1, 7.7));
  let threshold = mix(0.3, 0.72, age);
  let lace = smoothstep(threshold - 0.07, threshold + 0.07, n);
  let rim = 1.0 - smoothstep(0.0, 0.35, -d);
  let foam = max(lace, rim * (1.0 - age));
  let edge = 1.0 - smoothstep(-px, px, d);
  let alpha = foam * edge * 0.85 * pow(1.0 - age, 1.4);
  if (alpha <= 0.002) { discard; }
  return vec4<f32>(${fr}, ${fg}, ${fb}, alpha);
}`;
    return {
      shaders: { [this.name]: { code, libs: [] } },
      draws: [drawSpec({ label: this.name, shader: this.name, vertices: 6 * PATCH * PATCH, instances: s.samples * segs, phase: "on-water", cull: "none", depthWrite: false, blend: "alpha" })],
    };
  }
}

/** One slice's capsules: segment j from joint j to j + 1, valid where either end cuts the water. */
export function fillRow(joints: readonly (readonly number[])[], waterline: readonly number[], age: number, spread: number, out: Float32Array, first: number): void {
  const segs = joints.length - 1;
  const valid = (j: number): boolean => waterline[j] > 1e-3 || waterline[j + 1] > 1e-3;
  for (let j = 0; j < segs; j++) {
    if (!valid(j)) continue;
    const a = joints[j], b = joints[j + 1];
    // A joint clear of the water contributes no width; the capsule still reaches it so the foam tapers to a point.
    const grow = spread * age;
    out.set([a[0], a[2], b[0], b[2], waterline[j] + (waterline[j] > 1e-3 ? grow : 0), waterline[j + 1] + (waterline[j + 1] > 1e-3 ? grow : 0), age, 1,
      j > 0 && valid(j - 1) ? 1 : 0, j + 1 < segs && valid(j + 1) ? 1 : 0, 0, 0], (first + j) * ENTRY * 4);
  }
}

/** Foam on the water where the body crossed the surface over the last `over` seconds, fading with age. */
export function wake(body: Body, spec: WakeSpec): Drawable {
  const full: Required<WakeSpec> = { over: spec.over, samples: spec.samples ?? 10, spread: spec.spread ?? 0.35, colour: spec.colour ?? [0.95, 0.98, 0.97] };
  if (!(full.over > 0)) throw new Error(`wake: over must be positive, got ${full.over}`);
  if (!(Number.isInteger(full.samples) && full.samples >= 1)) throw new Error(`wake: samples must be a positive integer, got ${full.samples}`);
  if (body.skeleton.joints < 2) throw new Error(`wake: "${body.name}" has no skeleton segments to leave a wake with`);
  return new Wake(`wake_${body.name}`, body, full);
}
