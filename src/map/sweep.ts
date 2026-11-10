// The sweep (DESIGN.md 7.2, "multiply"): a section swept along a spine, with frames transported along the spine. It is
// a Surface (the skin, rho = 1), a Chart (body coordinates (m, v, rho)) and a Skeleton (capsules along the spine).
//
// Body coordinates: m in [0, 1] runs along the spine (its parameter), v in [0, 1] runs round it (v = 0 the +side
// line, 0.25 the dorsal line, 0.5 the other side, 0.75 the belly), rho is the distance out as a fraction of the
// thickness (the skin is rho = 1).
//
// Frames cannot be computed locally (they are transported along the spine), so the sweep tabulates its spine once per
// frame on the CPU (`transport.ts`) into a per-frame table, and everything else (the skin, its normal, the chart, the
// skeleton) is ordinary expressions over `fetch`es of that table. The vertex stage, the pose pass of parts riding the
// body, the shadows and the baker all evaluate the same expressions.

import {
  add, clamp, cos, cross, div, f, fetch, floor, lift, lt, ge, max, min, mix, mul, normalize, pow, saturate, select, sin, sqrt,
  sub, substitute, TAU, time, length, vec3, type Env, type Expr, type In, type TableFill, type TablePrep, type TableSource,
} from "../expr.ts";
import { compose, rotate, world } from "./chart.ts";
import { poseStrand, STATION_VEC4S, type V3 } from "./transport.ts";
import type { Chart, End, F, Frame, P3, Placeable, Skeleton, Surface, Topology } from "./types.ts";

type Fn1 = (m: F) => F;

/** How an end closes, canonical. Lengths are fractions of m. */
export type Tip =
  | { readonly kind: "round"; readonly length: number }
  | { readonly kind: "point"; readonly length: number }
  | { readonly kind: "open" };

/** A superellipse section: half-width `girth(m)` (tips not yet applied), height over width `aspect(m)`. */
export type Section = {
  readonly girth: Fn1;
  readonly aspect: Fn1;
  /** Superellipse exponent: 2 an ellipse, higher boxier. At least 1.2 (the normal needs |x|^(n-1)). */
  readonly squareness: number;
  readonly tips: readonly [Tip, Tip];
};

/** How frames are carried along the spine. */
export type Transport = {
  /** How many frames the pose pass computes. */
  readonly stations: number;
  /** Roll about the tangent, radians, added to the rotation-minimising frame. */
  readonly twist: Fn1;
  /** Direction the frame starts from at m = 0 (the dorsal side faces it). */
  readonly up: P3;
  /** Per world unit: how fast accumulated roll bleeds back toward `up`. 0 is the pure rotation-minimising frame. */
  readonly level: number;
};

/** An interpolated station: position, d(position)/dm, frame (tangent, dorsal, side), stretch. */
export type FrameSample = { readonly p: P3; readonly der: P3; readonly t: P3; readonly d: P3; readonly s: P3; readonly stretch: F };

/** Closure factor of an end at distance x (in m) from it: 0 at the end, 1 beyond the tip length. */
function tipLaw(tip: Tip, x: F): F {
  switch (tip.kind) {
    case "round": { const q = saturate(sub(1, div(x, tip.length))); return sqrt(max(sub(1, mul(q, q)), 0)); }
    case "point": return saturate(div(x, tip.length));
    case "open": return f(1);
  }
}
function endOf(tip: Tip): End {
  switch (tip.kind) {
    case "round": case "point": return "pole"; // the radius reaches 0 at the end
    case "open": return "cap";
  }
}

/** Quaternion (x, y, z, w) rotation of v, as WGSL's usual `v + 2 q.xyz x (q.xyz x v + q.w v)`. */
function qrot(q: Expr<"vec4">, v: P3): P3 {
  const u = q.xyz;
  return add(v, mul(cross(u, add(cross(u, v), mul(v, q.w))), 2));
}

/**
 * The station table's sample at m: Catmull-Rom through the station positions with mirrored phantom ends (as in
 * `through`), its derivative per unit m, the nlerped frame and the interpolated stretch. Neighbour indices are clamped
 * before they are read, so no fetch leaves the table even where `select` discards it.
 */
