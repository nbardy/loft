// Checks: arithmetic on the CPU from the same description the GPU draws (DESIGN.md 8). A scene module exports
// `checks`; `loft check <scene.ts>` compiles the scene, runs them through `compileScene(...).cpu` over posed frames
// and prints a table. Each check is a value with a name and one way to measure; none of them renders anything.
//
// Skeletons are what is measured: a body's chain of capsules (the same chain that casts its shadow). Their radii are
// the larger of the section's half-width and half-height, so distances are conservative for flattened sections.

import type { Artefact, Camera, FrameState } from "./artefact.ts";
import { compileScene, type Compiled, type CpuFn } from "./compile/index.ts";
import { Expr, lift, time, variable, vec2, vec4, type In, type TableFrame } from "./expr.ts";
import { cameraMatrices, project } from "./host/camera.ts";
import type { Body } from "./map/sweep.ts";
import type { Skeleton } from "./map/types.ts";
import type { Scene } from "./scene.ts";
import type { Arming, Attack, Effect, Hold, Motion, Part, Ring, Strike, Target } from "./spacetime/attack.ts";
import type { Thrown } from "./geom/grab.ts";
import { hitSdf } from "./spacetime/hit.ts";
import { hullBoxDistance, hullDistance, type HullDistance } from "./geom/hull_distance.ts";

type V3 = readonly [number, number, number];
type Joint = { readonly p: V3; readonly r: number };
type Inputs = Readonly<Record<string, number | readonly number[]>>;

/** What a check reports. `value` is the measured number the verdict was taken on. */
export type Verdict = { readonly name: string; readonly pass: boolean; readonly value: number; readonly measured: string; readonly limit: string };

/** Measurements over one compiled scene, with inputs held at the given values (time aside). */
export class Probe {
  private readonly frames = new Map<number, TableFrame>();
  private readonly fns = new Map<readonly Expr[], CpuFn>();
  constructor(readonly compiled: Compiled, readonly inputs: Inputs) {}

  get artefact(): Artefact { return this.compiled.artefact; }

  state(t: number): FrameState {
    const a = this.artefact;
    const inputs = new Float32Array(Math.max(1, a.inputFloats));
    for (const s of a.inputs) {
      const set = s.name === "time" ? [t] : this.inputs[s.name];
      const v = set === undefined ? s.def : typeof set === "number" ? [set] : set;
      if (v.length !== s.def.length) throw new Error(`check: input "${s.name}" takes ${s.def.length} numbers, got ${v.length}`);
      inputs.set(v, s.slot);
    }
    for (const name of Object.keys(this.inputs)) if (!a.inputs.some((s) => s.name === name)) throw new Error(`check: the scene has no input "${name}"`);
    return { time: t, inputs, params: Float32Array.from(a.params.map((p) => p.def)), curves: a.curves };
  }

  /** Values of expressions at time t (free variables bound by `vars`). */
  eval(roots: readonly Expr[], t: number, vars: Record<string, number> = {}): (number | number[])[] {
    let fn = this.fns.get(roots);
    if (!fn) { fn = this.compiled.cpu(roots); this.fns.set(roots, fn); }
    let frame = this.frames.get(t);
    if (!frame) { frame = this.compiled.frame(this.state(t)); this.frames.set(t, frame); }
    return fn(frame, vars) as (number | number[])[];
  }

  scalar(e: In<"f32">, t = 0): number { return this.eval([lift<"f32">(e)], t)[0] as number; }

  /** A compiled function of the plane, for evaluating a distance many times at one instant. */
  planar(e: (p: Expr<"vec2">) => Expr<"f32">, t: number): (x: number, z: number) => number {
    const roots = [e(vec2(variable("check_x", "f32"), variable("check_z", "f32")))];
    const vars = { check_x: 0, check_z: 0 };
    return (x, z) => { vars.check_x = x; vars.check_z = z; return this.eval(roots, t, vars)[0] as number; };
  }

  joints(s: Skeleton, range: readonly [number, number], t: number): Joint[] {
    const [lo, hi] = range;
    let roots = this.skeletonRoots.get(s);
    if (!roots) { roots = Array.from({ length: s.joints }, (_, k) => { const j = s.joint(k); return vec4(j.p, j.r); }); this.skeletonRoots.set(s, roots); }
    const all = this.eval(roots, t) as number[][];
    return all.map((v) => ({ p: [v[0], v[1], v[2]] as V3, r: v[3] })).slice(lo, hi + 1);
  }
  private readonly skeletonRoots = new Map<Skeleton, Expr[]>();
}

export interface Check {
  readonly name: string;
  run(probe: Probe): Verdict;
}

