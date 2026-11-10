// Surfaces written as maps, and the products that build them from curves (DESIGN.md 7.2, "multiply"). `sweep` lives in
// sweep.ts (it needs frames transported on the CPU); everything here is closed form.

import { add, clamp, cos, cross, div, length, lift, max, mix, mul, sin, sub, TAU, vec3, type In } from "../expr.ts";
import { through } from "../curve.ts";
import { rotate } from "./chart.ts";
import { NO_SKELETON, type Curve, type End, type F, type P3, type Placed, type Surface, type Topology } from "./types.ts";

/** Parameter step of the finite differences that give a map's normal. Small against [0, 1], large against f32 noise. */
const FD = 1e-3;

/**
 * The unit normal of a map, from central differences of the same expression: `d/du x d/dv`, normalised (a degenerate
 * point, where the two derivatives are parallel, gives the zero vector rather than NaN). Symbolic differentiation
 * (src/grad.ts) can replace this without changing any caller.
 */
export function mapNormal(point: (u: F, v: F) => P3): (u: F, v: F) => P3 {
  return (u, v) => {
    const du = sub(point(add(u, FD), v), point(sub(u, FD), v));
    const dv = sub(point(u, add(v, FD)), point(u, sub(v, FD)));
    const n = cross(du, dv);
    return div(n, max(length(n), 1e-12));
  };
}

/** A surface written directly as a function of (u, v); its normal comes from the map itself. */
export function surface(point: (u: F, v: F) => P3, topology: Topology): Surface {
  return { point, normal: mapNormal(point), topology, skeleton: NO_SKELETON };
}

/** A surface carried by a placement: points and normals in the placed frame, scaled (an instance of it). */
export function framed(s: Surface, at: Placed): Surface {
  return {
    point: (u, v) => add(at.frame.o, mul(rotate(at.frame, s.point(u, v)), at.scale)),
    normal: (u, v) => rotate(at.frame, s.normal(u, v)),
    topology: s.topology,
    skeleton: NO_SKELETON,
  };
}

/**
 * The ruled surface from `root` (v = 0) to `edge` (v = 1): a membrane standing on a curve. The normal is the ruling
 * crossed with nothing degenerate: `root'(u) x (edge - root)`, where root' is a central difference (so a sheet whose
 * edge comes down to its root, as a fin does at its ends, keeps the direction of its neighbours there).
 */
export function sheet(root: Curve, edge: Curve): Surface {
  const point = (u: F, v: F): P3 => mix(root(u), edge(u), v);
  return {
    point,
    normal: (u) => {
      const along = sub(root(clamp(add(u, FD), 0, 1)), root(clamp(sub(u, FD), 0, 1)));
      const n = cross(along, sub(edge(u), root(u)));
      return div(n, max(length(n), 1e-12));
    },
    topology: { u: ["free", "free"], v: "free" },
    skeleton: NO_SKELETON,
  };
}

/**
 * A surface of revolution about the y axis: `profile(u)` is (radius, height); v runs once round, counter-clockwise
 * seen from +y. Ends where the radius is zero are poles; the caller says which (`ends`).
 */
export function lathe(profile: (u: F) => In<"vec2">, ends: readonly [End, End]): Surface {
  const point = (u: F, v: F): P3 => {
    const rh = lift<"vec2">(profile(u)), a = mul(v, -TAU);
    return vec3(mul(rh.x, cos(a)), rh.y, mul(rh.x, sin(a)));
  };
  return surface(point, { u: ends, v: "periodic" });
}

/** Through a row of curves: v = 0 is the first, v = 1 the last, interpolated by a Catmull-Rom spline in between. */
export function loft(curves: readonly Curve[]): Surface {
  if (curves.length < 2) throw new Error(`loft: need at least two curves, got ${curves.length}`);
  return surface((u, v) => through(v, curves.map((c) => c(u))), { u: ["free", "free"], v: "free" });
}

/** A curve moved along `by` (a direction and a length in one vector): v = 0 is the curve, v = 1 the moved copy. */
export function extrude(curve: Curve, by: In<"vec3">): Surface {
  const d = lift<"vec3">(by);
  return surface((u, v) => add(curve(u), mul(d, v)), { u: ["free", "free"], v: "free" });
}

/**
 * A sheet made solid: a lens of half-thickness `half` round it. v goes once round the section (0 the sheet's v = 0
 * edge, 0.25 the front face at the middle, 0.5 the v = 1 edge, 0.75 the back face), so the result is periodic in v and
 * thins to the sheet's two edges; free u ends become caps.
 */
export function thicken(s: Surface, half: In<"f32">): Surface {
  const point = (u: F, v: F): P3 => {
    const w = sub(0.5, mul(cos(mul(v, TAU)), 0.5));
    return add(s.point(u, w), mul(s.normal(u, w), mul(sin(mul(v, TAU)), half)));
  };
  return surface(point, { u: thickEnds(s.topology.u), v: "periodic" });
}
const CAPPED: Readonly<Record<End, End>> = { free: "cap", pole: "pole", cap: "cap" };
function thickEnds(u: Topology["u"]): Topology["u"] {
  return u === "periodic" ? u : [CAPPED[u[0]], CAPPED[u[1]]];
}
