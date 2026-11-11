// Many straight ropes as one stroke: standing rigging, ratlines, stays. Each rope is a pair of end points (expressions;
// they may move) and a sag; the end points are evaluated once per frame into a per-frame table (a list: each rope its
// own expressions, filled by a pose stage), and one
// `draw.curve` stroke runs through every rope in turn, so a ship's rigging is one draw call however many ropes it has.
//
// The jumps between ropes are hidden in the stroke's own grid: each rope gets `segments` cells plus three more, a
// zero-width vertex before its first point and after its last, so the cells that join one rope to the next have zero
// width at both ends (zero area: nothing is drawn there).

import { add, clamp, f, fetch, floor, lift, listOf, max, mix, mul, step, sub, vec3, vec4, type Expr, type In, type TableSource } from "../expr.ts";
import { draw } from "../draw/index.ts";
import type { Drawable } from "../compile/context.ts";
import { pixelWorld } from "../lib/wgsl.ts";
import type { F, P3 } from "../map/types.ts";

/** One rope: from a to b, sagging by `sag` (world units, at its middle, downward). */
export type Rope = { readonly a: P3; readonly b: P3; readonly sag: number };

export type LinesSpec = {
  /** Half-width of every rope, world units (never thinner than about half a pixel on screen). */
  readonly width: number;
  readonly colour: readonly [number, number, number];
  /** Cells along each rope. Default 6. */
  readonly segments?: number;
};

/** All of `ropes` as one stroke. */
export function lines(name: string, ropes: readonly Rope[], spec: LinesSpec): Drawable {
  if (ropes.length < 1) throw new Error(`lines "${name}": need at least one rope`);
  const S = spec.segments ?? 6;
  if (!(Number.isInteger(S) && S >= 1)) throw new Error(`lines "${name}": segments must be a positive integer, got ${S}`);
  const N = ropes.length, per = S + 3, cells = N * per;
  const table: TableSource = { name: `ropes_${name}`, lanes: 2, entries: N, role: "data", program: listOf(ropes.map((r) => [vec4(r.a, r.sag), vec4(r.b, 0)])) };
  // Position along the stroke in cells, the rope it falls in, and the position within that rope's cells.
  const where = (u: F): { k: F; local: F } => {
    const x = mul(u, cells);
    const k = clamp(floor(mul(add(x, 0.5), 1 / per)), 0, N - 1);
    return { k, local: sub(x, mul(k, per)) };
  };
  const curve = (uu: In<"f32">): P3 => {
    const { k, local } = where(lift<"f32">(uu));
    const r = clamp(mul(sub(local, 1), 1 / S), 0, 1);
    const a = fetch(table, k, 0), b = fetch(table, k, 1);
    const sag = mul(mul(mul(r, sub(1, r)), 4), a.w);
    return sub(mix(a.xyz, b.xyz, r), vec3(0, sag, 0));
  };
  const width = (u: F): Expr<"f32"> => {
    const { local } = where(u);
    // Full width from the first point to the last; zero on the two extra vertices round each rope.
    const on = mul(step(0.5, local), step(local, S + 1.5));
    return mul(max(f(spec.width), mul(pixelWorld(curve(u)), 0.55)), on);
  };
  const [r, g, b] = spec.colour;
  return draw.curve(name, curve, { width, paint: () => vec3(r, g, b), segments: cells });
}