export function stationSample(t: TableSource, m: F): FrameSample {
  const count = t.entries, span = count - 1;
  const x = mul(clamp(m, 0, 1), span);
  const k = min(floor(x), count - 2);
  const fr = sub(x, k);
  const P = (i: F): P3 => fetch(t, i, 0).xyz;
  const p1 = P(k), p2 = P(add(k, 1));
  const p0 = select(lt(k, 0.5), sub(mul(p1, 2), p2), P(max(sub(k, 1), 0)));
  const p3 = select(ge(add(k, 2), count - 0.5), sub(mul(p2, 2), p1), P(min(add(k, 2), count - 1)));
  const a = sub(p2, p0);
  const b = sub(add(sub(mul(p0, 2), mul(p1, 5)), mul(p2, 4)), p3);
  const c = sub(add(sub(mul(p1, 3), p0), p3), mul(p2, 3));
  const f2 = mul(fr, fr), f3 = mul(f2, fr);
  const p = mul(add(add(mul(p1, 2), mul(a, fr)), add(mul(b, f2), mul(c, f3))), 0.5);
  const der = mul(add(a, add(mul(b, mul(fr, 2)), mul(c, mul(f2, 3)))), 0.5 * span);
  const q = normalize(mix(fetch(t, k, 1), fetch(t, add(k, 1), 1), fr));
  const stretch = mix(fetch(t, k, 2).z, fetch(t, add(k, 1), 2).z, fr);
  return { p, der, t: qrot(q, lift<"vec3">([1, 0, 0])), d: qrot(q, lift<"vec3">([0, 1, 0])), s: qrot(q, lift<"vec3">([0, 0, 1])), stretch };
}

/**
 * sign(x) |x|^k written as x (x^2)^((k-1)/2), floored inside the pow (WGSL's pow is undefined at 0). It must be
 * continuous through 0: the earlier sign(x) pow(max(|x|, 1e-6), k) jumped by 2 (1e-6)^k there, and on the dorsal
 * line cos(angle) is +6e-17 in f64 but -4e-8 in f32, so CPU and GPU landed on opposite sides of a 2e-4 step
 * (caught by the CPU-against-GPU surface test at squareness 3).
 */
const superPow = (x: F, k: number): F => mul(x, pow(max(mul(x, x), 1e-12), (k - 1) / 2));

/** Finite-difference step in m for the taper slope (the lean of the normal along the spine). */
const SLOPE_H = 1e-3;

type Ring = { readonly st: FrameSample; readonly pos: P3; readonly normal: P3; readonly radial: P3 };

/** The memo of a body's time slices, shared by every slice of it. */
export type Slices = Map<Expr<"f32">, { readonly body: Body; readonly index: number }>;

/** A swept body. Build with `sweep`; `strand` (src/geom/strand.ts) is the library form with defaults and a look. */
export class Body implements Surface, Chart, Placeable<Body> {
  /** The per-frame table of transported stations. */
  readonly table: StationTable;
  readonly topology: Topology;
  readonly skeleton: Skeleton;

  constructor(
    readonly name: string,
    /** The spine in the local coordinates of `chart` (and of time). */
    readonly spine: (m: F) => P3,
    readonly section: Section,
    readonly transport: Transport,
    /** Where the spine lives: `world`, or a host's chart (`place(body, on(host, ...))`). */
    readonly chart: Chart,
    /** The time this body is taken at (`time` itself for the live body). */
    readonly slice: F,
    protected readonly slices: Slices,
    tableName: string,
  ) {
    this.table = new StationTable(tableName, this);
    this.topology = { u: [endOf(section.tips[0]), endOf(section.tips[1])], v: "periodic" };
    this.skeleton = {
      joints: transport.stations,
      joint: (k) => {
        const r = fetch(this.table, k, 2);
        return { p: fetch(this.table, k, 0).xyz, r: max(r.x, r.y) };
      },
    };
  }

  /** Expressions of this body read at its slice time. */
  private now<T extends readonly Expr[]>(es: T): T {
    return (this.slice === time ? es : substitute(es, time, this.slice)) as unknown as T;
  }

  /** Tip closure at m: both ends multiplied. */
  closure(m: F): F {
    return mul(tipLaw(this.section.tips[0], m), tipLaw(this.section.tips[1], sub(1, m)));
  }

  /** Half-width before stretch, with the tips applied. */
  halfWidth(m: F): F {
    return mul(this.section.girth(m), this.closure(m));
  }

