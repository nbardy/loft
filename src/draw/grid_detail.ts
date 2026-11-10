// Level of detail for a drawn surface (`Detail` kind "grid", src/artefact.ts): the levels its grid may be drawn at and
// the per-frame table of bounds the host chooses among them by (src/host/detail.ts).
//
// Levels halve both cell counts from the authored grid, each axis stopping at a floor: 8 cells round a closed
// direction (an octagon: at the sizes where the floor binds, under about 9 px of radius at the default target, its
// silhouette is within 0.7 px of the circle), 2 along an open one. A halving keeps every other vertex of the finer
// level where the counts divide, so a level change moves no vertex that both levels share.
//
// Bounds are measured, not declared: the surface is evaluated on the CPU at 4 x 4 points of its grid each frame (the
// copies of a `Points` set share one evaluation of their surface, carried by each copy's frame). From them: a sphere
// (the samples' box centre, radius to the farthest sample) and the world length of the longest grid line along each
// axis, which over the cell counts is the cell size the host compares with its pixel target.

import { f, vec4, type Expr, type TableSource } from "../expr.ts";
import type { DetailSpec } from "../compile/context.ts";
import type { F, P3, Points, Surface, Topology } from "../map/types.ts";

const SAMPLES = 4;
const FLOOR = { closed: 8, open: 2 } as const;

/** Cell counts at each level, finest first, from the authored counts and the topology's closed directions. */
export function gridLevels(nu: number, nv: number, topology: Topology): { readonly cells: readonly [number, number]; readonly vertices: number }[] {
  const floorU = Math.min(nu, topology.u === "periodic" ? FLOOR.closed : FLOOR.open);
  const floorV = Math.min(nv, topology.v === "periodic" ? FLOOR.closed : FLOOR.open);
  const levels: { cells: readonly [number, number]; vertices: number }[] = [];
  let u = nu, v = nv;
  for (;;) {
    levels.push({ cells: [u, v], vertices: 6 * u * v });
    const u2 = Math.max(floorU, Math.round(u / 2)), v2 = Math.max(floorV, Math.round(v / 2));
    if (u2 === u && v2 === v) return levels;
    u = u2; v = v2;
  }
}

/** One copy, or one per point (as the drawable draws it). */
export type BoundCopies = { readonly kind: "one" } | { readonly kind: "points"; readonly points: Points };

/**
 * The per-frame table of a drawn surface's bounds: two lanes per copy, (centre, radius) and (length along u, along v).
 * `at` maps the grid's [0, 1] to the surface's coordinates, as the drawable samples them.
 */
export function boundTable(name: string, surface: Surface, at: { readonly u: (s: F) => F; readonly v: (s: F) => F }, copies: BoundCopies): TableSource {
  const entries = copies.kind === "one" ? 1 : copies.points.count;
  return {
    name: `bound_${name}`, lanes: 2, entries, role: "data",
    prepare: (p) => {
      const points: P3[] = [];
      for (let j = 0; j < SAMPLES; j++) for (let i = 0; i < SAMPLES; i++) points.push(surface.point(at.u(f(i / (SAMPLES - 1))), at.v(f(j / (SAMPLES - 1)))));
      const samples = p.compile(points);
      const frames = copiesRoots(copies);
      const placed = p.compile(frames);
      return (frame, out) => {
        const s = samples(frame) as number[][];
        const local = measure(s);
        const fr = placed(frame) as number[][];
        // A copy's sphere is centred on its origin and holds the surface's own sphere wherever the copy turns it
        // (radius |centre| + radius): reading the copy's axes too doubled the cost of a set of teeth.
        const reach = Math.hypot(...local.centre) + local.radius;
        for (let c = 0; c < entries; c++) {
          const [x, y, z, k] = copies.kind === "one" ? [...local.centre, 1] : fr[c]!;
          const scale = Math.abs(k!), r = copies.kind === "one" ? local.radius : reach;
          out.set([x!, y!, z!, scale * r, scale * local.lengths[0], scale * local.lengths[1]], 8 * c);
        }
      };
    },
  };
}

/** Per copy: its origin and scale, as one vec4. */
function copiesRoots(c: BoundCopies): Expr[] {
  switch (c.kind) {
    case "one": return [];
    case "points": return Array.from({ length: c.points.count }, (_, i) => { const at = c.points.at(f(i)); return vec4(at.frame.o, at.scale); });
  }
}

/** The samples' sphere and the longest grid line along each axis (samples are row-major, u fastest). */
function measure(s: readonly (readonly number[])[]): { readonly centre: readonly [number, number, number]; readonly radius: number; readonly lengths: readonly [number, number] } {
  const lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
  for (const q of s) for (let a = 0; a < 3; a++) { lo[a] = Math.min(lo[a]!, q[a]!); hi[a] = Math.max(hi[a]!, q[a]!); }
  const centre = [(lo[0]! + hi[0]!) / 2, (lo[1]! + hi[1]!) / 2, (lo[2]! + hi[2]!) / 2] as const;
  const dist = (a: readonly number[], b: readonly number[]) => Math.hypot(a[0]! - b[0]!, a[1]! - b[1]!, a[2]! - b[2]!);
  const radius = s.reduce((r, q) => Math.max(r, dist(q, centre)), 0);
  let lu = 0, lv = 0;
  for (let j = 0; j < SAMPLES; j++) {
    let a = 0, b = 0;
    for (let i = 0; i + 1 < SAMPLES; i++) {
      a += dist(s[j * SAMPLES + i]!, s[j * SAMPLES + i + 1]!);
      b += dist(s[i * SAMPLES + j]!, s[(i + 1) * SAMPLES + j]!);
    }
    lu = Math.max(lu, a); lv = Math.max(lv, b);
  }
  return { centre, radius, lengths: [lu, lv] };
}

/** The detail a lit surface states: its levels, chosen by its bound table. */
export function gridDetail(bound: TableSource, nu: number, nv: number, topology: Topology): DetailSpec {
  return { kind: "grid", bound, levels: gridLevels(nu, nv, topology) };
}
