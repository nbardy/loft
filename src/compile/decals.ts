// Decals derived from the skeleton, on the CPU, once per frame: foam collars where strands cross the water and the
// shadows their bodies cast on it. Both are read per pixel by the sea's fragment stage (shaders.ts, DECALS_WGSL).

import { STATION_VEC4S, v3, type V3 } from "../geom/stations.ts";
import type { StationRange } from "./context.ts";

/** Floats per decal entry: two vec4s. The table starts with one header vec4 (x = count). */
const ENTRY = 8;

export type SeaHeight = (x: number, z: number) => number;

/** One station read back out of the table, with the sea height under it: position, frame axes, radii. */
type Read = { p: V3; t: V3; d: V3; s: V3; a: number; b: number; dy: number };

/** Columns of the rotation matrix of a unit quaternion: the images of x, y and z (tangent, dorsal, side). */
function axes(x: number, y: number, z: number, w: number): [V3, V3, V3] {
  return [
    [1 - 2 * (y * y + z * z), 2 * (x * y + w * z), 2 * (x * z - w * y)],
    [2 * (x * y - w * z), 1 - 2 * (x * x + z * z), 2 * (y * z + w * x)],
    [2 * (x * z + w * y), 2 * (y * z - w * x), 1 - 2 * (x * x + y * y)],
  ];
}

/** Every station once per frame (the sea height is the expensive part, and both decal kinds need it). */
export function readStations(table: Float32Array, count: number, height: SeaHeight): Read[] {
  const out: Read[] = [];
  for (let i = 0; i < count; i++) {
    const o = i * STATION_VEC4S * 4;
    const [t, d, s] = axes(table[o + 4], table[o + 5], table[o + 6], table[o + 7]);
    const p: V3 = [table[o], table[o + 1], table[o + 2]];
    out.push({ p, t, d, s, a: table[o + 8], b: table[o + 9], dy: p[1] - height(p[0], p[2]) });
  }
  return out;
}

const flat = (v: V3): [number, number] => [v[0], v[2]];
const unit2 = (x: number, z: number): [number, number] => { const l = Math.hypot(x, z); return l > 1e-9 ? [x / l, z / l] : [1, 0]; };
const foamWidth = (r: number): number => 0.12 + 0.45 * r;

function put(out: Float32Array, n: number, e: readonly number[]): number {
  // Tables are sized for the worst case (decalVec4s), so running out means that sizing is wrong.
  if (4 + (n + 1) * ENTRY > out.length) throw new Error(`decal table full at ${n} entries (${out.length / 4} vec4s): decalVec4s undercounts`);
  out.set(e, 4 + n * ENTRY);
  return n + 1;
}

/**
 * Foam collars. Two sources, both from the posed rings:
 * - a ring whose plane cuts the sea (a body lying awash): the cut's two waterline points (strand_kit::waterline_cut)
 *   give an ellipse across the body at that station;
 * - a spine segment that pierces the sea (a neck rising out): a tube meeting a plane at elevation alpha cuts an
 *   ellipse of radii (r, r / sin alpha), centred where the spine crosses.
 * Their smooth union in the sea shader is the waterline loop, so foam needs no placement by hand.
 */
export function collars(all: readonly Read[], ranges: readonly StationRange[], out: Float32Array): void {
  let n = 0;
  for (const r of ranges) {
    for (let k = 0; k < r.count; k++) {
      const st = all[r.base + k];
      const dy = st.dy;
      const ay = st.a * st.s[1], by = st.b * st.d[1];
      const amp = Math.hypot(ay, by);
      if (amp > 1e-5 && Math.abs(dy) < amp) {
        const phi0 = Math.atan2(by, ay), spread = Math.acos(-dy / amp);
        const cut = [phi0 + spread, phi0 - spread].map((phi) => v3.add(st.p, v3.add(v3.scale(st.s, st.a * Math.cos(phi)), v3.scale(st.d, st.b * Math.sin(phi)))));
        const across = [cut[1][0] - cut[0][0], cut[1][2] - cut[0][2]];
        const half = Math.hypot(across[0], across[1]) / 2;
        // Along the body the ellipse reaches to the neighbouring stations, so consecutive cuts overlap.
        const next = all[r.base + Math.min(k + 1, r.count - 1)], prev = all[r.base + Math.max(k - 1, 0)];
        const spacing = Math.hypot(next.p[0] - prev.p[0], next.p[2] - prev.p[2]) / 2;
        const along = unit2(-across[1], across[0]);
        // A ring that barely breaks the surface cuts a short chord: its foam shrinks with it. (At full width, a
        // skimming ridge drew a long hairline of foam: the ellipse is thin but the band round it was not.)
        const radius = Math.max(st.a, st.b), depth = Math.min(1, half / Math.max(radius, 1e-6));
        if (depth > 0.15) {
          n = put(out, n, [(cut[0][0] + cut[1][0]) / 2, (cut[0][2] + cut[1][2]) / 2, Math.max(spacing * 0.75, half * 0.3), half, along[0], along[1], foamWidth(radius) * depth, k]);
        }
      }
      if (k + 1 < r.count) {
        const nx = all[r.base + k + 1];
        const dy1 = nx.dy;
        if ((dy < 0) !== (dy1 < 0)) {
          const f = dy / (dy - dy1);
          const p = v3.add(st.p, v3.scale(v3.sub(nx.p, st.p), f));
          const tan = v3.unit(v3.sub(nx.p, st.p));
          const a = st.a + (nx.a - st.a) * f, b = st.b + (nx.b - st.b) * f;
          const sinAlpha = Math.max(Math.abs(tan[1]), 0.3);
          const dir = unit2(...flat(tan));
          n = put(out, n, [p[0], p[2], b / sinAlpha, a, dir[0], dir[1], foamWidth(Math.max(a, b)), k]);
        }
      }
    }
  }
  out[0] = n;
}

/**
 * Cast shadows: each segment between stations above the water, projected along the sun onto the local sea surface,
 * is a tapered capsule on the sea; softer and fainter the higher it is (strand_kit::cast_shadow).
 */
export function shadows(all: readonly Read[], ranges: readonly StationRange[], sun: V3, out: Float32Array): void {
  let n = 0;
  for (const r of ranges) {
    const foot = (i: number): { xz: [number, number]; h: number; r: number } => {
      const st = all[r.base + i];
      const h = st.dy;
      const lift = Math.max(h, 0) / sun[1];
      return { xz: [st.p[0] - sun[0] * lift, st.p[2] - sun[2] * lift], h, r: Math.max(st.a, st.b) };
    };
    let a = foot(0);
    for (let k = 1; k < r.count; k++) {
      const b = foot(k);
      const h = Math.min(a.h, b.h);
      if (h > 0.02 && Math.max(a.r, b.r) > 1e-3) {
        const strength = 0.85 * smooth((h - 0.02) / 0.35) * Math.exp(-h / 9);
        n = put(out, n, [a.xz[0], a.xz[1], b.xz[0], b.xz[1], a.r, b.r, strength, 0.03 + 0.04 * h]);
      }
      a = b;
    }
  }
  out[0] = n;
}

const smooth = (x: number): number => { const t = Math.min(1, Math.max(0, x)); return t * t * (3 - 2 * t); };

/** Table sizes, in vec4s, for a given total station count (worst case: every station contributes). */
export const decalVec4s = (stations: number): { collars: number; shadows: number } => ({ collars: 1 + 2 * 2 * stations, shadows: 1 + 2 * stations });
