// Compose in front (DESIGN.md 7.2): `warp` evaluates a shape at a mapped coordinate. On a field or a paint this is
// folding (one motif, every copy, the same cost); on a surface or a curve it reparametrises; on points it maps the
// index. There is one function per signature because the signatures are plain functions and cannot be told apart at
// run time.

import { vec2 } from "../expr.ts";
import { SceneError } from "../errors.ts";
import { compose as composeFolds, type Folded } from "../lib/fold.ts";
import type { Curve, F, Field, P3, Paint, Posed, Poses, Surface, Warp } from "./types.ts";

/**
 * Applies a fold (a `Warp`: a map from a point to its folded point, with the cell it fell in) to something that is a
 * function of position, so that thing repeats or mirrors wherever the fold sends points onto it. One member per kind
 * of thing: `paint`, `field`, `surface`, `curve`, `points`.
 */
export const warp = {
  /** The paint at the folded coordinate: the motif repeats wherever the fold sends points onto it. */
  paint: (paint: Paint, w: Warp<"vec2">): Paint => (u, v, at) => { const x = w(vec2(u, v)).x; return paint(x.x, x.y, at); },
  /** A distance field through a fold: evaluated at the folded point and turned back into a distance by the fold. */
  field: (field: Field, w: Warp<"vec3">): Field => (p) => { const q = w(p); return q.toWorld(field(q.x)); },
  /** A surface reparametrised: the same shape, its (u, v) moved by the map (its topology is the caller's to keep). */
  surface: (s: Surface, w: Warp<"vec2">): Surface => ({
    point: (u, v) => { const x = w(vec2(u, v)).x; return s.point(x.x, x.y); },
    normal: (u, v) => { const x = w(vec2(u, v)).x; return s.normal(x.x, x.y); },
    anchor: (u, v) => { const x = w(vec2(u, v)).x; return s.anchor(x.x, x.y); },
    topology: s.topology,
    skeleton: s.skeleton,
  }),
  /** A curve reparametrised by a map of [0, 1] (to concentrate samples toward a tip, say). */
  curve: (c: Curve, w: (u: F) => F): Curve => (u) => c(w(u as F)),
  /** Poses whose index is mapped (a generator: index i becomes copy w(i)). */
  points: (p: Poses, count: number, w: (i: F) => F): Poses => ({ count, at: (i): Posed => p.at(w(i)) }),
};

/**
 * Paint from a motif drawn in one cell of a fold: the motif sees the folded point, the cell's identity (for per-copy
 * variation) and the stretch (to turn widths in the cell into widths on the surface). A list of folds is applied in
 * order (`tile([shear, fold2.repeat([1, 1])], motif)`), as `compose` from src/lib chains them.
 */
export function tile(w: Warp<"vec2"> | readonly Warp<"vec2">[], motif: (cell: Folded<"vec2">) => P3): Paint {
  // Friction S8 (2026-11-16): a chain needed lib's `compose`, which the main entry cannot export by that name (the
  // chart `compose` has it), so tile takes the chain itself.
  if (typeof w !== "function" && w.length === 0) throw new SceneError("tile", "an empty list of folds", "a fold, or a list of one or more", "tile(fold2.repeat([1, 1]), motif)");
  const chain = typeof w === "function" ? w : w.reduce((a, b) => composeFolds(a, b));
  return (u, v) => motif(chain(vec2(u, v)));
}
