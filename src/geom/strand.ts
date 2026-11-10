// A strand: a spine plus a thickness, which defines body coordinates (m, v, rho). m in [0, 1] runs along the spine
// (it is the spine's parameter), v in [0, 1] runs round it (v = 0 the +side line, 0.25 the dorsal line, 0.5 the
// other side, 0.75 the belly), rho is distance out as a fraction of the thickness (the skin is rho = 1).
//
// The spec is canonicalised once (`strand`); everything after that is straight-line expression building. The surface
// is an expression of (m, v) over a *station sample* (position, frame, stretch interpolated from the pose table), so
// the same expression is emitted to WGSL for the vertex stage and to JavaScript for checks.

import {
  add, clamp, cos, cross, div, dot, length, lift, max, mix, mul, normalize, pow, saturate, sin, sqrt, sub,
  TAU, variable, vec3, type Expr, type In,
} from "../expr.ts";

export type Rgb = readonly [number, number, number];
type Fn1 = (m: Expr<"f32">) => Expr<"f32">;

export type TipSpec = "round" | "point" | "open" | { readonly round: number } | { readonly point: number };
/** A colour, or colour as a function of body coordinates. */
export type PaintSpec = Rgb | ((m: Expr<"f32">, v: Expr<"f32">) => Expr<"vec3">);

export type StrandSpec = {
  readonly name: string;
  /** The spine as a function of m (and of time, inputs, params). */
  readonly spine: (m: Expr<"f32">) => Expr<"vec3">;
  /** How many frames the pose pass computes. Default 32. */
  readonly stations?: number;
  /** Half-width along the side direction. */
  readonly girth: Fn1 | number;
  /** `aspect` = half-height over half-width; `squareness` is the superellipse exponent (2 an ellipse, higher boxier). */
  readonly section?: { readonly aspect?: Fn1 | number; readonly squareness?: number };
  /** Roll about the tangent, radians, added to the rotation-minimising frame. */
  readonly twist?: Fn1;
  /** How each end closes: one spec for both, or [at m = 0, at m = 1]. Default round at both. */
  readonly tips?: TipSpec | readonly [TipSpec, TipSpec];
  readonly paint: PaintSpec;
  /** Grid density hints for the vertex stage: rings along, segments around. */
  readonly detail?: { readonly along?: number; readonly around?: number };
  /** Direction the frame starts from at m = 0 (the dorsal side faces it). Default world up. */
  readonly up?: In<"vec3">;
  /** Per world unit: how fast accumulated roll bleeds back toward `up`. Default 0 (pure rotation-minimising). */
  readonly level?: number;
};

/** How an end closes, canonical. Lengths are fractions of m. */
export type Tip =
  | { readonly kind: "round"; readonly length: number }
  | { readonly kind: "point"; readonly length: number }
  | { readonly kind: "open" };

const DEFAULT_TIP = { round: 0.05, point: 0.12 } as const;

function canonicalTip(spec: TipSpec, name: string): Tip {
  if (spec === "round" || spec === "point") return { kind: spec, length: DEFAULT_TIP[spec] };
  if (spec === "open") return { kind: "open" };
  if (typeof spec === "object" && "round" in spec) return { kind: "round", length: positive(spec.round, `${name}: tip round length`) };
  if (typeof spec === "object" && "point" in spec) return { kind: "point", length: positive(spec.point, `${name}: tip point length`) };
  throw new Error(`${name}: unknown tip ${JSON.stringify(spec)}`);
}

function positive(x: number, what: string): number {
  if (!(x > 0) || !Number.isFinite(x)) throw new Error(`${what} must be a positive number, got ${x}`);
  return x;
}

/** Closure factor of an end at distance x (in m) from it: 0 at the end, 1 beyond the tip length. */
function tipLaw(tip: Tip, x: Expr<"f32">): Expr<"f32"> {
  switch (tip.kind) {
    case "round": { const q = saturate(sub(1, div(x, tip.length))); return sqrt(max(sub(1, mul(q, q)), 0)); }
    case "point": return saturate(div(x, tip.length));
    case "open": return lift<"f32">(1);
  }
}

const fn1 = (x: Fn1 | number): Fn1 => (typeof x === "number" ? () => lift<"f32">(x) : x);

/** Interpolated pose at some m, as expressions bound by whoever emits the surface (WGSL locals or JS values). */
export type FrameSample = {
  readonly p: Expr<"vec3">;
  /** d(position)/dm on the interpolated spine. */
  readonly der: Expr<"vec3">;
  readonly t: Expr<"vec3">;
  readonly d: Expr<"vec3">;
  readonly s: Expr<"vec3">;
  readonly stretch: Expr<"f32">;
};

