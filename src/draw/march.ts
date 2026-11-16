// march: a field's sphere trace as a hit function (docs/proposals/geometry_and_rays.md section 4.3), written in
// expressions with `iterate(..., until)`, so it runs in any shader that calls it (a reflected ray in a paint, a shadow
// ray in a light) and on the CPU unchanged.
//
// It is draw.field's march (field.ts `fragmentWgsl`) without its drawing: steps by the field's distance from the ray's
// entry into `within`, a hit when the distance falls below 0.3 of the ray's width there, the closest approach between
// two samples (where the two spheres of free space meet) kept as the near miss, the exhausted march taken as a hit (it
// is crawling along a surface), two secant steps after a hit, and the normal by four differences a quarter of the
// ray's width apart. What it does not do that draw.field does: stop at the first near miss within an ink width (an
// inner silhouette); a ray that misses reports where gap / width was least along its whole length.

import { add, and, div, dot, f, ge, gt, iterate, lt, max, min, mul, ne, neg, normalize, not, select, sqrt, sub, substitute, vec3, type Expr } from "../expr.ts";
import type { F, Field, P3 } from "../map/types.ts";
import { checkField, fieldPixel } from "./field.ts";
import type { Ray, Traced } from "./traced.ts";
import type { TraceBound } from "./trace.ts";
import { aabbOf } from "./trace.ts";
import { SceneError } from "../errors.ts";

/** A hit is a distance below this share of the ray's width (draw.field's HIT). */
const HIT = 0.3;
/** Secant steps after a hit. */
const REFINE = 2;

/** Where the ray o + t d (d unit) is inside a bound, as expressions: (enter, leave); enter > leave for a miss. */
function spanOf(b: TraceBound, o: P3, d: P3): readonly [F, F] {
  switch (b.kind) {
    case "sphere": {
      const oc = sub(o, vec3(...b.centre)), bb = dot(oc, d), h = sub(mul(bb, bb), sub(dot(oc, oc), b.radius * b.radius)), s = sqrt(max(h, 0));
      return [select(ge(h, 0), sub(neg(bb), s), f(1)), select(ge(h, 0), add(neg(bb), s), f(-1))];
    }
    // A capsule is clipped by its axis-aligned box: a looser span, never a shorter one.
    case "box": case "capsule": {
      const box = aabbOf(b);
      const inv = div(vec3(1, 1, 1), d);
      const a = mul(sub(vec3(...box.min), o), inv), c = mul(sub(vec3(...box.max), o), inv);
      const lo = min(a, c), hi = max(a, c);
      return [max(lo.x, max(lo.y, lo.z)), min(hi.x, min(hi.y, hi.z))];
    }
  }
}

/**
 * A field's sphere trace as a hit function: steps by the field's distance from the ray's t0, clipped to `within`,
 * stopping at a hit (distance below a share of the ray's width), at t1, or after `steps` evaluations. Checked like
 * draw.field: sampled inside `within` on the CPU and refused if it can overshoot, unless `safety` is given.
 *
 * Law: sphere tracing a field the CPU check finds 1-Lipschitz; the field reads `fieldPixel` as the ray's width at the
 * point. Parameters: `within` (the bound searched), `steps` (field samples at most), `safety` (a step scale in (0, 1]
 * that skips the check). Cannot: march a field that overshoots without `safety`; stop before `steps` on a grazing ray
 * (it takes the point reached); be cheap when the field is expensive (a second ray costs another march); ink an inner
 * silhouette (draw.field's own march does). Relatives: draw.field (the field drawn); over-relaxed steps (Keinert et al.
 * 2014); segment tracing for fields with a known Lipschitz bound per region; your own hit in draw.traced.
 */
