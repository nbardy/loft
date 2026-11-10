// A crest: a sheet standing on a curve drawn in a body's coordinates (a dorsal fin, a frill). The root runs along the
// line of constant v on the host's skin, slightly inside it (rho 0.85, so no gap shows where it meets the body); the
// free edge rises along the section's outward direction, raked along the spine. Its coordinates are (x, f): x in
// [0, 1] along its span on the host, f in [0, 1] from the root to the free edge. It rides the host's chart, so it
// bends, sways and dives with the body for free.
//
// It rises along the section's outward direction, not the skin normal: near a closing tip the normal turns to face
// along the spine, and a fin standing on it became a spike shooting out of the tail.

import { abs, add, lift, mix, mul, sin, smoothstep, sub, vec3, type Expr } from "../expr.ts";
import type { Curve as KeyCurve } from "../curve.ts";
import { drawSurface, spacing, type SurfaceDrawable } from "../draw/surface.ts";
import { sheet } from "../map/surface.ts";
import type { Chart, Curve, F, Paint } from "../map/types.ts";
import type { Rgb } from "./strand.ts";

type Fn1 = (x: F) => F;

export type CrestSpec = {
  /** The body it stands on (any chart in body coordinates (m, v, rho)). */
  readonly on: Chart;
  /** Span on the host, in m. */
  readonly from: number;
  readonly to: number;
  /** Height along the span (x in [0, 1]): a keyed curve (its `.x`), a scalar function, or a number. */
  readonly height: KeyCurve<"vec4"> | ((x: F) => F) | number;
  /** Line on the host it stands on. Default 0.25, the dorsal line. */
  readonly v?: number;
  /** Membrane colour (rays are drawn darker), or colour as a function of (x, f). */
  readonly paint: Rgb | Paint;
  /** How far the edge sags between rays, as a share of height. Default 0.3. */
  readonly sag?: number;
  /** Supporting rays. Default 12. */
  readonly rays?: number;
  /** Lean toward increasing m, as a share of height. Default 0.4. */
  readonly rake?: number;
  /** Sideways lean of the free edge as a share of height, as a function of host m (and time). Default none. */
  readonly flutter?: Fn1;
  readonly name: string;
  /** Rows along the span. Default 96. */
  readonly rows?: number;
};

/** Cells from the root to the free edge. */
const COLS = 3;

export function crest(spec: CrestSpec): SurfaceDrawable {
  const name = spec.name;
  if (!(spec.from >= 0 && spec.to <= 1 && spec.from < spec.to)) throw new Error(`crest "${name}": need 0 <= from < to <= 1, got ${spec.from}..${spec.to}`);
  const rays = spec.rays ?? 12, sag = spec.sag ?? 0.3, rake = spec.rake ?? 0.4, v = spec.v ?? 0.25;
  const h = spec.height;
  // A keyed curve returns a vec4 (its value is in .x); a scalar function returns its value directly.
  const height: Fn1 = typeof h === "number" ? () => lift<"f32">(h)
    : (x) => { const e = (h as (x: F) => Expr)(x); return e.ty === "vec4" ? (e as Expr<"vec4">).x : (e as F); };
  const flutter = spec.flutter ?? (() => lift<"f32">(0));
  const hostM = (x: F): F => mix(lift<"f32">(spec.from), lift<"f32">(spec.to), x);
  const at = (x: F) => spec.on.frame(vec3(hostM(x), v, 0.85));
  const root: Curve = (x) => at(lift<"f32">(x)).o;
  const edge: Curve = (xx) => {
    const x = lift<"f32">(xx), g = at(x);
    const between = sin(mul(x, Math.PI * rays));
    const tall = mul(height(x), sub(1, mul(mul(between, between), sag)));
    const rise = add(g.y, mul(g.x, rake));
    return add(add(g.o, mul(rise, tall)), mul(g.z, mul(flutter(hostM(x)), tall)));
  };
  const p = spec.paint;
  const paint: Paint = typeof p === "function" ? p : membrane(p, rays);
  return drawSurface(name, sheet(root, edge), {
    paint, grid: { u: spacing.even(spec.rows ?? 96), v: spacing.even(COLS) }, sides: "two", outline: "border",
  });
}

/** The default membrane: darker toward the root, with darker rays (as in `strand_kit::crest`). */
function membrane(c: Rgb, rays: number): Paint {
  const colour = vec3(c[0], c[1], c[2]);
  return (x, f) => {
    const sheetColour = mul(colour, add(0.62, mul(f, 0.38)));
    // A ray sits wherever sin(pi rays x) crosses 0: a thin line there, the same hue darkened.
    const line = mul(sub(1, smoothstep(0.06, 0.16, abs(sin(mul(x, Math.PI * rays))))), 0.8);
    return mix(sheetColour, mul(colour, 0.4), line);
  };
}