/** Something with a skeleton to measure: a whole body, or a part (its stretch of joints). */
export type Measured = Body | Part;
function chainOf(x: Measured): { readonly skeleton: Skeleton; readonly range: readonly [number, number]; readonly name: string } {
  if ("skeleton" in x) return { skeleton: x.skeleton, range: [0, x.skeleton.joints - 1], name: x.name };
  const n = x.body.skeleton.joints - 1;
  const lo = Math.floor(x.from * n), hi = Math.ceil(x.to * n);
  return { skeleton: x.body.skeleton, range: [lo, Math.max(hi, lo + 1)], name: x.name };
}

// ---- geometry -----------------------------------------------------------------------------------------------------

const subV = (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const dotV = (a: V3, b: V3): number => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const clamp01 = (x: number): number => Math.min(1, Math.max(0, x));

/** Closest parameters (s, t) on segments p0-p1 and q0-q1 (Ericson, Real-Time Collision Detection 5.1.9). */
function closest(p0: V3, p1: V3, q0: V3, q1: V3): [number, number] {
  const d1 = subV(p1, p0), d2 = subV(q1, q0), r = subV(p0, q0);
  const a = dotV(d1, d1), e = dotV(d2, d2), f = dotV(d2, r);
  if (a <= 1e-12 && e <= 1e-12) return [0, 0];
  if (a <= 1e-12) return [0, clamp01(f / e)];
  const c = dotV(d1, r);
  if (e <= 1e-12) return [clamp01(-c / a), 0];
  const b = dotV(d1, d2), den = a * e - b * b;
  let s = den > 1e-12 ? clamp01((b * f - c * e) / den) : 0;
  let t = (b * s + f) / e;
  if (t < 0) { t = 0; s = clamp01(-c / a); } else if (t > 1) { t = 1; s = clamp01((b - c) / a); }
  return [s, t];
}
const lerp = (a: V3, b: V3, s: number): V3 => [a[0] + (b[0] - a[0]) * s, a[1] + (b[1] - a[1]) * s, a[2] + (b[2] - a[2]) * s];

/** Least surface-to-surface distance between two chains of tapered capsules (negative: they overlap). */
function chainGap(a: readonly Joint[], b: readonly Joint[]): number {
  let best = Infinity;
  for (let i = 0; i + 1 < a.length; i++) for (let j = 0; j + 1 < b.length; j++) {
    const [s, t] = closest(a[i].p, a[i + 1].p, b[j].p, b[j + 1].p);
    const pa = lerp(a[i].p, a[i + 1].p, s), pb = lerp(b[j].p, b[j + 1].p, t);
    const ra = a[i].r + (a[i + 1].r - a[i].r) * s, rb = b[j].r + (b[j + 1].r - b[j].r) * t;
    best = Math.min(best, Math.hypot(...subV(pa, pb)) - ra - rb);
  }
  return best;
}

/** Least distance in the water plane from a chain's footprint to a disc (a vertical cylinder). */
function discGap(a: readonly Joint[], centre: readonly [number, number], radius: number): number {
  let best = Infinity;
  const c: V3 = [centre[0], 0, centre[1]];
  for (let i = 0; i + 1 < a.length; i++) {
    const p0: V3 = [a[i].p[0], 0, a[i].p[2]], p1: V3 = [a[i + 1].p[0], 0, a[i + 1].p[2]];
    const [s] = closest(p0, p1, c, c);
    best = Math.min(best, Math.hypot(...subV(lerp(p0, p1, s), c)) - (a[i].r + (a[i + 1].r - a[i].r) * s) - radius);
  }
  return best;
}

/** Distance from a chain to a target at time t. */
function gap(probe: Probe, chain: readonly Joint[], target: Target, t: number): number {
  switch (target.kind) {
    case "disc": {
      const [c, r] = probe.eval([target.centre, target.radius], t) as [number[], number];
      return discGap(chain, [c[0], c[1]], r);
    }
    case "body": {
      const b = chainOf(target.body);
      return chainGap(chain, probe.joints(b.skeleton, b.range, t));
    }
    case "hull": {
      const f = target.frame;
      const [o, x, y, z] = probe.eval([f.o, f.x, f.y, f.z], t) as unknown as V3[];
      const local = (p: V3): V3 => { const d = subV(p, o!); return [dotV(d, x!), dotV(d, y!), dotV(d, z!)]; };
      return hullGap(chain.map((j) => ({ p: local(j.p), r: j.r })), hullDistance(target.hull));
    }
  }
}

/**
 * Least distance from a chain of tapered capsules to a hull (negative: inside it): the hull's signed distance at points
 * along each capsule's axis, no farther apart than half the smaller radius, less the radius there. Exact to that
 * spacing (the distance is 1-Lipschitz, so between samples it can dip by at most a quarter radius below them).
 */
function hullGap(a: readonly Joint[], h: HullDistance): number {
  let best = Infinity;
  for (let i = 0; i + 1 < a.length; i++) {
    const p0 = a[i]!.p, p1 = a[i + 1]!.p, r0 = a[i]!.r, r1 = a[i + 1]!.r;
    const n = Math.max(1, Math.ceil(Math.hypot(...subV(p1, p0)) / Math.max(0.5 * Math.min(r0, r1), 0.01)));
    for (let k = 0; k <= n; k++) {
      const s = k / n, p = lerp(p0, p1, s), r = r0 + (r1 - r0) * s;
      // The box is a lower bound: a sample that cannot beat the best so far is not measured.
      if (hullBoxDistance(h, p) - r >= best) continue;
      best = Math.min(best, h.distance(p) - r);
    }
  }
  return best;
}

/** Times from t0 to t1 (inclusive of t0, exclusive of t1) at the check rate. */
function times(t0: number, t1: number, rate: number): number[] {
  const n = Math.max(1, Math.ceil((t1 - t0) * rate));
  return Array.from({ length: n }, (_, i) => t0 + i / rate);
}

const fmt = (x: number, digits = 3): string => (Number.isFinite(x) ? x.toFixed(digits) : String(x));

/**
 * The share of a chain's footprint on the water (its capsules seen from above) for which `outside(x, z)` holds,
 * sampled on a grid `step` apart.
 */
export function footprintShare(chain: readonly Joint[], outside: (x: number, z: number) => boolean, step: number): { readonly share: number; readonly area: number } {
  const xs = chain.flatMap((j) => [j.p[0] - j.r, j.p[0] + j.r]), zs = chain.flatMap((j) => [j.p[2] - j.r, j.p[2] + j.r]);
  const [x0, x1, z0, z1] = [Math.min(...xs), Math.max(...xs), Math.min(...zs), Math.max(...zs)];
  let inside = 0, out = 0;
  for (let x = x0 + step / 2; x < x1; x += step) for (let z = z0 + step / 2; z < z1; z += step) {
    if (discGap(chain, [x, z], 0) > 0) continue;
    inside++;
    if (outside(x, z)) out++;
  }
  if (inside === 0) throw new Error("footprintShare: the chain has no footprint at this sampling step");
  return { share: out / inside, area: inside * step * step };
}

// ---- the checks ---------------------------------------------------------------------------------------------------

/**
 * A time a check is given: seconds, or an attack's arming (`attack.arming`), which a check measures at its first
 * arming time. An attack that repeats arms at `attack.arm`, an expression of `time`; evaluated at time 0 that was a
 * cycle that had already passed or not yet begun, so checks read the arming's first time instead.
 */
export type CheckTime = In<"f32"> | Arming;
function timeOf(probe: Probe, t: CheckTime): number {
  if (typeof t === "number" || t instanceof Expr) return probe.scalar(t);
  switch (t.kind) {
    case "once": return probe.scalar(t.at);
    case "every": return probe.scalar(t.first);
  }
}

/**
 * The striking part's footprint at the arming time lies within the hit shape: at most `tolerance` of it outside. An
 * attack that repeats is measured at its first arming.
 */
export function landsInside(strike: Strike, o: { readonly tolerance?: number; readonly step?: number } = {}): Check {
  const tol = o.tolerance ?? 0.02, step = o.step ?? 0.03;
  const a = strike.attack, c = chainOf(strike.part);
  return {
    name: `${a.name}: ${c.name} lands inside the hit shape`,
    run(probe) {
      const T = probe.scalar(a.first);
      const chain = probe.joints(c.skeleton, c.range, T);
      const sdf = probe.planar((p) => hitSdf(a.hit, p), T);
      const { share } = footprintShare(chain, (x, z) => sdf(x, z) > 0, step);
      return { name: this.name, pass: share <= tol, value: share, measured: `${(share * 100).toFixed(1)}% outside`, limit: `<= ${(tol * 100).toFixed(1)}%` };
    },
  };
}

/** The least distance between `body` and `target` from `from` until (not including) `until` is at least `margin`. */
export function staysClear(body: Measured, target: Target, o: { readonly until: CheckTime; readonly from?: CheckTime; readonly margin?: number; readonly rate?: number; readonly label?: string }): Check {
  const c = chainOf(body), margin = o.margin ?? 0, rate = o.rate ?? 60;
  return {
    name: o.label ?? `${c.name} stays clear of the target`,
    run(probe) {
      const t1 = timeOf(probe, o.until), t0 = o.from === undefined ? 0 : timeOf(probe, o.from);
      let least = Infinity, when = t0;
      for (const t of times(t0, t1, rate)) {
        const g = gap(probe, probe.joints(c.skeleton, c.range, t), target, t);
        if (g < least) { least = g; when = t; }
      }
      return { name: this.name, pass: least >= margin, value: least, measured: `${fmt(least)} at t=${fmt(when, 2)} (${fmt(t0, 2)}..${fmt(t1, 2)})`, limit: `>= ${fmt(margin)}` };
    },
  };
}

/**
 * The striking part reaches the target at the arming time (within `contact`) and never enters it over the whole
 * attack (it stops at the hull: the least distance is at least `-tolerance`). An attack that repeats is measured over its
 * first arming.
 */
export function stopsAt(strike: Strike, target: Target, o: { readonly contact?: number; readonly tolerance?: number; readonly rate?: number } = {}): Check {
  const a = strike.attack, c = chainOf(strike.part), contact = o.contact ?? 0.6, tol = o.tolerance ?? 0.05, rate = o.rate ?? 60;
  return {
    name: `${a.name}: ${c.name} stops at the target`,
    run(probe) {
      const T = probe.scalar(a.first);
      const at = gap(probe, probe.joints(c.skeleton, c.range, T), target, T);
      let least = Infinity;
      for (const t of times(T - a.span.before, T + a.span.after, rate)) least = Math.min(least, gap(probe, probe.joints(c.skeleton, c.range, t), target, t));
      return {
        name: this.name, pass: at <= contact && least >= -tol, value: least,
        measured: `gap ${fmt(at)} at arming, least ${fmt(least)}`, limit: `gap <= ${fmt(contact)}, least >= ${fmt(-tol)}`,
      };
    },
  };
}

/** A feature (an eye) is at least `pixels` across, in its narrower direction, from a named camera at time `at`. */
export function minScreenSize(feature: Measured, camera: string, pixels: number, o: { readonly at?: CheckTime; readonly size?: readonly [number, number] } = {}): Check {
  const c = chainOf(feature), [w, h] = o.size ?? [1280, 800];
  return {
    name: `${c.name} from "${camera}" is at least ${pixels} px`,
    run(probe) {
      const cam: Camera | undefined = probe.artefact.cameras[camera];
      if (cam === undefined) throw new Error(`minScreenSize: the scene has no camera "${camera}" (cameras: ${Object.keys(probe.artefact.cameras).join(", ")})`);
      const t = o.at === undefined ? 0 : timeOf(probe, o.at);
      const m = cameraMatrices(cam, w / h);
      const f = m.forward, right = norm([-f[2], 0, f[0]]), up = cross(right, f);
      let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity;
      for (const j of probe.joints(c.skeleton, c.range, t)) for (const d of [right, up]) for (const s of [-1, 1]) {
        const q = project(m, [j.p[0] + d[0] * j.r * s, j.p[1] + d[1] * j.r * s, j.p[2] + d[2] * j.r * s], w, h);
        x0 = Math.min(x0, q.x); x1 = Math.max(x1, q.x); y0 = Math.min(y0, q.y); y1 = Math.max(y1, q.y);
      }
      const across = Math.min(x1 - x0, y1 - y0);
      return { name: this.name, pass: across >= pixels, value: across, measured: `${fmt(across, 1)} px at t=${fmt(t, 2)}`, limit: `>= ${pixels} px` };
    },
  };
}
const norm = (a: V3): V3 => { const l = Math.hypot(...a); return [a[0] / l, a[1] / l, a[2] / l]; };
const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];

