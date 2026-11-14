// A polished ball mirrors an upright ring beside it. The ball's paint sends the reflected ray to the ring's own field
// (exact, `march`) and to everything else's stand-in (`castRay`); the sky colour fills the rest. A tracer is a
// function, so the ring that draws itself also answers the ball's reflected ray.
import { add, bound, castRay, div, dot, draw, f, lt, march, mix, mul, plane, saturate, scene, select, sub, toon, vec3, type F, type P3, type Ray, type Traced } from "../src/index.ts";
import { sdSphere, sdTorus } from "../src/lib/index.ts";

const ORANGE = vec3(0.9, 0.45, 0.1), SKY = vec3(0.75, 0.85, 0.95);
const ring = (p: P3) => sdTorus(sub(p, vec3(2.4, 1.2, 0)), 0.9, 0.25);   // in the xy plane: it faces +z
const ringBound = bound.box([1.2, 0, -0.4], [3.6, 2.4, 0.4]);
const ringHit = march(ring, { within: ringBound, steps: 48 });

const mirror = (hit: { p: P3; normal: P3; d: P3 }): P3 => {
  const r = sub(hit.d, mul(hit.normal, mul(2, dot(hit.d, hit.normal))));
  const ray: Ray = { o: add(hit.p, mul(hit.normal, 0.01)), d: r, t0: f(0), t1: f(30), width: (t) => mul(t, 0.01) };
  const a = ringHit(ray), b = castRay(ray);
  const cover = (h: Traced): F => sub(1, saturate(div(h.gap, ray.width(h.t))));   // 1 on a hit, 0 well clear
  // Nearer first: whichever of the ring and the rest the ray meets sooner covers the other.
  const over = (near: P3, kNear: F, far: P3, kFar: F): P3 => mix(mix(SKY, far, kFar), near, kNear);
  return select(lt(a.t, b.t), over(ORANGE, cover(a), b.colour, cover(b)), over(b.colour, cover(b), ORANGE, cover(a)));
};

export default scene({
  ground: plane, style: toon(), sun: [-0.4, 0.8, 0.45],
  things: [
    draw.field("ring", ring, { bounds: ringBound, paint: () => ORANGE, steps: 48, outline: "silhouette", schedule: "trace" }),
    draw.traced("ball", { bound: bound.sphere([0, 1, 0], 1.06), hit: march((p) => sdSphere(sub(p, vec3(0, 1, 0)), 1), { within: bound.sphere([0, 1, 0], 1.06), steps: 32 }), paint: mirror, outline: "silhouette" }),
  ],
  cameras: { whole: { kind: "perspective", eye: [1, 1.6, 5], target: [1.2, 1, 0], fovY: 0.6 } },
});
