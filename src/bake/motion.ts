// The animated bake (DESIGN.md 16): a scene's bodies at t = 0, and how each moves over a span, as glTF can carry it.
//
// Every baked body gets one `Motion`. A swept body (`Body`) is skinned: one joint per station, keyed from the station
// table the pose pass fills (position, transported frame, and a scale for girth, aspect and stretch). Anything else
// whose triangles keep their topology is keyed as morph targets when it moves. The file's motion is measured against
// Loft's own evaluation at every key and between keys; nothing goes over the tolerance silently.

import { tolerance, type Tolerance } from "../check.ts";
import { SceneError } from "../errors.ts";
import { time, variable } from "../expr.ts";
import { Body, stationSample } from "../map/sweep.ts";
import type { Scene } from "../scene.ts";
import { prepareBake, type BodyMesh, type Density, type Sampler, type Skipped } from "./mesh.ts";

/** The most a file's motion may differ from the scene's, as a share of each thing's size (its rest box's diagonal). */
export const MOTION_TOLERANCE: Tolerance = tolerance.share(1e-3);
/** Keys of the pilot that measures how fast things accelerate; also the fewest keys written. */
const PILOT_KEYS = 32;
/** The most keys over a span: past this, a thing the keys cannot follow is refused, not thinned. */
const MOST_KEYS = 1024;
/** The most joints put between two stations: past this a body's skin is refused for morph targets. */
const MOST_REFINE = 8;

/** The largest turn of any joint from one key to the next, and the key it starts at. */
function largestTurn(rows: readonly (readonly JointRow[])[], times: readonly number[]): { readonly turn: number; readonly at: number } {
  let turn = 0, at = 0;
  for (let i = 0; i + 1 < rows.length; i++) rows[i]!.forEach((s, j) => {
    const d = qangle(qmul(rows[i + 1]![j]!.q, qconj(s.q)));
    if (d > turn) { turn = d; at = times[i]!; }
  });
  return { turn, at };
}

export type V3 = [number, number, number];
/** A unit quaternion (x, y, z, w), glTF's order and Loft's. */
export type Quat = [number, number, number, number];
/** One joint's local transform at one key: glTF's TRS. */
export type JointKey = { readonly t: V3; readonly r: Quat; readonly s: V3 };

/** How far the file's motion is from the scene's, in world units, and the thing's size it is judged against. */
export type MotionError = {
  /** Largest vertex distance at the keys. */
  readonly atKeys: number;
  /** Largest vertex distance half way between keys. */
  readonly betweenKeys: number;
  /** The rest bounding box's diagonal. */
  readonly size: number;
  /** What DESIGN.md 16 predicts before measuring (skin geometry plus keys), for a skinned thing. */
  readonly predicted: number | undefined;
};

export type Motion =
  /** Moves less than the tolerance over the span (or a field, meshed once): the rest mesh alone. */
  | { readonly kind: "still"; readonly why: string; readonly moved: number; readonly size: number }
  | {
    readonly kind: "skinned";
    /** The key times, 0 .. span (each thing has its own). */
    readonly times: readonly number[];
    /** keys[i][j]: joint j at key i. */
    readonly keys: readonly (readonly JointKey[])[];
    /** Per vertex, two joints and their weights (DESIGN.md 16, "Weights"). */
    readonly joints: Uint16Array;
    readonly weights: Float32Array;
    /** Column-major 4x4 per joint: the inverse of its rest transform. */
    readonly inverseBind: Float32Array;
    readonly error: MotionError;
  }
  /** One target per key (positions minus the rest's), weights keyed one-hot. */
  | { readonly kind: "morphed"; readonly why: string; readonly times: readonly number[]; readonly deltas: readonly Float32Array[]; readonly error: MotionError }
  /** Even the keys miss the tolerance: written at rest, and printed. */
  | { readonly kind: "refused"; readonly why: string; readonly error: MotionError };

export type Animated = {
  /** The bodies at t = 0, the rest every motion is relative to. */
  readonly bodies: readonly BodyMesh[];
  readonly motions: readonly Motion[];
  readonly skipped: readonly Skipped[];
  readonly span: number;
  /** Where the span came from. */
  readonly spanFrom: "period" | "seconds";
};

