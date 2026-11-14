// A ball bored through along y, traced exactly by intervals (`draw.traced`): the sphere's span along the ray minus the
// bore's. No march and no distance field: the author writes only the intersection, and Loft supplies the bound, the
// view ray, depth, coverage, ink, the style's light and the CPU twin. The same `bead` answers any ray (export it and
// cast a shadow or a reflected ray at it from another thing's paint).
import { add, and, bound, div, dot, draw, ge, length, lt, max, mul, neg, normalize, plane, scene, select, sqrt, sub, toon, vec2, vec3, type Ray, type Traced } from "../src/index.ts";

const C = vec3(0, 1, 0), R = 1, BORE = 0.35;

/** Where `ray` first meets the bead, or where it passes nearest it. */
export function bead(ray: Ray): Traced {
  const oc = sub(ray.o, C);
  // Sphere: t^2 + 2 b t + c = 0.
  const b = dot(oc, ray.d), hs = sub(mul(b, b), sub(dot(oc, oc), R * R)), rs = sqrt(max(hs, 0));
  const s0 = sub(neg(b), rs), s1 = add(neg(b), rs);
  // Bore, the infinite cylinder x^2 + z^2 = BORE^2 about C: a t^2 + 2 e t + g = 0 in xz.
  const a = add(mul(ray.d.x, ray.d.x), mul(ray.d.z, ray.d.z)), e = add(mul(oc.x, ray.d.x), mul(oc.z, ray.d.z));
  const hc = sub(mul(e, e), mul(a, sub(add(mul(oc.x, oc.x), mul(oc.z, oc.z)), BORE * BORE))), rc = sqrt(max(hc, 0));
  const c0 = div(sub(neg(e), rc), max(a, 1e-9)), c1 = div(add(neg(e), rc), max(a, 1e-9));
  const inBore = and(ge(hc, 0), and(ge(s0, c0), lt(s0, c1)));      // the sphere's entry falls inside the bore
  const t = select(inBore, c1, s0);                                  // then the first surface is the bore's far wall
  const hit = and(ge(hs, 0), and(ge(t, ray.t0), and(lt(t, s1), lt(t, ray.t1)))); // ...if the wall ends inside the ball
  const p = add(ray.o, mul(ray.d, t)), q0 = sub(p, C);
  const normal = select(inBore, normalize(vec3(neg(q0.x), 0, neg(q0.z))), normalize(q0)); // the bore's faces its axis
  // A miss: the bead's distance where the ray passes nearest its centre (coverage and ink are measured from it).
  const tn = max(neg(b), ray.t0), q = sub(add(ray.o, mul(ray.d, tn)), C);
  const gap = max(sub(length(q), R), sub(BORE, length(vec2(q.x, q.z))));
  return { t: select(hit, t, tn), gap: select(hit, 0, max(gap, 1e-6)), normal };
}

export default scene({
  ground: plane, style: toon(), sun: [-0.4, 0.8, 0.45],
  things: [draw.traced("bead", { bound: bound.sphere([0, 1, 0], 1.06), // room round the solid for its ink
    hit: bead, paint: () => vec3(0.8, 0.3, 0.2), outline: "silhouette" })],
  cameras: {
    whole: { kind: "perspective", eye: [2.5, 3.5, 3.5], target: [0, 1, 0], fovY: 0.6 },
    through: { kind: "perspective", eye: [0.05, 4.2, 0.08], target: [0, 1, 0], fovY: 0.6 },
  },
});