// ---- holds --------------------------------------------------------------------------------------------------------

/**
 * How far a measurement may be from what it should be: a distance in world units, or a share of a reference that the
 * check taking it names (for `gripReaches`, the held thing's width). A value, so a check never takes a bare number.
 */
export type Tolerance = { readonly kind: "units"; readonly units: number } | { readonly kind: "share"; readonly share: number };
export const tolerance = {
  units: (units: number): Tolerance => {
    if (!(units >= 0 && Number.isFinite(units))) throw new Error(`tolerance.units: need a finite distance >= 0, got ${units}`);
    return { kind: "units", units };
  },
  share: (share: number): Tolerance => {
    if (!(share >= 0 && Number.isFinite(share))) throw new Error(`tolerance.share: need a finite share >= 0, got ${share}`);
    return { kind: "share", share };
  },
};
/** The distance a tolerance allows, given the reference its check names. */
function allowance(t: Tolerance, reference: number): number {
  switch (t.kind) {
    case "units": return t.units;
    case "share": return t.share * reference;
  }
}
const describe = (t: Tolerance, allowed: number, of: string): string => {
  switch (t.kind) {
    case "units": return fmt(allowed);
    case "share": return `${fmt(allowed)} (${(t.share * 100).toFixed(0)}% of ${of})`;
  }
};