/**
 * What a sampler can be keyed as, classified once. `chain`: a live, whole swept body (its stations are joints).
 * `free`: fixed triangles that are not a body (morph targets when they move). `field`: meshed once by surface nets.
 */
type Kind =
  | { readonly kind: "chain"; readonly body: Body }
  | { readonly kind: "free"; readonly why: string }
  | { readonly kind: "field" };

function kindOf(s: Sampler): Kind {
  const form = s.form;
  switch (form.kind) {
    case "surface": {
      const b = form.surface;
      if (!(b instanceof Body)) return { kind: "free", why: "a surface that is not a swept body has no chain" };
      // A slice at another time (a smear's copy) or a part of a body posts stations a skin does not follow.
      if (b.slice !== time || b.posted.kind !== "all") return { kind: "free", why: "a body taken at another time has no live chain" };
      return { kind: "chain", body: b };
    }
    case "mesh": return { kind: "free", why: "a mesh has no chain" };
    case "field": return { kind: "field" };
    case "medium": case "hit": throw new Error(`bake: ${form.kind} "${form.name}" has no sampler`); // samplersOf skips them
  }
}

// ---- small linear algebra (quaternions x, y, z, w; matrices column-major as glTF writes them) ----

const sub3 = (a: readonly number[], b: readonly number[]): V3 => [a[0]! - b[0]!, a[1]! - b[1]!, a[2]! - b[2]!];
const len3 = (a: readonly number[]): number => Math.hypot(a[0]!, a[1]!, a[2]!);
function qrot(q: Quat, v: readonly number[]): V3 {
  const [x, y, z, w] = q;
  const cx = y * v[2]! - z * v[1]! + w * v[0]!, cy = z * v[0]! - x * v[2]! + w * v[1]!, cz = x * v[1]! - y * v[0]! + w * v[2]!;
  return [v[0]! + 2 * (y * cz - z * cy), v[1]! + 2 * (z * cx - x * cz), v[2]! + 2 * (x * cy - y * cx)];
}
const qconj = (q: Quat): Quat => [-q[0], -q[1], -q[2], q[3]];
function qmul(a: Quat, b: Quat): Quat {
  return [
    a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
    a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
    a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
    a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
  ];
}
const qangle = (q: Quat): number => 2 * Math.acos(Math.min(1, Math.abs(q[3])));
function qnorm(q: readonly number[]): Quat {
  const l = Math.hypot(q[0]!, q[1]!, q[2]!, q[3]!);
  return [q[0]! / l, q[1]! / l, q[2]! / l, q[3]! / l];
}
/** Spherical interpolation, as a glTF viewer does for LINEAR rotation keys (the keys are already on one hemisphere). */
function slerp(a: Quat, b: Quat, x: number): Quat {
  const d = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
  if (d > 0.9995) return qnorm(a.map((ai, k) => ai + (b[k]! - ai) * x));
  const th = Math.acos(Math.min(1, d)), s = Math.sin(th);
  const wa = Math.sin((1 - x) * th) / s, wb = Math.sin(x * th) / s;
  return a.map((ai, k) => wa * ai + wb * b[k]!) as Quat;
}
/** T R S as a column-major 4x4. */
function trs(k: JointKey): Float64Array {
  const [x, y, z, w] = k.r, m = new Float64Array(16);
  const r = [
    1 - 2 * (y * y + z * z), 2 * (x * y + z * w), 2 * (x * z - y * w),
    2 * (x * y - z * w), 1 - 2 * (x * x + z * z), 2 * (y * z + x * w),
    2 * (x * z + y * w), 2 * (y * z - x * w), 1 - 2 * (x * x + y * y),
  ];
  for (let c = 0; c < 3; c++) for (let row = 0; row < 3; row++) m[4 * c + row] = r[3 * c + row]! * k.s[c]!;
  m[12] = k.t[0]; m[13] = k.t[1]; m[14] = k.t[2]; m[15] = 1;
  return m;
}
/** The inverse of a rigid T R: R^T and -R^T t. */
function inverseRigid(t: V3, r: Quat): Float64Array {
  const m = trs({ t: [0, 0, 0], r, s: [1, 1, 1] }), inv = new Float64Array(16);
  for (let c = 0; c < 3; c++) for (let row = 0; row < 3; row++) inv[4 * c + row] = m[4 * row + c]!;
  for (let row = 0; row < 3; row++) inv[12 + row] = -(inv[row]! * t[0] + inv[4 + row]! * t[1] + inv[8 + row]! * t[2]);
  inv[15] = 1;
  return inv;
}
function mul4(a: Float64Array, b: Float64Array): Float64Array {
  const m = new Float64Array(16);
  for (let c = 0; c < 4; c++) for (let row = 0; row < 4; row++) {
    let s = 0;
    for (let k = 0; k < 4; k++) s += a[4 * k + row]! * b[4 * c + k]!;
    m[4 * c + row] = s;
  }
  return m;
}