  /**
   * The skin at (m, v): a superellipse ring in the frame's side/dorsal plane, radii thinned by 1 / sqrt(stretch). The
   * normal is analytic (the in-plane superellipse gradient, minus the taper's lean along the tangent, as in
   * `strand_kit::ring_point`), so shading never depends on tessellation, and it turns to face along the spine at a
   * closing tip.
   */
  private ring(m: F, v: F): Ring {
    const st = stationSample(this.table, m);
    const aspect = this.section.aspect;
    const keep = div(1, sqrt(max(st.stretch, 0.05)));
    const a = (x: F) => mul(this.halfWidth(x), keep);
    const b = (x: F) => mul(a(x), aspect(x));
    const m0 = clamp(sub(m, SLOPE_H), 0, 1), m1 = clamp(add(m, SLOPE_H), 0, 1);
    // d(radius)/d(arc length): the radii difference over the m step, divided by the spine's speed |dP/dm|.
    const ds = mul(sub(m1, m0), max(length(st.der), 1e-6));
    const slopeA = div(sub(a(m1), a(m0)), ds), slopeB = div(sub(b(m1), b(m0)), ds);

    const n = this.section.squareness, e = 2 / n;
    const angle = mul(v, TAU);
    const c = cos(angle), s = sin(angle);
    const sp = superPow;
    const x = sp(c, e), y = sp(s, e);
    const A = a(m), B = mul(A, aspect(m));
    const pos = add(st.p, add(mul(st.s, mul(A, x)), mul(st.d, mul(B, y))));
    // The gradient of |x/a|^n + |y/b|^n, multiplied through by a (so a ring collapsed at a tip keeps a direction):
    // (gx * b / a, gy) up to scale, and b / a is the aspect.
    const gx = mul(sp(x, n - 1), aspect(m)), gy = sp(y, n - 1);
    const gl = max(sqrt(add(mul(gx, gx), mul(gy, gy))), 1e-9);
    const nx = div(gx, gl), ny = div(gy, gl);
    const lean = add(mul(slopeA, mul(x, nx)), mul(slopeB, mul(y, ny)));
    const inPlane = add(mul(st.s, nx), mul(st.d, ny));
    const normal = normalize(sub(inPlane, mul(st.t, lean)));
    return { st, pos, normal, radial: normalize(inPlane) };
  }

  point(u: F, v: F): P3 { return this.now([this.ring(u, v).pos])[0]; }
  /**
   * The skin straightened at rest: (arc length along the rest spine, the rest section's ring). A function of (u, v)
   * alone (rest arc lengths are lane r.w of the stations; the section is read at time 0), so a pattern over it rides the
   * body through any bend, and every slice of the body (`at(t)`) has the same anchor.
   */
  anchor(u: F, v: F): P3 {
    const t = this.table, span = t.entries - 1;
    const x = mul(clamp(u, 0, 1), span);
    const k = min(floor(x), span - 1);
    const s = mix(fetch(t, k, 2).w, fetch(t, add(k, 1), 2).w, sub(x, k));
    const [a, aspect] = substitute([this.halfWidth(u), this.section.aspect(u)], time, f(0)) as F[];
    const e = 2 / this.section.squareness, angle = mul(v, TAU);
    return vec3(s, mul(a, superPow(cos(angle), e)), mul(mul(a, aspect), superPow(sin(angle), e)));
  }
  normal(u: F, v: F): P3 { return this.now([this.ring(u, v).normal])[0]; }

  /**
   * Body coordinates (m, v, rho) to the point and the frame there: `x` the spine tangent (toward increasing m), `y` the
   * outward direction at v within the section (no taper lean), `z = x cross y`. At v = 0.25 it is the station frame.
   */
  frame(c: P3): Frame {
    const r = this.ring(c.x, c.y);
    const [o, x, y, z] = this.now([mix(r.st.p, r.pos, c.z), r.st.t, r.radial, cross(r.st.t, r.radial)] as const);
    return { o, x, y, z };
  }

  /** This body with `time` read as t: its own stations, posed every frame (a telegraph, a smear). */
  at(t: F): Body {
    const seen = this.slices.get(t);
    if (seen) return seen.body;
    const index = this.slices.size;
    const body = this.remake(this.chart.at(t), t, this.slices, `${this.table.base}_${index}`);
    this.slices.set(t, { body, index });
    return body;
  }

  /** The same body with its spine placed in `chart` (composed with where it already was). */
  placed(chart: Chart): Body {
    return this.remake(compose(chart, this.chart), time, new Map(), this.table.base);
  }

  /**
   * The same body placed in `chart`, posting its own table under `tableName`. `placed` keeps the table's name (the
   * placed body replaces the unplaced one); this is for carrying a whole creature somewhere while its unplaced self
   * is still read, as the host its other parts are placed on (src/geom/whirlpool.ts coils the serpent this way).
   */
  placedAs(chart: Chart, tableName: string): Body {
    return this.remake(compose(chart, this.chart), time, new Map(), tableName);
  }

  /** The same kind of body elsewhere or at another time (a subclass that carries more, such as a look, overrides it). */
  protected remake(chart: Chart, slice: F, slices: Slices, tableName: string): Body {
    return new Body(this.name, this.spine, this.section, this.transport, chart, slice, slices, tableName);
  }
}