/** The point of a chain's surface nearest p (the chain's capsules, tapered), and how far p is from it (negative inside). */
function nearestSurface(chain: readonly Joint[], p: V3): { readonly at: V3; readonly d: number } {
  let best = { at: chain[0]!.p, d: Infinity };
  for (let i = 0; i + 1 < chain.length; i++) {
    const [s] = closest(chain[i]!.p, chain[i + 1]!.p, p, p);
    const q = lerp(chain[i]!.p, chain[i + 1]!.p, s), r = chain[i]!.r + (chain[i + 1]!.r - chain[i]!.r) * s;
    const to = subV(p, q), len = Math.hypot(...to);
    if (len - r < best.d) best = { at: len > 1e-9 ? [q[0] + (to[0] / len) * r, q[1] + (to[1] / len) * r, q[2] + (to[2] / len) * r] : q, d: len - r };
  }
  return best;
}

/**
 * A two-sided gripper (a claw's fingers, a jaw's halves) reaches the thing its hold takes, and keeps it: at the closing
 * instant (the arming time) and at every sampled instant of the hold, the point of each drawn finger's surface nearest
 * the thing's middle is found. The grip's middle (halfway between those points) must be within `middle` of the thing's
 * middle, and their distance apart within `gap` of the thing's width times its crush. Shares are of the thing's width.
 * It measures the drawn fingers, not the attack's own grip, which matches by construction. Measured over the first arming.
 */