/** A joint's transform at time t between keys: LINEAR translation and scale, slerped rotation, as glTF plays them. */
function keyAt(keys: readonly (readonly JointKey[])[], times: readonly number[], t: number, j: number): JointKey {
  const span = times[times.length - 1]!, n = times.length - 1;
  const x = Math.min(Math.max(t / span, 0), 1) * n, i = Math.min(Math.floor(x), n - 1), a = x - i;
  const k0 = keys[i]![j]!, k1 = keys[i + 1]![j]!;
  const lerp = (p: V3, q: V3): V3 => [p[0] + (q[0] - p[0]) * a, p[1] + (q[1] - p[1]) * a, p[2] + (q[2] - p[2]) * a];
  return { t: lerp(k0.t, k1.t), r: slerp(k0.r, k1.r, a), s: lerp(k0.s, k1.s) };
}

/** Linear blend skinning of rest positions at time t (the file's motion, evaluated on the CPU). */
export function skin(rest: Float32Array, m: Omit<Extract<Motion, { kind: "skinned" }>, "error">, t: number): Float32Array {
  const n = m.keys[0]!.length;
  const C = Array.from({ length: n }, (_, j) => mul4(trs(keyAt(m.keys, m.times, t, j)), Float64Array.from(m.inverseBind.subarray(16 * j, 16 * j + 16))));
  const out = new Float32Array(rest.length);
  for (let v = 0; v < rest.length / 3; v++) {
    const x = rest[3 * v]!, y = rest[3 * v + 1]!, z = rest[3 * v + 2]!;
    for (let k = 0; k < 4; k++) {
      const w = m.weights[4 * v + k]!;
      if (w === 0) continue;
      const c = C[m.joints[4 * v + k]!]!;
      for (let r = 0; r < 3; r++) out[3 * v + r]! += w * (c[r]! * x + c[4 + r]! * y + c[8 + r]! * z + c[12 + r]!);
    }
  }
  return out;
}

function boxDiagonal(p: Float32Array): number {
  const lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
  for (let i = 0; i < p.length; i++) { lo[i % 3] = Math.min(lo[i % 3]!, p[i]!); hi[i % 3] = Math.max(hi[i % 3]!, p[i]!); }
  return len3(sub3(hi, lo));
}
function farthest(a: Float32Array, b: Float32Array): number {
  let worst = 0;
  for (let i = 0; i < a.length; i += 3) worst = Math.max(worst, Math.hypot(a[i]! - b[i]!, a[i + 1]! - b[i + 1]!, a[i + 2]! - b[i + 2]!));
  return worst;
}
const allowance = (size: number): number => {
  switch (MOTION_TOLERANCE.kind) {
    case "units": return MOTION_TOLERANCE.units;
    case "share": return MOTION_TOLERANCE.share * size;
  }
};

/**
 * A body's spine as joints: at any J points m_j = j / (J - 1) along it, the sweep's own sample there (`stationSample`:
 * the Catmull-Rom point, the nlerped frame, the lerped stretch; what the skin at m is built on), its half-width and
 * the section's girth and aspect. J = stations reads the stations themselves; a finer J puts joints between them.
 */
type Rig = { readonly stations: number; readonly leastStretch: number; read(frame: Frame, joints: number): JointRow[] };
type JointRow = { readonly p: V3; readonly q: Quat; readonly a: number; readonly b: number; readonly stretch: number; readonly girth: number; readonly aspect: number };
type Frame = ReturnType<ReturnType<typeof prepareBake>["frameAt"]>;

