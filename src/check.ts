// Checks: arithmetic on the CPU from the same description the GPU draws (DESIGN.md 8). A scene module exports
// `checks`; `loft check <scene.ts>` compiles the scene, runs them through `compileScene(...).cpu` over posed frames
// and prints a table. Each check is a value with a name and one way to measure; none of them renders anything.
//
// Skeletons are what is measured: a body's chain of capsules (the same chain that casts its shadow). Their radii are
// the larger of the section's half-width and half-height, so distances are conservative for flattened sections.

import type { Artefact, Camera, FrameState } from "./artefact.ts";
import { compileScene, type Compiled, type CpuFn } from "./compile/index.ts";
import { Expr, lift, variable, vec2, vec4, type In, type TableFrame } from "./expr.ts";
import { cameraMatrices, project } from "./host/camera.ts";
import type { Body } from "./map/sweep.ts";
import type { Skeleton } from "./map/types.ts";
import type { Scene } from "./scene.ts";
import type { Arming, Part, Strike, Target } from "./spacetime/attack.ts";
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