export function gripReaches(attack: Attack<Motion, Hold>, fingers: readonly [Measured, Measured], o: { readonly middle: Tolerance; readonly gap: Tolerance; readonly rate?: number }): Check {
  const [a, b] = fingers.map(chainOf) as [ReturnType<typeof chainOf>, ReturnType<typeof chainOf>], rate = o.rate ?? 30;
  const roots = [attack.thing(), attack.crush(), attack.effect.radius];
  return {
    name: `${attack.name}: ${a.name} and ${b.name} close on the thing and keep it`,
    run(probe) {
      const T = probe.scalar(attack.first);
      const sample = (t: number) => {
        const [thing, crush, radius] = probe.eval(roots, t) as [number[], number, number];
        const c: V3 = [thing[0]!, thing[1]!, thing[2]!];
        const pa = nearestSurface(probe.joints(a.skeleton, a.range, t), c).at, pb = nearestSurface(probe.joints(b.skeleton, b.range, t), c).at;
        const mid: V3 = [(pa[0] + pb[0]) / 2, (pa[1] + pb[1]) / 2, (pa[2] + pb[2]) / 2];
        const apart = Math.hypot(...subV(pa, pb)), want = 2 * radius * crush;
        return { t, width: 2 * radius, middle: Math.hypot(...subV(mid, c)), apart, want, gap: Math.abs(apart - want) };
      };
      const close = sample(T);
      const okMiddle = allowance(o.middle, close.width), okGap = allowance(o.gap, close.width);
      // The worst instant is the one nearest to failing, on either measure.
      const score = (s: typeof close) => Math.max(s.middle / okMiddle, s.gap / okGap);
      const worst = times(T, T + attack.effect.seconds, rate).map(sample).reduce((w, s) => (score(s) > score(w) ? s : w), close);
      const line = (s: typeof close) => `middle ${fmt(s.middle)} off, ${fmt(s.apart, 2)} apart (want ${fmt(s.want, 2)})`;
      return {
        name: this.name, pass: score(worst) <= 1, value: score(worst),
        measured: `closing: ${line(close)}; worst t=${fmt(worst.t, 2)}: ${line(worst)}`,
        limit: `middle <= ${describe(o.middle, okMiddle, "width")}, gap within ${describe(o.gap, okGap, "width")}`,
      };
    },
  };
}

/**
 * The warning comes first and covers what is taken: it starts no later than the motion's first movement
 * (`warn >= span.before`), and for a hold the thing's footprint where it is taken (a disc of its radius round its
 * middle) lies inside the warned hit shape. Measured at the first arming.
 */
export function warned(attack: Attack): Check {
  return {
    name: `${attack.name}: the warning comes first and covers what is taken`,
    run(probe) {
      const lead = attack.span.before, warn = probe.scalar(attack.warn);
      const early = warn - lead;
      const cover = covered(probe, attack.effect, attack);
      const pass = early >= 0 && cover.inside;
      return { name: this.name, pass, value: early, measured: `warns ${fmt(warn, 2)} s, moves ${fmt(lead, 2)} s before arming; ${cover.measured}`, limit: `warning >= first movement; ${cover.limit}` };
    },
  };
}
/** What the warned shape must cover, by effect: a hit's shape is its own place; a hold's thing must be inside it. */
function covered(probe: Probe, e: Effect, a: Attack): { readonly inside: boolean; readonly measured: string; readonly limit: string } {
  switch (e.kind) {
    case "instant": return { inside: true, measured: "a hit (its shape is its place)", limit: "-" };
    case "hold": {
      const T = probe.scalar(a.first);
      const [at, r] = probe.eval([e.at, e.radius], T) as [number[], number];
      const d = probe.planar((p) => hitSdf(a.hit, p), T)(at[0]!, at[2]!) + r;
      return { inside: d <= 0, measured: `the thing's footprint ${fmt(-d)} inside the shape's edge`, limit: "footprint inside" };
    }
  }
}