/** The rotation taking x, y, z to the columns t, d, s, as a unit quaternion (Shepperd's method: the largest pivot). */
function quatOfAxes(t: readonly number[], d: readonly number[], s: readonly number[]): Quat {
  const [m00, m10, m20] = t as [number, number, number], [m01, m11, m21] = d as [number, number, number], [m02, m12, m22] = s as [number, number, number];
  const tr = m00 + m11 + m22;
  if (tr > 0) { const k = 2 * Math.sqrt(1 + tr); return qnorm([(m21 - m12) / k, (m02 - m20) / k, (m10 - m01) / k, k / 4]); }
  if (m00 > m11 && m00 > m22) { const k = 2 * Math.sqrt(1 + m00 - m11 - m22); return qnorm([k / 4, (m01 + m10) / k, (m02 + m20) / k, (m21 - m12) / k]); }
  if (m11 > m22) { const k = 2 * Math.sqrt(1 + m11 - m00 - m22); return qnorm([(m01 + m10) / k, k / 4, (m12 + m21) / k, (m02 - m20) / k]); }
  const k = 2 * Math.sqrt(1 + m22 - m00 - m11);
  return qnorm([(m02 + m20) / k, (m12 + m21) / k, k / 4, (m10 - m01) / k]);
}

function rigOf(body: Body, cpu: ReturnType<typeof prepareBake>["compiled"]["cpu"]): Rig {
  const M = variable("m", "f32"), st = stationSample(body.table, M);
  const run = cpu([st.p, st.t, st.d, st.s, st.stretch, body.halfWidth(M), body.section.girth(M), body.section.aspect(M)]);
  const least = body.section.leastStretch;
  return {
    stations: body.transport.stations, leastStretch: least,
    read: (frame, joints) => Array.from({ length: joints }, (_, j) => {
      const [p, t, d, s, stretch, hw, girth, aspect] = run(frame, { m: j / (joints - 1) }) as [number[], number[], number[], number[], number, number, number, number];
      const a = hw / Math.sqrt(Math.max(stretch, least));
      return { p: [p[0]!, p[1]!, p[2]!], q: quatOfAxes(t, d, s), a, b: a * aspect, stretch, girth, aspect };
    }),
  };
}

/**
 * Joint j's key from its station now and at rest. Scale along the frame's axes (x tangent, y dorsal, z side): the ring
 * is `s A cos + d B sin` (src/map/sweep.ts), so z scales by A(t) / A(0) and y by B(t) / B(0), where
 * A = girth * closure / sqrt(max(stretch, least)) and B = A * aspect; the closure is a function of m alone and cancels,
 * which keeps the ratio finite at a tip where A itself is 0.
 */
function jointKey(now: JointRow, rest: JointRow, least: number): JointKey {
  const keep = Math.sqrt(Math.max(rest.stretch, least) / Math.max(now.stretch, least));
  const sa = (now.girth / rest.girth) * keep, sb = sa * (now.aspect / rest.aspect);
  return { t: now.p, r: now.q, s: [1, sb, sa] };
}

/**
 * DESIGN.md 16's prediction of the skin's geometric error at one time, before any vertex is compared: per segment the
 * change of the Catmull-Rom sagitta `(p_j + p_{j+1} - p_{j-1} - p_{j+2}) / 16` against the rest's carried rigidly, plus
 * the blend's shrink `r d theta^2 / 8` for the change d theta of the rotation between neighbours.
 */
function predictSkin(now: readonly JointRow[], rest: readonly JointRow[]): number {
  let worst = 0;
  for (let j = 1; j + 2 < now.length; j++) {
    const sag = (s: readonly JointRow[]): V3 => sub3(sub3([s[j]!.p[0] + s[j + 1]!.p[0], s[j]!.p[1] + s[j + 1]!.p[1], s[j]!.p[2] + s[j + 1]!.p[2]], s[j - 1]!.p), s[j + 2]!.p).map((x) => x / 16) as V3;
    const carry = qmul(now[j]!.q, qconj(rest[j]!.q));
    const e1 = len3(sub3(sag(now), qrot(carry, sag(rest))));
    const rel = (s: readonly JointRow[]) => qmul(qconj(s[j]!.q), s[j + 1]!.q);
    const dth = qangle(qmul(rel(now), qconj(rel(rest))));
    const r = Math.max(now[j]!.a, now[j]!.b, now[j + 1]!.a, now[j + 1]!.b);
    worst = Math.max(worst, e1 + (r * dth * dth) / 8);
  }
  return worst;
}

