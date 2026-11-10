// The `stations` table: the once-per-frame CPU pose of every strand, and the two readers of it (WGSL for the vertex
// stage, JavaScript for checks and tests), kept side by side because they must agree.
//
// Layout, 3 vec4 per station (DESIGN.md section 3):
//   p = (position xyz, arc fraction)       q = frame quaternion (x, y, z, w)       r = (half-width, half-height, stretch, 0)
// The frame quaternion maps local x to the tangent (toward increasing m), local y to the dorsal direction and local z
// to the side (= tangent x dorsal). Stations are uniform in the spine parameter m.
//
// Interpolation between stations: positions by uniform Catmull-Rom over the stations (so the drawn spine is C1 and
// passes through every station), frames by normalised lerp of quaternions, stretch linearly. Tangents at stations
// are central differences, which is exactly the Catmull-Rom derivative at a knot, so frames and the drawn curve agree.

export const STATION_VEC4S = 3;

export type V3 = [number, number, number];
export type Q4 = [number, number, number, number];

export const v3 = {
  add: (a: V3, b: V3): V3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]],
  sub: (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]],
  scale: (a: V3, s: number): V3 => [a[0] * s, a[1] * s, a[2] * s],
  axpy: (s: number, a: V3, b: V3): V3 => [b[0] + s * a[0], b[1] + s * a[1], b[2] + s * a[2]],
  dot: (a: V3, b: V3): number => a[0] * b[0] + a[1] * b[1] + a[2] * b[2],
  cross: (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]],
  len: (a: V3): number => Math.sqrt(a[0] * a[0] + a[1] * a[1] + a[2] * a[2]),
  unit: (a: V3): V3 => { const l = Math.sqrt(a[0] * a[0] + a[1] * a[1] + a[2] * a[2]); return [a[0] / l, a[1] / l, a[2] / l]; },
  /** Rotate v about the unit axis (Rodrigues). */
  rotate: (v: V3, axis: V3, angle: number): V3 => {
    const s = Math.sin(angle), c = Math.cos(angle), d = v3.dot(axis, v) * (1 - c), x = v3.cross(axis, v);
    return [v[0] * c + x[0] * s + axis[0] * d, v[1] * c + x[1] * s + axis[1] * d, v[2] * c + x[2] * s + axis[2] * d];
  },
};

/** Quaternion (x, y, z, w) of the rotation whose matrix has columns a, b, c (orthonormal, right-handed). */
export function quatFromColumns(a: V3, b: V3, c: V3): Q4 {
  const tr = a[0] + b[1] + c[2];
  if (tr > 0) { const s = 2 * Math.sqrt(tr + 1); return [(b[2] - c[1]) / s, (c[0] - a[2]) / s, (a[1] - b[0]) / s, s / 4]; }
  if (a[0] > b[1] && a[0] > c[2]) { const s = 2 * Math.sqrt(1 + a[0] - b[1] - c[2]); return [s / 4, (b[0] + a[1]) / s, (c[0] + a[2]) / s, (b[2] - c[1]) / s]; }
  if (b[1] > c[2]) { const s = 2 * Math.sqrt(1 + b[1] - a[0] - c[2]); return [(b[0] + a[1]) / s, s / 4, (c[1] + b[2]) / s, (c[0] - a[2]) / s]; }
  const s = 2 * Math.sqrt(1 + c[2] - a[0] - b[1]);
  return [(c[0] + a[2]) / s, (c[1] + b[2]) / s, s / 4, (a[1] - b[0]) / s];
}

/** Rotate v by the unit quaternion q (same formula as WGSL `qrot`). */
export function qrot(q: Q4 | readonly number[], v: V3): V3 {
  const u: V3 = [q[0], q[1], q[2]];
  const t = v3.add(v3.cross(u, v), v3.scale(v, q[3]));
  return v3.add(v, v3.scale(v3.cross(u, t), 2));
}

/** One strand's samples for one frame, before they are packed into the table. */
export type StrandSamples = {
  /** Spine positions at m_k = k / (count - 1). */
  readonly pos: readonly V3[];
  /** The same spine at rest (stretch is measured against it). */
  readonly rest: readonly V3[];
  /** Twist (radians about the tangent) at each station. */
  readonly twist: readonly number[];
  /** Half-width before stretch, including tip closure, at each station. */
  readonly halfWidth: readonly number[];
  /** Height over width at each station. */
  readonly aspect: readonly number[];
  /** World direction the frame starts from at m = 0 (projected off the tangent). */
  readonly up: V3;
  /** Per world unit: how fast accumulated roll bleeds back toward `up`. 0 is the pure rotation-minimising frame. */
  readonly level: number;
};