// ---- throws -------------------------------------------------------------------------------------------------------

const lenV = (a: V3): number => Math.hypot(a[0], a[1], a[2]);
const asV3 = (a: number[]): V3 => [a[0]!, a[1]!, a[2]!];

/** One-sided second-order velocities at t from five samples h apart: exact for the two quadratics that meet at a release. */
function velocitiesAt(at: (t: number) => V3, t: number, h: number): { readonly here: V3; readonly before: V3; readonly after: V3 } {
  const p0 = at(t), b1 = at(t - h), b2 = at(t - 2 * h), a1 = at(t + h), a2 = at(t + 2 * h);
  return {
    here: p0,
    before: [0, 1, 2].map((k) => (3 * p0[k]! - 4 * b1[k]! + b2[k]!) / (2 * h)) as unknown as V3,
    after: [0, 1, 2].map((k) => (-3 * p0[k]! + 4 * a1[k]! - a2[k]!) / (2 * h)) as unknown as V3,
  };
}

/**
 * The hand-off a throw promises a game (docs/throw_and_shove.md, "The contract"): at the stated release instant the
 * thing is at the stated release point, and the gripper has brought it to the stated release velocity. Measured on the
 * attack's own `thing` before the release (the thing is in the grip, so this is the gripper's motion) from three
 * samples a 120th of a second apart, carried to the release instant, which is exact for a uniform acceleration. `speed` is how far the velocity
 * may be off (a share is of the release speed), `place` how far the point (a share of the stroke's length). It holds
 * whoever places the thing afterwards.
 */
export function releasesAt(attack: Attack<Motion, Hold<Thrown>>, o: { readonly speed: Tolerance; readonly place: Tolerance }): Check {
  const e = attack.effect, toss = attack.toss();
  const thing = [attack.thing()], stated = [toss.velocity, toss.release, toss.windback];
  return {
    name: `${attack.name}: the grip lets go where, when and as fast as it says`,
    run(probe) {
      const T = probe.scalar(attack.first), released = T + e.seconds;
      const [v0, release, windback] = (probe.eval(stated, T) as number[][]).map(asV3) as [V3, V3, V3];
      // Only instants before the release are sampled (at it the thing is already its placer's): the quadratic through
      // three of them, a 120th of a second apart, gives the place and the velocity at the release itself.
      const h = 1 / 120, [p1, p2, p3] = [1, 2, 3].map((k) => asV3(probe.eval(thing, released - k * h)[0] as number[])) as [V3, V3, V3];
      const here = [0, 1, 2].map((k) => 3 * p1[k]! - 3 * p2[k]! + p3[k]!) as unknown as V3;
      const before = [0, 1, 2].map((k) => (5 * p1[k]! - 8 * p2[k]! + 3 * p3[k]!) / (2 * h)) as unknown as V3;
      const speed = lenV(v0), off = lenV(subV(before, v0)), miss = lenV(subV(here, release));
      const okSpeed = allowance(o.speed, speed), okPlace = allowance(o.place, lenV(subV(release, windback)));
      return {
        name: this.name, pass: off <= okSpeed && miss <= okPlace, value: off,
        measured: `t=${fmt(released, 2)}: at (${release.map((x) => fmt(x, 2)).join(", ")}), ${fmt(miss)} off; velocity (${v0.map((x) => fmt(x, 2)).join(", ")}), ${fmt(speed, 2)}/s, the grip's ${fmt(off)} off`,
        limit: `velocity within ${describe(o.speed, okSpeed, "release speed")}; point within ${describe(o.place, okPlace, "stroke")}`,
      };
    },
  };
}

/**
 * The stand-in's flight is the throw the release promises, and it lands: measured on the attack's own `thing` over its
 * first arming (a hold whose thrown thing the stand-in places):
 * - the flight starts at the release point with the release velocity: just after the release its velocity (three
 *   samples a 120th of a second apart, exact for a ballistic flight) is within `speed` of the stated one (a share is
 *   of the release speed);
 * - through the flight it stays above the height it lands at, until it lands;
 * - at the stated landing time it is within `place` of the stated landing place (a share is of the distance flown);
 * - it lands at least `least` from where it was taken, measured along the water.
 */