/** The frame sample as free variables, named as WGSL `StationSample` fields (`st.p`, ...). */
export const FRAME_VARS: FrameSample = {
  p: variable("st_p", "vec3"), der: variable("st_der", "vec3"), t: variable("st_t", "vec3"),
  d: variable("st_d", "vec3"), s: variable("st_s", "vec3"), stretch: variable("st_stretch", "f32"),
};
export const FRAME_WGSL: Readonly<Record<string, string>> = {
  st_p: "st.p", st_der: "st.der", st_t: "st.t", st_d: "st.d", st_s: "st.s", st_stretch: "st.stretch",
};

export type SurfacePoint = { readonly pos: Expr<"vec3">; readonly normal: Expr<"vec3">; readonly radii: readonly [Expr<"f32">, Expr<"f32">] };

/** Finite-difference step in m for the taper slope (the lean of the normal along the spine). */
const SLOPE_H = 1e-3;

export class Strand {
  readonly kind = "strand" as const;
  constructor(
    readonly name: string,
    readonly spine: (m: Expr<"f32">) => Expr<"vec3">,
    readonly stations: number,
    readonly girth: Fn1,
    readonly aspect: Fn1,
    readonly squareness: number,
    readonly twist: Fn1,
    readonly tips: readonly [Tip, Tip],
    readonly paint: (m: Expr<"f32">, v: Expr<"f32">) => Expr<"vec3">,
    readonly along: number,
    readonly around: number,
    readonly up: Expr<"vec3">,
    readonly level: number,
  ) {}

  /** Tip closure at m: both ends multiplied. */
  closure(m: Expr<"f32">): Expr<"f32"> {
    return mul(tipLaw(this.tips[0], m), tipLaw(this.tips[1], sub(1, m)));
  }

  /** Half-width before stretch, with the tips applied. */
  halfWidth(m: Expr<"f32">): Expr<"f32"> {
    return mul(this.girth(m), this.closure(m));
  }

  /**
   * The surface at body coordinates (m, v) over a station sample: a superellipse ring in the frame's side/dorsal
   * plane, radii thinned by 1 / sqrt(stretch). The normal is analytic (the in-plane superellipse gradient, minus the
   * taper's lean along the tangent, as in `strand_kit::ring_point`), so shading never depends on tessellation, and it
   * turns to face along the spine at a closing tip.
   */
  surface(st: FrameSample, m: Expr<"f32">, v: Expr<"f32">): SurfacePoint {
    const keep = div(1, sqrt(max(st.stretch, 0.05)));
    const a = (x: Expr<"f32">) => mul(this.halfWidth(x), keep);
    const b = (x: Expr<"f32">) => mul(a(x), this.aspect(x));
    const m0 = clamp(sub(m, SLOPE_H), 0, 1), m1 = clamp(add(m, SLOPE_H), 0, 1);
    // d(radius)/d(arc length): the radii difference over the m step, divided by the spine's speed |dP/dm|.
    const ds = mul(sub(m1, m0), max(length(st.der), 1e-6));
    const slopeA = div(sub(a(m1), a(m0)), ds), slopeB = div(sub(b(m1), b(m0)), ds);

    const n = this.squareness, e = 2 / n;
    const angle = mul(v, TAU);
    const c = cos(angle), s = sin(angle);
    // sign(x) |x|^k written as x (x^2)^((k-1)/2), floored inside the pow (WGSL's pow is undefined at 0). It must be
    // continuous through 0: the earlier sign(x) pow(max(|x|, 1e-6), k) jumped by 2 (1e-6)^k there, and on the dorsal
    // line cos(angle) is +6e-17 in f64 but -4e-8 in f32, so CPU and GPU landed on opposite sides of a 2e-4 step
    // (caught by the CPU-against-GPU surface test at squareness 3).
    const sp = (x: Expr<"f32">, k: number) => mul(x, pow(max(mul(x, x), 1e-12), (k - 1) / 2));
    const x = sp(c, e), y = sp(s, e);
    const A = a(m), B = mul(A, this.aspect(m));
    const pos = add(st.p, add(mul(st.s, mul(A, x)), mul(st.d, mul(B, y))));
    // The gradient of |x/a|^n + |y/b|^n, multiplied through by a (so a ring collapsed at a tip keeps a direction):
    // (gx * b / a, gy) up to scale, and b / a is the aspect.
    const gx = mul(sp(x, n - 1), this.aspect(m)), gy = sp(y, n - 1);
    const gl = max(sqrt(add(mul(gx, gx), mul(gy, gy))), 1e-9);
    const nx = div(gx, gl), ny = div(gy, gl);
    const lean = add(mul(slopeA, mul(x, nx)), mul(slopeB, mul(y, ny)));
    const normal = normalize(sub(add(mul(st.s, nx), mul(st.d, ny)), mul(st.t, lean)));
    return { pos, normal, radii: [A, B] };
  }
}