/** Central difference over the stations (one-sided at the ends): the Catmull-Rom derivative at each knot, per index. */
function knotDerivative(p: readonly V3[], k: number): V3 {
  const n = p.length;
  if (k === 0) return v3.sub(p[1], p[0]);
  if (k === n - 1) return v3.sub(p[n - 1], p[n - 2]);
  return v3.scale(v3.sub(p[k + 1], p[k - 1]), 0.5);
}

/**
 * Pose one strand into `out` starting at float offset `at`: rotation-minimising frames by double reflection (Wang,
 * Juettler, Zheng and Liu 2008) seeded from `up`, with `strand_kit::Spine::new`'s levelling, then twist; arc
 * fractions; and radii with volume preservation (radii scale by 1 / sqrt(stretch)).
 */
export function poseStrand(s: StrandSamples, out: Float32Array, at: number): void {
  const n = s.pos.length;
  const tan = s.pos.map((_, k) => v3.unit(knotDerivative(s.pos, k)));
  let arc = 0;
  const arcs = [0];
  for (let k = 1; k < n; k++) arcs.push((arc += v3.len(v3.sub(s.pos[k], s.pos[k - 1]))));
  const total = Math.max(arc, 1e-9);

  const seed = v3.axpy(-v3.dot(s.up, tan[0]), tan[0], s.up);
  // A tangent parallel to `up` leaves no seed direction: fall back to a fixed perpendicular (a decision, not an error).
  let dorsal = v3.len(seed) > 1e-4 ? v3.unit(seed) : v3.unit(v3.cross(tan[0], [0, 0, 1]));
  let prevQ: Q4 = [0, 0, 0, 1];
  for (let k = 0; k < n; k++) {
    const t = tan[k];
    if (k > 0) {
      const step = v3.sub(s.pos[k], s.pos[k - 1]);
      const c1 = v3.dot(step, step);
      // Double reflection: across the chord's bisecting plane, then across the plane taking the reflected tangent onto
      // the new tangent. A zero-length chord (a collapsed spine) leaves the frame as it was.
      if (c1 > 1e-12) {
        const rL = v3.axpy((-2 / c1) * v3.dot(step, dorsal), step, dorsal);
        const tL = v3.axpy((-2 / c1) * v3.dot(step, tan[k - 1]), step, tan[k - 1]);
        const v2 = v3.sub(t, tL), c2 = v3.dot(v2, v2);
        dorsal = c2 > 1e-12 ? v3.axpy((-2 / c2) * v3.dot(v2, rL), v2, rL) : rL;
      }
      dorsal = v3.unit(v3.axpy(-v3.dot(dorsal, t), t, dorsal));
      // Levelling (strand_kit): the weight falls to 0 as the dorsal vector nears a quarter turn from "up in the normal
      // plane", so a neck leaning back past vertical keeps its own back instead of flipping.
      const target = v3.axpy(-v3.dot(s.up, t), t, s.up);
      const horizontal = v3.len(target);
      if (s.level > 0 && horizontal > 1e-3) {
        const tg = v3.scale(target, 1 / horizontal);
        const agree = Math.max(v3.dot(dorsal, tg), 0);
        const angle = Math.atan2(v3.dot(v3.cross(dorsal, tg), t), v3.dot(dorsal, tg));
        const gain = 1 - Math.exp(-s.level * Math.sqrt(c1) * horizontal * horizontal * agree * agree);
        dorsal = v3.rotate(dorsal, t, angle * gain);
      }
    }
    // Twist is applied to a copy: it must not feed back into the propagation.
    const d = s.twist[k] === 0 ? dorsal : v3.rotate(dorsal, t, s.twist[k]);
    const side = v3.unit(v3.cross(t, d));
    let q = quatFromColumns(t, d, side);
    // Keep neighbours in one hemisphere so the vertex stage's nlerp takes the short way round.
    if (q[0] * prevQ[0] + q[1] * prevQ[1] + q[2] * prevQ[2] + q[3] * prevQ[3] < 0) q = [-q[0], -q[1], -q[2], -q[3]];
    prevQ = q;

    const stretch = v3.len(knotDerivative(s.pos, k)) / Math.max(v3.len(knotDerivative(s.rest, k)), 1e-9);
    const keep = 1 / Math.sqrt(Math.max(stretch, 0.05));
    const o = at + k * STATION_VEC4S * 4;
    out[o] = s.pos[k][0]; out[o + 1] = s.pos[k][1]; out[o + 2] = s.pos[k][2]; out[o + 3] = arcs[k] / total;
    out[o + 4] = q[0]; out[o + 5] = q[1]; out[o + 6] = q[2]; out[o + 7] = q[3];
    out[o + 8] = s.halfWidth[k] * keep; out[o + 9] = s.halfWidth[k] * s.aspect[k] * keep; out[o + 10] = stretch; out[o + 11] = 0;
  }
}

