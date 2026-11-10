// A crest: a membrane standing on a strand's skin along a line of constant v (a dorsal fin, a frill). Its own
// coordinates are (x, f): x in [0, 1] along its span on the host, f in [0, 1] from the root to the free edge. It rides
// the host's pose, so it bends, sways and dives with the body for free.

import { abs, add, cross, lift, mix, mul, normalize, sin, smoothstep, sub, vec3, type Expr } from "../expr.ts";
import type { Curve } from "../curve.ts";
import type { FrameSample, Rgb, Strand } from "./strand.ts";

type Fn1 = (x: Expr<"f32">) => Expr<"f32">;

export type CrestSpec = {
  readonly on: Strand;
  /** Span on the host, in m. */
  readonly from: number;
  readonly to: number;
  /** Height along the span (x in [0, 1]): a keyed curve (its `.x`), a scalar function, or a number. */
  readonly height: Curve<"vec4"> | ((x: Expr<"f32">) => Expr<"f32">) | number;
  /** Line on the host it stands on. Default 0.25, the dorsal line. */
  readonly v?: number;
  /** Membrane colour (rays are drawn darker), or colour as a function of (x, f). */
  readonly paint: Rgb | ((x: Expr<"f32">, f: Expr<"f32">) => Expr<"vec3">);
  /** How far the edge sags between rays, as a share of height. Default 0.3. */
  readonly sag?: number;
  /** Supporting rays. Default 12. */
  readonly rays?: number;
  /** Lean toward increasing m, as a share of height. Default 0.4. */
  readonly rake?: number;
  /** Sideways lean of the free edge as a share of height, as a function of host m (and time). Default none. */
  readonly flutter?: Fn1;
  /** Name for the shader; default `<host>_crest`. */
  readonly name?: string;
  /** Rows along the span. Default 96. */
  readonly rows?: number;
};

export class Crest {
  readonly kind = "crest" as const;
  constructor(
    readonly name: string,
    readonly host: Strand,
    readonly from: number,
    readonly to: number,
    readonly height: Fn1,
    readonly v: number,
    readonly paint: (x: Expr<"f32">, f: Expr<"f32">) => Expr<"vec3">,
    readonly sag: number,
    readonly rays: number,
    readonly rake: number,
    readonly flutter: Fn1,
    readonly rows: number,
  ) {}

  /** Host m at span position x. */
  hostM(x: Expr<"f32">): Expr<"f32"> {
    return mix(lift<"f32">(this.from), lift<"f32">(this.to), x);
  }

  /**
   * The membrane at (x, f) over the host's station sample at m = hostM(x). The root sits slightly inside the skin
   * (rho 0.85) so no gap shows where it meets the body. The normal is the sheet's (tangent x rise); the fragment
   * stage turns it toward the viewer, since a membrane has two faces.
   */
  point(st: FrameSample, x: Expr<"f32">, f: Expr<"f32">): { pos: Expr<"vec3">; normal: Expr<"vec3"> } {
    const m = this.hostM(x);
    const skin = this.host.surface(st, m, lift<"f32">(this.v));
    const root = mix(st.p, skin.pos, 0.85);
    const between = sin(mul(x, Math.PI * this.rays));
    const h = mul(this.height(x), sub(1, mul(mul(between, between), this.sag)));
    const rise = add(skin.normal, mul(st.t, this.rake));
    const sway = mul(cross(st.t, skin.normal), mul(this.flutter(m), mul(h, mul(f, f))));
    const pos = add(add(root, mul(rise, mul(h, f))), sway);
    return { pos, normal: normalize(cross(st.t, rise)) };
  }
}

export function crest(spec: CrestSpec): Crest {
  const name = spec.name ?? `${spec.on.name}_crest`;
  if (!(spec.from >= 0 && spec.to <= 1 && spec.from < spec.to)) throw new Error(`crest "${name}": need 0 <= from < to <= 1, got ${spec.from}..${spec.to}`);
  const rays = spec.rays ?? 12;
  const h = spec.height;
  // A keyed curve returns a vec4 (its value is in .x); a scalar function returns its value directly.
  const height: Fn1 = typeof h === "number" ? () => lift<"f32">(h)
    : (x) => { const e = (h as (x: Expr<"f32">) => Expr)(x); return e.ty === "vec4" ? (e as Expr<"vec4">).x : (e as Expr<"f32">); };
  const p = spec.paint;
  const paint = typeof p === "function" ? p : membrane(p, rays);
  return new Crest(
    name, spec.on, spec.from, spec.to, height, spec.v ?? 0.25, paint, spec.sag ?? 0.3, rays, spec.rake ?? 0.4,
    spec.flutter ?? (() => lift<"f32">(0)), spec.rows ?? 96,
  );
}

/** The default membrane: darker toward the root, with darker rays (as in `strand_kit::crest`). */
function membrane(c: Rgb, rays: number): (x: Expr<"f32">, f: Expr<"f32">) => Expr<"vec3"> {
  const colour = vec3(c[0], c[1], c[2]);
  return (x, f) => {
    const sheet = mul(colour, add(0.62, mul(f, 0.38)));
    // A ray sits wherever sin(pi rays x) crosses 0: a thin line there, the same hue darkened.
    const line = mul(sub(1, smoothstep(0.06, 0.16, abs(sin(mul(x, Math.PI * rays))))), 0.8);
    return mix(sheet, mul(colour, 0.4), line);
  };
}