export function throwLands(attack: Attack<Motion, Hold<Thrown>>, o: { readonly least: number; readonly speed: Tolerance; readonly place: Tolerance; readonly rate?: number }): Check {
  const rate = o.rate ?? 60, e = attack.effect, toss = attack.toss();
  const thing = [attack.thing()], stated = [toss.velocity, toss.release, toss.land, e.at, e.end.up];
  return {
    name: `${attack.name}: the thing flies from the release as released, and lands where and when stated`,
    run(probe) {
      const T = probe.scalar(attack.first), released = T + e.seconds, lands = released + e.end.flight;
      const at = (t: number): V3 => asV3(probe.eval(thing, t)[0] as number[]);
      const [v0, release, land, taken, up] = (probe.eval(stated, T) as number[][]).map(asV3) as [V3, V3, V3, V3, V3];
      const v = velocitiesAt(at, released, 1 / 120);
      const speed = lenV(v0), jump = Math.max(lenV(subV(v.after, v0)), lenV(subV(v.after, v.before)));
      const okSpeed = allowance(o.speed, speed);
      // Above the landing height from the release until the last sample before touchdown.
      let least = Infinity, when = released;
      for (const t of times(released, lands - 0.5 / rate, rate)) { const c = dotV(subV(at(t), land), up); if (c < least) { least = c; when = t; } }
      const flown = lenV(subV(land, release)), miss = Math.max(lenV(subV(at(lands), land)), lenV(subV(v.here, release))), okPlace = allowance(o.place, flown);
      const d = subV(land, taken), rise = dotV(d, up), far = Math.sqrt(Math.max(dotV(d, d) - rise * rise, 0));
      return {
        name: this.name, pass: jump <= okSpeed && least > 0 && miss <= okPlace && far >= o.least, value: far,
        measured: `leaves at ${fmt(speed, 2)}/s, ${fmt(jump)} off the release; least ${fmt(least, 2)} above landing height (t=${fmt(when, 2)}); lands ${fmt(miss)} off at t=${fmt(lands, 2)}, ${fmt(far, 2)} from where taken`,
        limit: `velocity within ${describe(o.speed, okSpeed, "release speed")}; above until it lands; within ${describe(o.place, okPlace, "distance flown")}; >= ${fmt(o.least, 2)} away`,
      };
    },
  };
}

/**
 * A thrown thing, as a ball of radius `size` round its middle, keeps at least `margin` clear of every target (the
 * thrower's own body, its shell) from its release to its touchdown. Measured over the first arming.
 */
export function fliesClear(attack: Attack<Motion, Hold<Thrown>>, targets: readonly Target[], o: { readonly size: number; readonly margin?: number; readonly rate?: number }): Check {
  const rate = o.rate ?? 60, margin = o.margin ?? 0, e = attack.effect, thing = [attack.thing()];
  if (targets.length < 1) throw new Error(`fliesClear "${attack.name}": no targets`);
  return {
    name: `${attack.name}: the thrown thing flies clear of ${targets.length} target${targets.length === 1 ? "" : "s"}`,
    run(probe) {
      const released = probe.scalar(attack.first) + e.seconds;
      let least = Infinity, when = released, which = 0;
      for (const t of times(released, released + e.end.flight, rate)) {
        const p = asV3(probe.eval(thing, t)[0] as number[]);
        // A ball as a capsule of no length: the gap measures are written for chains.
        const ball: Joint[] = [{ p, r: o.size }, { p: [p[0], p[1] + 1e-4, p[2]], r: o.size }];
        targets.forEach((target, k) => { const g = gap(probe, ball, target, t); if (g < least) { least = g; when = t; which = k; } });
      }
      return { name: this.name, pass: least >= margin, value: least, measured: `${fmt(least, 2)} from target ${which} (${targets[which]!.kind}) at t=${fmt(when, 2)}`, limit: `>= ${fmt(margin, 2)}` };
    },
  };
}

// ---- the aftermath on the water -----------------------------------------------------------------------------------

/**
 * The stand-in's shove is the wave's own impulse, and it is enough. Measured over the first arming for a thing afloat
 * at `place` (x, z):
 * - just after the front arrives, the stand-in's drift (`pushed`) moves at the impulse the wave states there
 *   (`wave.impulse(place)`, units per second), within `impulse` (a share is of that impulse): the stand-in is one
 *   consumer of the field a game samples, not a second formula;
 * - six drag times later it has come to `wave.shoved(place)` (to 0.5%), which is at least `least`.
 */