export type AnimateOptions = { readonly density: Density; readonly seconds: number | undefined };

/** Bakes a scene at rest and keys every body's motion over its period (or `seconds`). */
export function bakeAnimated(scene: Scene, options: AnimateOptions): Animated {
  const spanFrom = options.seconds !== undefined ? "seconds" : scene.loop.kind === "every" ? "period" : undefined;
  if (spanFrom === undefined) throw new SceneError("bake --animated", "a scene with no period and no --seconds", "a span to key over", "pass --seconds S (or give the scene a period)");
  const span = options.seconds ?? (scene.loop.kind === "every" ? scene.loop.seconds : NaN);
  if (!(span > 0 && Number.isFinite(span))) throw new SceneError("bake --animated", `a span of ${span} s`, "a positive number of seconds", "pass --seconds S > 0");

  const prep = prepareBake(scene, options.density);
  const kinds = prep.samplers.map(kindOf);
  const frame0 = prep.frameAt(0);
  const bodies = prep.samplers.map((s) => s.mesh(frame0));
  const sizes = bodies.map((b) => boxDiagonal(b.positions));
  const sampled = kinds.map((k) => k.kind !== "field");
  const pointsAt = (t: number) => { const fr = prep.frameAt(t); return { frame: fr, points: prep.samplers.map((s, i) => (sampled[i] ? s.positions(fr) : bodies[i]!.positions)) }; };

  // The pilot: how far and how fast each thing moves (second differences over PILOT_KEYS keys).
  const dt0 = span / PILOT_KEYS;
  const pilot = Array.from({ length: PILOT_KEYS + 1 }, (_, i) => (i === 0 ? bodies.map((b) => b.positions) : pointsAt(i * dt0).points));
  const looped = spanFrom === "period";
  const accel = bodies.map((_, k) => {
    if (!sampled[k]) return 0;
    let worst = 0;
    for (let i = looped ? 0 : 1; i < PILOT_KEYS; i++) {
      // On a loop, key PILOT_KEYS is key 0 again, so key -1 is key PILOT_KEYS - 1.
      const a = pilot[(i + PILOT_KEYS - 1) % PILOT_KEYS]![k]!, b = pilot[i]![k]!, c = pilot[i + 1]![k]!;
      for (let v = 0; v < b.length; v += 3) worst = Math.max(worst, Math.hypot(a[v]! - 2 * b[v]! + c[v]!, a[v + 1]! - 2 * b[v + 1]! + c[v + 1]!, a[v + 2]! - 2 * b[v + 2]! + c[v + 2]!));
    }
    return worst / (dt0 * dt0);
  });
  const rigs = kinds.map((k) => (k.kind === "chain" ? rigOf(k.body, prep.compiled.cpu) : undefined));
  const motions = kinds.map((kind, k): Motion => {
    const size = sizes[k]!;
    switch (kind.kind) {
      case "field": return { kind: "still", why: "a field, meshed once by surface nets (the field baker refuses one that changes)", moved: 0, size };
      case "chain": return keyThing(k, size, rigs[k]!, undefined);
      case "free": return keyThing(k, size, undefined, kind.why);
    }
  });
  return { bodies, motions, skipped: prep.skipped, span, spanFrom };

  /**
   * One thing's keys and its measured error. Each thing has its own keys (glTF samplers each have their own times), so
   * a blinking lid does not multiply the keys of a slow leg: N = max(PILOT_KEYS, the least that holds half the budget
   * by `a dt^2 / 8 <= tol size / 2`), a its pilot acceleration; N = PILOT_KEYS reuses the pilot as the keys.
   */
  function keyThing(k: number, size: number, rig: Rig | undefined, unchained: string | undefined): Motion {
    const allowed = allowance(size), a = accel[k]!, sampler = prep.samplers[k]!, rest = bodies[k]!.positions;
    const wanted = a > 0 ? Math.ceil(span * Math.sqrt(a / (4 * allowed))) : PILOT_KEYS;
    const sample = (N: number) => {
      const dt = span / N;
      const times = Array.from({ length: N + 1 }, (_, i) => (i === N ? span : i * dt));
      const keys = times.map((t, i) => (i === 0 ? rest : N === PILOT_KEYS ? pilot[i]![k]! : sampler.positions(prep.frameAt(t))));
      const mids = Array.from({ length: N }, (_, i) => sampler.positions(prep.frameAt((i + 0.5) * dt)));
      // The keys' own error: a straight blend between neighbouring keys against the scene half way (the morph's error).
      const misses = mids.map((p, i) => farthest(keys[i]!.map((x, v) => (x + keys[i + 1]![v]!) / 2), p));
      const between = Math.max(...misses);
      return { N, dt, times, keys, mids, between, worstAt: (misses.indexOf(between) + 0.5) * dt };
    };
    const first = sample(Math.min(Math.max(PILOT_KEYS, wanted), MOST_KEYS));
    let s = first;
    // The pilot's second differences average the acceleration over two of its steps, so a quick move (a blink, a
    // snap) reads low. The keys' own midpoint error is that bound measured: a_eff = 8 e / dt^2. Once, re-key from it.
    if (s.between > allowed / 2 && s.N < MOST_KEYS) s = sample(Math.min(MOST_KEYS, Math.ceil(s.N * Math.sqrt(s.between / (allowed / 2)))));
    const { N, dt, times, keys: keyPoints, mids: midPoints } = s;
    const moved = Math.max(...keyPoints.map((p) => farthest(p, rest)), ...midPoints.map((p) => farthest(p, rest)));
    if (moved <= allowed) return { kind: "still", why: `moves at most ${moved.toExponential(2)} over the span`, moved, size };
    const timeTerm = (a * dt * dt) / 8;

    let why = unchained ?? "";
    let turned: { readonly turn: number; readonly at: number } | undefined;
    if (rig !== undefined) {
      // Joints: the stations, or r - 1 more between each pair when the stations' predicted error is over half the
      // budget (both terms of DESIGN.md 16 go as the joint spacing squared, so r = ceil(sqrt(e / (tol / 2)))).
      const frames = times.map((t) => prep.frameAt(t));
      const atStations = frames.map((fr) => rig.read(fr, rig.stations));
      const coarse = Math.max(...atStations.map((rows) => predictSkin(rows, atStations[0]!)));
      const r = Math.min(MOST_REFINE, Math.max(1, Math.ceil(Math.sqrt(coarse / (allowed / 2)))));
      const keyRows = r === 1 ? atStations : frames.map((fr) => rig.read(fr, (rig.stations - 1) * r + 1));
      turned = largestTurn(keyRows, times);
      const skinned = skinOf(k, rig, keyRows, times);
      if (typeof skinned === "object") {
        const atKeys = Math.max(...keyPoints.map((p, i) => farthest(skin(rest, skinned, times[i]!), p)));
        const betweenKeys = Math.max(...midPoints.map((p, i) => farthest(skin(rest, skinned, (i + 0.5) * dt), p)));
        const predicted = Math.max(...keyRows.map((rows) => predictSkin(rows, keyRows[0]!))) + timeTerm;
        const error: MotionError = { atKeys, betweenKeys, size, predicted };
        if (Math.max(atKeys, betweenKeys) <= allowed) return { ...skinned, error };
        why = `its skin (${keyRows[0]!.length} joints) misses by ${Math.max(atKeys, betweenKeys).toExponential(2)}, over ${allowed.toExponential(2)} (predicted ${predicted.toExponential(2)})`;
      } else why = skinned;
    }

    // Morph targets: exact at the keys, a straight blend between them.
    const error: MotionError = { atKeys: 0, betweenKeys: s.between, size, predicted: timeTerm };
    if (s.between <= allowed) return { kind: "morphed", why, times, deltas: keyPoints.map((p) => p.map((x, v) => x - rest[v]!)), error };
    // A sweep whose roll jumps (a seed `up` along the spine, so the frame's start is decided by rounding) turns half a
    // turn in one key, and no number of keys follows it.
    const rolled = turned !== undefined && turned.turn > Math.PI / 2 ? `; a joint turns ${turned.turn.toFixed(2)} rad between the keys at ${turned.at.toFixed(3)} s (the sweep's frame jumps there: is its \`up\` along the spine?)` : "";
    // Smooth motion's midpoint error falls as dt^2; one that falls slower has a kink (as dt) or a jump (not at all).
    const fell = s === first ? "" : ` (${first.N} keys missed by ${first.between.toExponential(2)}: the error falls as dt^${(Math.log(first.between / s.between) / Math.log(s.N / first.N)).toFixed(1)}, where smooth motion falls as dt^2)`;
    return { kind: "refused", why: `${why}; and ${N} keys${wanted > MOST_KEYS ? ` (of ${wanted} wanted)` : ""} miss by ${s.between.toExponential(2)} between them, most at ${s.worstAt.toFixed(3)} s${fell}${rolled}`, error };
  }

  /** Joints and weights for a chain, or why it has none: a chain whose rest section has no width cannot be scaled from it. */
  function skinOf(k: number, rig: Rig, keyRows: readonly JointRow[][], times: readonly number[]): Omit<Extract<Motion, { kind: "skinned" }>, "error"> | string {
    const rest = keyRows[0]!;
    const flat = rest.findIndex((s) => !(s.girth > 0 && s.aspect > 0));
    if (flat >= 0) return `station ${flat} has girth ${rest[flat]!.girth} and aspect ${rest[flat]!.aspect} at rest, so no joint scale reaches its width later`;
    const keys: JointKey[][] = [];
    for (const rows of keyRows) {
      const before = keys[keys.length - 1];
      keys.push(rows.map((now, j) => {
        const key = jointKey(now, rest[j]!, rig.leastStretch);
        // q and -q are one rotation; keep each key on the previous key's side so LINEAR rotation takes the short way.
        const p = before?.[j]!.r ?? rest[j]!.q, d = p[0] * key.r[0] + p[1] * key.r[1] + p[2] * key.r[2] + p[3] * key.r[3];
        return d < 0 ? { ...key, r: key.r.map((x) => -x) as Quat } : key;
      }));
    }
    const n = rest.length, inverseBind = new Float32Array(16 * n);
    rest.forEach((s, j) => inverseBind.set(inverseRigid(s.p, s.q), 16 * j));
    const uv = bodies[k]!.uvs, count = uv.length / 2;
    const joints = new Uint16Array(4 * count), weights = new Float32Array(4 * count);
    for (let v = 0; v < count; v++) {
      // Linear in m between joints j and j + 1, fr = m (n - 1) - j: at the stations, stationSample's own nlerp and
      // stretch lerp; between them (finer joints), the same blend over a shorter span.
      const x = Math.min(Math.max(uv[2 * v]!, 0), 1) * (n - 1), j = Math.min(Math.floor(x), n - 2), fr = x - j;
      // Only joints that weigh something are named (glTF: an unused slot is joint 0 with weight 0).
      ([[j, 1 - fr], [j + 1, fr]] as const).filter(([, w]) => w > 0).forEach(([jj, w], slot) => { joints[4 * v + slot] = jj; weights[4 * v + slot] = w; });
    }
    return { kind: "skinned", times, keys, joints, weights, inverseBind };
  }
}

/** One line per body: what it became and how far its motion is from the scene's. */
export function motionSummary(a: Animated): string[] {
  const rel = (e: MotionError) => {
    const worst = Math.max(e.atKeys, e.betweenKeys);
    return `error ${worst.toExponential(2)} (${(worst / e.size).toExponential(2)} of ${e.size.toPrecision(3)})${e.predicted === undefined ? "" : `, predicted ${e.predicted.toExponential(2)}`}`;
  };
  return a.motions.map((m, k) => {
    const name = a.bodies[k]!.name.padEnd(18);
    switch (m.kind) {
      case "still": return `${name} still     ${m.why}`;
      case "skinned": return `${name} skinned   ${m.keys[0]!.length} joints, ${m.times.length - 1} keys, ${rel(m.error)} [keys ${m.error.atKeys.toExponential(2)}, between ${m.error.betweenKeys.toExponential(2)}]`;
      case "morphed": return `${name} morphed   ${m.deltas.length} targets, ${rel(m.error)}: ${m.why}`;
      case "refused": return `${name} REFUSED   written at rest, ${rel(m.error)}: ${m.why}`;
    }
  });
}