/** A body's stations: entry k is station k (m = k / (stations - 1)); lanes p, q, r as in transport.ts. */
export class StationTable implements TableSource {
  readonly lanes = STATION_VEC4S;
  readonly entries: number;
  readonly role = "stations";
  constructor(readonly base: string, private readonly body: Body) {
    this.entries = body.transport.stations;
  }
  get name(): string { return this.base; }

  prepare(p: TablePrep): TableFill {
    const b = this.body, n = this.entries;
    const ms = Array.from({ length: n }, (_, k) => f(k / (n - 1)));
    const local = ms.map((m) => b.spine(m));
    const roots: Expr[] = [];
    // The spine reaches the world through the chart; for a body on a host this reads the host's table, which the pose
    // pass fills first (tables are filled on first read).
    // The local spine too: stretch is measured on it (DESIGN.md 7.6), not on the placed one, so a chart that bends or
    // lengthens the spine (a whirlpool winding a body into a coil) does not thin the body. Measuring the placed spine
    // against the local rest thinned a coiled serpent by 1 / sqrt(5) (found by tests/ships.whirlpool.test.ts).
    ms.forEach((m, k) => roots.push(b.chart.frame(local[k]).o, b.transport.twist(m), b.halfWidth(m), b.section.aspect(m), local[k]));
    roots.push(rotate(b.chart.frame(local[0]), b.transport.up));
    const run = p.compile(b.slice === time ? roots : substitute(roots, time, b.slice));
    // Stretch is measured against the rest pose: the local spine at time 0. It depends only on params, non-time inputs
    // and curves, which rarely change between frames, so the last one is kept (a memo keyed on all of those, so the
    // fill stays a function of its frame).
    const rest = p.compile(substitute(local, time, f(0)));
    let restKey = "", restCurves = new Float32Array(0), restValues: V3[] = [];
    const restOf = (frame: Parameters<TableFill>[0]): V3[] => {
      const env: Env = frame.env;
      const { time: _, ...inputs } = env.inputs;
      const key = JSON.stringify([inputs, env.params]);
      const curves = env.curves ?? new Float32Array(0);
      if (key !== restKey || !sameFloats(curves, restCurves)) {
        restValues = rest(frame) as V3[];
        restKey = key;
        restCurves = curves.slice();
      }
      return restValues;
    };
    const PER = 5;
    return (frame, out) => {
      const v = run(frame);
      const pos: V3[] = [], twist: number[] = [], halfWidth: number[] = [], aspect: number[] = [], here: V3[] = [];
      for (let k = 0; k < n; k++) {
        pos.push(v[k * PER] as V3); twist.push(v[k * PER + 1] as number); halfWidth.push(v[k * PER + 2] as number); aspect.push(v[k * PER + 3] as number);
        here.push(v[k * PER + 4] as V3);
      }
      poseStrand({ pos, local: here, rest: restOf(frame), twist, halfWidth, aspect, up: v[n * PER] as V3, level: b.transport.level }, out, 0);
    };
  }
}

function sameFloats(a: Float32Array, b: Float32Array): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

const IDENT = /^[A-Za-z][A-Za-z0-9_]*$/;

/** A live body's constructor arguments, checked: in world coordinates, at `time`. Subclasses (`Strand`) build from it. */
export function sweepArgs(name: string, spine: (m: F) => P3, section: Section, transport: Transport): ConstructorParameters<typeof Body> {
  if (!IDENT.test(name)) throw new Error(`sweep "${name}": names must be identifiers (they name tables and shaders)`);
  if (!(Number.isInteger(transport.stations) && transport.stations >= 4)) throw new Error(`sweep "${name}": stations must be an integer >= 4, got ${transport.stations}`);
  if (!(section.squareness >= 1.2)) throw new Error(`sweep "${name}": squareness must be >= 1.2 (the normal needs |x|^(n-1)), got ${section.squareness}`);
  return [name, spine, section, transport, world, time, new Map(), `body_${name}`];
}

/** A section swept along `spine` (in world coordinates; place it to put it elsewhere). */
export function sweep(name: string, spine: (m: F) => P3, section: Section, transport: Transport): Body {
  return new Body(...sweepArgs(name, spine, section, transport));
}

/** Rotate v about the unit axis by angle (Rodrigues), as an expression. */
export function rotateAbout(v: In<"vec3">, axis: In<"vec3">, angle: In<"f32">): P3 {
  const ve = lift<"vec3">(v), a = lift<"vec3">(axis);
  const c = cos(angle), s = sin(angle);
  return add(add(mul(ve, c), mul(cross(a, ve), s)), mul(a, mul(sub(1, c), add(add(mul(a.x, ve.x), mul(a.y, ve.y)), mul(a.z, ve.z)))));
}