/** Canonicalise a strand spec: every default is decided here, once. */
export function strand(spec: StrandSpec): Strand {
  const name = spec.name;
  if (!/^[A-Za-z][A-Za-z0-9_]*$/.test(name)) throw new Error(`strand "${name}": names must be identifiers (they name shaders)`);
  const stations = spec.stations ?? 32;
  if (!(Number.isInteger(stations) && stations >= 4)) throw new Error(`strand "${name}": stations must be an integer >= 4, got ${stations}`);
  const squareness = spec.section?.squareness ?? 2;
  if (!(squareness >= 1.2)) throw new Error(`strand "${name}": squareness must be >= 1.2 (the normal needs |x|^(n-1)), got ${squareness}`);
  const tips: readonly [TipSpec, TipSpec] = spec.tips === undefined ? ["round", "round"]
    : Array.isArray(spec.tips) ? (spec.tips as readonly [TipSpec, TipSpec]) : [spec.tips as TipSpec, spec.tips as TipSpec];
  const paint = spec.paint;
  const paintFn = typeof paint === "function" ? paint : () => vec3(...(paint as Rgb));
  return new Strand(
    name, spec.spine, stations, fn1(spec.girth), fn1(spec.section?.aspect ?? 1), squareness,
    spec.twist ?? (() => lift<"f32">(0)), [canonicalTip(tips[0], name), canonicalTip(tips[1], name)], paintFn,
    Math.round(spec.detail?.along ?? 96), Math.round(spec.detail?.around ?? 24),
    lift<"vec3">(spec.up ?? [0, 1, 0]), spec.level ?? 0,
  );
}

// ---- closed-form frames for attaching parts ---------------------------------------------------------------------

/**
 * A frame at m built in closed form from the spine expression: tangent by a finite difference, dorsal = `up` made
 * perpendicular. It is the pose pass's frame exactly at m = 0 (where the rotation-minimising frame is seeded), and a
 * close approximation on a gently curving spine; parts attached with it (heads, horns, eyes) are expressions, so they
 * move with the body without reading the pose table. Proper attachment (`attach`, DESIGN 2.3) is M2.
 */
export function frameOf(body: Strand, m: number): { p: Expr<"vec3">; der: Expr<"vec3">; t: Expr<"vec3">; d: Expr<"vec3">; s: Expr<"vec3"> } {
  const h = 2e-3;
  const m0 = Math.max(0, m - h), m1 = Math.min(1, m + h);
  const p = body.spine(lift<"f32">(m));
  const der = div(sub(body.spine(lift<"f32">(m1)), body.spine(lift<"f32">(m0))), m1 - m0);
  const t = normalize(der);
  const d = normalize(sub(body.up, mul(t, dot(body.up, t))));
  return { p, der, t, d, s: cross(t, d) };
}

/** A point on a strand's skin at (m, v, rho) using `frameOf`, with its outward normal. At rest (stretch 1). */
export function skinPoint(body: Strand, m: number, v: number, rho = 1): { pos: Expr<"vec3">; normal: Expr<"vec3">; frame: ReturnType<typeof frameOf> } {
  const fr = frameOf(body, m);
  const sp = body.surface({ ...fr, stretch: lift<"f32">(1) }, lift<"f32">(m), lift<"f32">(v));
  return { pos: mix(fr.p, sp.pos, rho), normal: sp.normal, frame: fr };
}

/** Rotate v about the unit axis by angle (Rodrigues), as an expression. */
export function rotate(v: In<"vec3">, axis: In<"vec3">, angle: In<"f32">): Expr<"vec3"> {
  const ve = lift<"vec3">(v), a = lift<"vec3">(axis);
  const c = cos(angle), s = sin(angle);
  return add(add(mul(ve, c), mul(cross(a, ve), s)), mul(a, mul(dot(a, ve), sub(1, c))));
}
