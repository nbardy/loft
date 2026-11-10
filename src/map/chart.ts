// Charts and placement (DESIGN.md 7.2, "compose behind"). A chart maps local coordinates to a point and a frame.
// Placement acts on what a thing is built from (a body's spine, an instance's frame), not on every vertex of its skin:
// mapping every skin vertex through a chart that bends or twists shears a body into slivers.

import { add, lift, mul, type In } from "../expr.ts";
import type { Chart, F, Frame, P3, Placeable, Placed, Points } from "./types.ts";

const X = lift<"vec3">([1, 0, 0]), Y = lift<"vec3">([0, 1, 0]), Z = lift<"vec3">([0, 0, 1]);

/** A direction given in frame g's axes, in world terms. */
export const rotate = (g: Frame, d: P3): P3 => add(add(mul(g.x, d.x), mul(g.y, d.y)), mul(g.z, d.z));

/** A frame given in a chart's local coordinates (origin and axes), carried to the chart's world. */
export function carry(chart: Chart, f: Frame): Frame {
  const g = chart.frame(f.o);
  return { o: g.o, x: rotate(g, f.x), y: rotate(g, f.y), z: rotate(g, f.z) };
}

/** The identity: local coordinates are world coordinates. */
export const world: Chart = { frame: (c) => ({ o: c, x: X, y: Y, z: Z }), at: () => world };

/**
 * The frame of `host` at the fixed local point `c`, as a rigid chart: local (x, y, z) goes to
 * `o + x X + y Y + z Z` in that frame. This is how a part rides a body (`place(horn, on(head, [0.14, v, 1]))`).
 */
export function on(host: Chart, c: In<"vec3">): Chart {
  const ce = lift<"vec3">(c);
  const g = host.frame(ce);
  return { frame: (l) => ({ o: add(g.o, rotate(g, l)), x: g.x, y: g.y, z: g.z }), at: (t) => on(host.at(t), ce) };
}

/** `inner`'s coordinates, then `outer`'s: frames of inner carried through outer. */
export function compose(outer: Chart, inner: Chart): Chart {
  return { frame: (c) => carry(outer, inner.frame(c)), at: (t) => compose(outer.at(t), inner.at(t)) };
}

/** Put a shape in a coordinate system. */
export function place<T>(shape: Placeable<T>, chart: Chart): T {
  return shape.placed(chart);
}

export type PointSpec = { readonly at: In<"vec3">; readonly scale: In<"f32"> };

/** Points with local frames at `at(i)` (axes of the coordinate system they are placed in) and scales. */
class PointSet implements Points, Placeable<PointSet> {
  constructor(readonly count: number, private readonly chart: Chart, private readonly spec: (i: F) => PointSpec) {}
  at(i: F): Placed {
    const s = this.spec(i);
    return { frame: carry(this.chart, { o: lift<"vec3">(s.at), x: X, y: Y, z: Z }), scale: lift<"f32">(s.scale) };
  }
  placed(chart: Chart): PointSet { return new PointSet(this.count, compose(chart, this.chart), this.spec); }
}

/**
 * `count` points at local coordinates `at(i)` with scale `scale(i)`. Placed on a body (`place(points(...), body)`),
 * the coordinates are body coordinates (m, v, rho) and each frame is the body's frame there.
 */
export function points(count: number, spec: (i: F) => PointSpec): PointSet {
  if (!(Number.isInteger(count) && count >= 1)) throw new Error(`points: count must be a positive integer, got ${count}`);
  return new PointSet(count, world, spec);
}
