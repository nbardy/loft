// Cast shadows on the water, from every thing with a skeleton.
//
// Each spine segment above the water, projected along the sun onto the local sea surface, is a tapered capsule;
// softer and fainter the higher it is (strand_kit::cast_shadow). The pose pass writes the capsules to a table, and
// each is drawn as its own small instanced patch of the sea surface laid over the water, so a shadow costs the area
// it covers, not the area of the sea. (The sea's fragment stage used to loop over every capsule for every sea pixel:
// most of a frame at 1280x800.)
//
// Overlap: shadows combine by max, not by stacking. A patch draws only the pixels where its own capsule is at least
// as dark as its neighbours' in the same chain (ties go to the later one), so the joints of a chain are drawn once.

import { STATION_VEC4S, type V3 } from "../geom/stations.ts";
import { draw, isSkeleton, type CompileContext, type Drawable, type Emitted, type PoseFrame, type StationRange } from "../compile/context.ts";

/** vec4s per capsule: (end a xz, end b xz), (radius a, radius b, strength, softness), (has previous, has next, 0, 0). */
const ENTRY = 3;
/** Quads per side of a capsule's patch: enough to follow the swell under a capsule a few units long. */
const PATCH = 4;

export const shadows: Drawable = {
  name: "shadows",
  draws(ctx: CompileContext): Emitted {
    const chains = ctx.things.filter(isSkeleton).map((t) => t.skeleton(ctx));
    const capacity = chains.reduce((n, c) => n + c.count - 1, 0);
    const table = ctx.table({ name: "shadows", vec4s: 1 + ENTRY * Math.max(capacity, 1), fill: (frame, out) => fillShadows(frame, chains, ctx.sun, out) });
    const t = table.wgsl;
    const code = /* wgsl */ `
const PATCH: u32 = ${PATCH}u;
struct VOut {
  @builtin(position) clip: vec4<f32>,
  @location(0) world: vec3<f32>,
  @location(1) @interpolate(flat) id: u32,
};
/// Shadow of capsule i at p: 0..1.
fn capsule_shadow(i: u32, p: vec2<f32>, px: f32) -> f32 {
  let a = ${t}(1u + ${ENTRY}u * i);
  let b = ${t}(2u + ${ENTRY}u * i);
  let pa = p - a.xy;
  let ba = a.zw - a.xy;
  let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-8), 0.0, 1.0);
  let r = mix(b.x, b.y, h);
  let soft = max(b.w, px);
  return b.z * (1.0 - smoothstep(r - soft, r + soft, length(pa - ba * h)));
}
@vertex fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VOut {
  var out: VOut;
  out.id = ii;
  if (ii >= u32(${t}(0u).x)) {
    // An unused slot: a degenerate triangle outside the clip volume.
    out.clip = vec4<f32>(0.0, 0.0, 2.0, 1.0);
    out.world = vec3<f32>(0.0);
    return out;
  }
  let a = ${t}(1u + ${ENTRY}u * ii);
  let b = ${t}(2u + ${ENTRY}u * ii);
  let ij = vec2<u32>((vi / 6u) / PATCH, (vi / 6u) % PATCH) + quad_corner(vi % 6u);
  let uv = vec2<f32>(ij) / f32(PATCH);
  let ab = a.zw - a.xy;
  let len = length(ab);
  let along = select(vec2<f32>(1.0, 0.0), ab / len, len > 1e-6);
  let across = vec2<f32>(-along.y, along.x);
  // Out to where the softened edge has faded, plus a margin for the pixel-wide antialiasing far away.
  let pad = max(b.x, b.y) + 2.0 * b.w + 0.1;
  let xz = a.xy + along * mix(-pad, len + pad, uv.x) + across * mix(-pad, pad, uv.y);
  out.world = vec3<f32>(xz.x, sea_height(xz), xz.y);
  out.clip = frame.view_proj * vec4<f32>(out.world, 1.0);
  return out;
}
@fragment fn fs(in: VOut) -> @location(0) vec4<f32> {
  let i = in.id;
  let p = in.world.xz;
  let px = sea_footprint(in.world);
  let s = capsule_shadow(i, p, px);
  let c = ${t}(3u + ${ENTRY}u * i);
  if (s <= 0.0 || (c.x > 0.5 && capsule_shadow(i - 1u, p, px) > s) || (c.y > 0.5 && capsule_shadow(i + 1u, p, px) >= s)) { discard; }
  return sea_shadow(in.world, -view_dir(in.world), distance(in.world, frame.eye.xyz), s);
}`;
    return {
      shaders: { shadows: code },
      draws: [draw({ label: "shadows", shader: "shadows", vertices: 6 * PATCH * PATCH, instances: Math.max(capacity, 1), phase: "on-water", cull: "none", depthWrite: false, blend: "alpha" })],
    };
  },
};

const smooth = (x: number): number => { const t = Math.min(1, Math.max(0, x)); return t * t * (3 - 2 * t); };

/** Projects the chains' stations along the sun onto the sea; writes the count to the header. */
export function fillShadows(frame: PoseFrame, chains: readonly StationRange[], sun: V3, out: Float32Array): void {
  let n = 0;
  for (const c of chains) {
    const table = frame.table(c.table);
    const foot = (k: number): { xz: [number, number]; h: number; r: number } => {
      const o = (c.base + k) * STATION_VEC4S * 4;
      const p: V3 = [table[o], table[o + 1], table[o + 2]];
      const h = p[1] - frame.seaHeight(p[0], p[2]);
      const lift = Math.max(h, 0) / sun[1];
      return { xz: [p[0] - sun[0] * lift, p[2] - sun[2] * lift], h, r: Math.max(table[o + 8], table[o + 9]) };
    };
    let a = foot(0), linked = false;
    for (let k = 1; k < c.count; k++) {
      const b = foot(k);
      const h = Math.min(a.h, b.h);
      if (h > 0.02 && Math.max(a.r, b.r) > 1e-3) {
        const strength = 0.85 * smooth((h - 0.02) / 0.35) * Math.exp(-h / 9);
        if (4 + (n + 1) * ENTRY * 4 > out.length) throw new Error(`shadows: table full at ${n} capsules (sized from the chains, so the sizing is wrong)`);
        // Linked to the previous capsule when that one was written for the previous segment of this chain.
        out.set([a.xz[0], a.xz[1], b.xz[0], b.xz[1], a.r, b.r, strength, 0.03 + 0.04 * h, linked ? 1 : 0, 0, 0, 0], 4 + n * ENTRY * 4);
        if (linked) out[4 + (n - 1) * ENTRY * 4 + 9] = 1;
        n++;
        linked = true;
      } else {
        linked = false;
      }
      a = b;
    }
  }
  out[0] = n;
}