export function march(field: Field, o: { readonly within: TraceBound; readonly steps: number; readonly safety?: number }): (ray: Ray) => Traced {
  if (!(Number.isInteger(o.steps) && o.steps >= 1)) throw new SceneError("march", `steps ${o.steps}`, "a whole number of field samples, at least 1", "steps: 48");
  if (o.safety !== undefined && !(o.safety > 0 && o.safety <= 1)) throw new SceneError("march", `safety ${o.safety}`, "a step scale in (0, 1]", "leave it out to have the field checked");
  const safety = o.safety ?? (checkField("march", field, o.within, "march"), 1);
  const at = (p: P3, fp: F): F => substitute([field(p)], fieldPixel, fp)[0] as F;
  return (ray: Ray): Traced => {
    const [enter, leave] = spanOf(o.within, ray.o, ray.d);
    const t0 = max(ray.t0, enter), t1 = min(ray.t1, leave);
    const point = (t: F): P3 => add(ray.o, mul(ray.d, t));
    // State: t, the last sample's t and distance, whether it was the field's own, the near miss (t, gap / width, gap),
    // how the march ended (0 running, 1 hit, 2 left the bound) and the distance at the hit.
    const init = [t0, t0, f(0), f(0), t0, f(1e30), f(1e30), select(gt(t0, t1), f(2), f(0)), f(0)] as const;
    const [t, prevT, prevDist, prevExact, nearT, , nearGap, done, hitDist] = iterate(o.steps, init, (s) => {
      const [t, prevT, prevDist, prevExact, nearT, nearPx, nearGap, done, hitDist] = s;
      const running = lt(done, 0.5);
      const fp = ray.width(t);
      const dist = at(point(t), fp);
      const hit = lt(dist, mul(fp, HIT));
      // The closest the surface can come between this sample and the last (field.ts): where their spheres meet.
      const step = sub(t, prevT);
      const x = div(add(sub(mul(dist, dist), mul(prevDist, prevDist)), mul(step, step)), max(mul(step, 2), 1e-20));
      const between = and(gt(prevExact, 0.5), and(gt(x, 0), lt(x, step)));
      const h = select(between, sqrt(max(sub(mul(dist, dist), mul(x, x)), 0)), dist);
      const th = select(between, sub(t, x), t);
      const hPx = div(h, ray.width(th));
      const nearer = and(running, and(not(hit), lt(hPx, nearPx)));
      const advance = and(running, not(hit));
      const tn = add(t, mul(dist, safety));
      const ended = select(hit, f(1), select(gt(tn, t1), f(2), f(0)));
      return [
        select(advance, tn, t), select(advance, t, prevT), select(advance, dist, prevDist), select(advance, f(1), prevExact),
        select(nearer, th, nearT), select(nearer, hPx, nearPx), select(nearer, h, nearGap),
        select(running, ended, done), select(and(running, hit), dist, hitDist),
      ] as const;
    }, (s) => ne(s[7], 0));
    // Out of samples inside the bound: the point reached is the hit (no hole; the distance is already small).
    const hit = lt(done, 1.5);
    let tr: F = t, ta: F = prevT, da: F = prevDist, db: F = hitDist, pairExact: Expr<"bool"> = gt(prevExact, 0.5);
    for (let r = 0; r < REFINE; r++) {
      const slope = div(sub(db, da), sub(tr, ta));
      const tn = select(and(pairExact, and(lt(slope, -1e-6), gt(tr, ta))), sub(tr, div(db, slope)), add(tr, mul(db, safety)));
      ta = tr; da = db; tr = tn;
      if (r + 1 < REFINE) db = at(point(tr), ray.width(tr));
      pairExact = lt(f(0), 1); // from here on both samples are the field's own
    }
    const tq = select(hit, tr, nearT);
    const p = point(tq), fp = ray.width(tq);
    // The gradient by differences on a tetrahedron a quarter of the width across (Quilez), as field.ts's NORMAL_WGSL.
    const e = max(mul(fp, 0.25), 1e-6);
    const k = (x: number, y: number, z: number) => mul(vec3(x, y, z), at(add(p, mul(vec3(x, y, z), e)), fp));
    const g = add(add(k(1, -1, -1), k(-1, -1, 1)), add(k(-1, 1, -1), k(1, 1, 1)));
    return { t: tq, gap: select(hit, f(0), max(nearGap, 1e-9)), normal: normalize(g) };
  };
}