// ---- readers -------------------------------------------------------------------------------------------------------

/** The stations table's name (its WGSL accessor is `t_stations`). */
export const STATIONS_TABLE = "stations";

/** WGSL: quaternion rotation and the interpolated station. Lives in `sceneWgsl`; reads the stations table. */
export const STATIONS_WGSL = /* wgsl */ `
fn qrot(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
  return v + 2.0 * cross(q.xyz, cross(q.xyz, v) + q.w * v);
}
struct StationSample {
  p: vec3<f32>,      // position on the interpolated spine
  der: vec3<f32>,    // d(position)/dm
  t: vec3<f32>,      // frame: tangent, dorsal, side
  d: vec3<f32>,
  s: vec3<f32>,
  stretch: f32,
};
fn station_at(base: u32, count: u32, m: f32) -> StationSample {
  let span = f32(count - 1u);
  let x = clamp(m, 0.0, 1.0) * span;
  let k = min(u32(floor(x)), count - 2u);
  let f = x - f32(k);
  let p1 = t_stations((base + k) * 3u).xyz;
  let p2 = t_stations((base + k + 1u) * 3u).xyz;
  // Mirrored phantom ends, as in \`through\`. The index is clamped first so it never leaves the strand's range.
  let p0 = select(t_stations((base + max(k, 1u) - 1u) * 3u).xyz, 2.0 * p1 - p2, k == 0u);
  let p3 = select(t_stations((base + min(k + 2u, count - 1u)) * 3u).xyz, 2.0 * p2 - p1, k + 2u >= count);
  let a = p2 - p0;
  let b = 2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3;
  let c = 3.0 * p1 - p0 + p3 - 3.0 * p2;
  var out: StationSample;
  out.p = 0.5 * (2.0 * p1 + a * f + b * f * f + c * f * f * f);
  out.der = 0.5 * (a + 2.0 * b * f + 3.0 * c * f * f) * span;
  let q = normalize(mix(t_stations((base + k) * 3u + 1u), t_stations((base + k + 1u) * 3u + 1u), f));
  out.t = qrot(q, vec3<f32>(1.0, 0.0, 0.0));
  out.d = qrot(q, vec3<f32>(0.0, 1.0, 0.0));
  out.s = qrot(q, vec3<f32>(0.0, 0.0, 1.0));
  out.stretch = mix(t_stations((base + k) * 3u + 2u).z, t_stations((base + k + 1u) * 3u + 2u).z, f);
  return out;
}
`;

export type StationSample = { p: V3; der: V3; t: V3; d: V3; s: V3; stretch: number };

/** JavaScript mirror of WGSL `station_at`, for checks and the CPU-against-GPU test. */
export function stationAt(table: Float32Array, base: number, count: number, m: number): StationSample {
  const span = count - 1;
  const x = Math.min(1, Math.max(0, m)) * span;
  const k = Math.min(Math.floor(x), count - 2);
  const f = x - k;
  const P = (i: number): V3 => { const o = (base + i) * 12; return [table[o], table[o + 1], table[o + 2]]; };
  const p1 = P(k), p2 = P(k + 1);
  const p0 = k === 0 ? v3.sub(v3.scale(p1, 2), p2) : P(k - 1);
  const p3 = k + 2 >= count ? v3.sub(v3.scale(p2, 2), p1) : P(k + 2);
  const a = v3.sub(p2, p0);
  const b = v3.sub(v3.add(v3.sub(v3.scale(p0, 2), v3.scale(p1, 5)), v3.scale(p2, 4)), p3);
  const c = v3.sub(v3.add(v3.sub(v3.scale(p1, 3), p0), p3), v3.scale(p2, 3));
  const p = v3.scale(v3.add(v3.add(v3.scale(p1, 2), v3.scale(a, f)), v3.add(v3.scale(b, f * f), v3.scale(c, f * f * f))), 0.5);
  const der = v3.scale(v3.add(a, v3.add(v3.scale(b, 2 * f), v3.scale(c, 3 * f * f))), 0.5 * span);
  const o1 = (base + k) * 12 + 4, o2 = (base + k + 1) * 12 + 4;
  const raw = [0, 1, 2, 3].map((i) => table[o1 + i] * (1 - f) + table[o2 + i] * f);
  const l = Math.hypot(raw[0], raw[1], raw[2], raw[3]);
  const q = raw.map((x) => x / l) as Q4;
  const stretch = table[(base + k) * 12 + 10] * (1 - f) + table[(base + k + 1) * 12 + 10] * f;
  return { p, der, t: qrot(q, [1, 0, 0]), d: qrot(q, [0, 1, 0]), s: qrot(q, [0, 0, 1]), stretch };
}