export function shoves(attack: Attack<Motion, Effect, Ring>, place: In<"vec2">, o: { readonly least: number; readonly impulse: Tolerance }): Check {
  const wave = attack.wave(), at = lift<"vec2">(place);
  const stated = [wave.shoved(at), wave.arrival(at), wave.impulse(at), at], pushed = [attack.pushed(at)];
  return {
    name: `${attack.name}: its ring shoves what floats at the place given, by its own impulse`,
    run(probe) {
      const T = probe.scalar(attack.first);
      const [end, arrival, kick, p] = probe.eval(stated, T) as [number[], number, number[], number[]];
      const drag = attack.aftermath.drag, front = T + arrival;
      const at2 = (t: number): V3 => { const v = probe.eval(pushed, t)[0] as number[]; return [v[0]!, 0, v[1]!]; };
      // The drift's velocity a thousandth of a drag time after the front, by a second-order one-sided difference.
      const h = drag / 400, t0 = front + h, v = velocitiesAt(at2, t0, h).after;
      const given = Math.hypot(kick[0]!, kick[1]!), decayed = Math.exp(-h / drag);
      const off = Math.hypot(v[0] - kick[0]! * decayed, v[2] - kick[1]! * decayed), okKick = allowance(o.impulse, given);
      const late = at2(front + 6 * drag), final = Math.hypot(end[0]!, end[1]!), short = Math.hypot(late[0] - end[0]!, late[2] - end[1]!);
      return {
        name: this.name, pass: final >= o.least && off <= okKick && short <= 0.005 * final, value: final,
        measured: `at (${fmt(p[0]!, 2)}, ${fmt(p[1]!, 2)}): front at t=${fmt(front, 2)} gives ${fmt(given, 2)}/s, the drift's start ${fmt(off)} off it; shoved ${fmt(final, 2)} (${fmt(Math.hypot(late[0], late[2]), 2)} by t=${fmt(front + 6 * drag, 2)})`,
        limit: `>= ${fmt(o.least, 2)}; impulse within ${describe(o.impulse, okKick, "impulse")}`,
      };
    },
  };
}

/**
 * Nothing moves before the front arrives: at each place, from a second before the arming time until the ring's front
 * reaches it, the shove and the wave's lift are exactly 0; and one drag time after it arrives the thing there has moved
 * (so the check is not passed by a wave that never comes). Measured over the first arming.
 */
export function frontFirst(attack: Attack<Motion, Effect, Ring>, places: readonly In<"vec2">[], o: { readonly rate?: number } = {}): Check {
  const rate = o.rate ?? 60, wave = attack.wave();
  if (places.length < 1) throw new Error(`frontFirst "${attack.name}": no places`);
  const roots = places.map((place) => { const at = lift<"vec2">(place); return { arrival: [wave.arrival(at)], moved: [attack.pushed(at), wave.lifts(at, time)] }; });
  return {
    name: `${attack.name}: nothing moves before its ring's front arrives`,
    run(probe) {
      const T = probe.scalar(attack.first);
      let early = 0, when = T, later = Infinity;
      for (const r of roots) {
        const arrival = probe.eval(r.arrival, T)[0] as number;
        for (const t of times(T - 1, T + arrival, rate)) {
          const [push, rise] = probe.eval(r.moved, t) as [number[], number];
          const m = Math.max(Math.abs(push[0]!), Math.abs(push[1]!), Math.abs(rise));
          if (m > early) { early = m; when = t; }
        }
        const [push] = probe.eval(r.moved, T + arrival + attack.aftermath.drag) as [number[], number];
        later = Math.min(later, Math.hypot(push[0]!, push[1]!));
      }
      return {
        name: this.name, pass: early === 0 && later > 0, value: early,
        measured: `most before the front ${early === 0 ? "0 (exactly)" : `${early.toExponential(2)} at t=${fmt(when, 2)}`}; a drag time after it, moved at least ${fmt(later, 2)}`,
        limit: "exactly 0 before; moving after",
      };
    },
  };
}

/** The compiled scene draws at most `triangles` triangles. */
export function budget(o: { readonly triangles: number }): Check {
  return {
    name: `triangle budget ${o.triangles}`,
    run(probe) {
      const n = probe.artefact.stats.maxTriangles;
      return { name: this.name, pass: n <= o.triangles, value: n, measured: `${n} triangles`, limit: `<= ${o.triangles}` };
    },
  };
}

// ---- running ------------------------------------------------------------------------------------------------------

/** Run checks over a scene with inputs held at the given values (defaults otherwise). */
export function runChecks(scene: Scene, checks: readonly Check[], o: { readonly inputs?: Inputs; readonly compiled?: Compiled } = {}): Verdict[] {
  const probe = new Probe(o.compiled ?? compileScene(scene), o.inputs ?? {});
  return checks.map((c) => c.run(probe));
}

/** The verdicts as a plain-text table. */
export function formatChecks(vs: readonly Verdict[]): string {
  const rows = [["check", "verdict", "measured", "limit"], ...vs.map((v) => [v.name, v.pass ? "pass" : "FAIL", v.measured, v.limit])];
  const widths = rows[0].map((_, i) => Math.max(...rows.map((r) => r[i].length)));
  const line = (r: readonly string[]) => r.map((x, i) => x.padEnd(widths[i])).join("  ").trimEnd();
  return [line(rows[0]), widths.map((w) => "-".repeat(w)).join("  "), ...rows.slice(1).map(line)].join("\n");
}
