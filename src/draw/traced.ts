// draw.traced: any solid the author can intersect, drawn per pixel inside a bound (docs/proposals/geometry_and_rays.md
// section 4.2). The author writes only `hit`; Loft supplies the bound's proxy, the view ray, depth, coverage, ink, the
// style's light and the CPU twin.

import type { F, P3 } from "../map/types.ts";
import type { TraceBound } from "./trace.ts";

/**
 * A ray, or a thin cone: from `o` along unit `d`, searched for t in [t0, t1]. `width(t)` is how wide the ray is at t.
 * For a view ray it is the pixel's footprint; for a shadow ray, the light's apparent size times t; for a reflection,
 * how much it blurs.
 */
export type Ray = { readonly o: P3; readonly d: P3; readonly t0: F; readonly t1: F; readonly width: (t: F) => F };

/**
 * Where a ray meets a solid, or passes nearest it. `gap` is the solid's distance at t: 0 on a hit, positive on a miss,
 * at the t where gap / width(t) is least. `normal` is the solid's outward normal there.
 */
export type Traced = { readonly t: F; readonly gap: F; readonly normal: P3 };

/** Where a traced solid is searched: one static bound, or one box per copy (expressions: copies may move). */
export type TracedBound = TraceBound | { readonly kind: "copies"; readonly count: number; readonly box: (copy: F) => { readonly centre: P3; readonly half: P3 } };

/** What a traced solid's paint sees at a hit: the point, the normal, the ray's direction and the copy's index. */
export type TracedHit = { readonly p: P3; readonly normal: P3; readonly d: P3; readonly copy: F };
export type TracedPaint = (hit: TracedHit) => P3;
