// A medium: a soft volume made of Gaussian blobs, fog layers and fog banks, lit by the sun, the sky and the scene's
// lights (`scene({ lights })`, src/light.ts). It is a few per-frame tables plus the closed forms of `src/soft/gauss.ts`; `draw.density` draws it, and
// other soft things (glows, flares, specks) read the same tables to be seen through it (`veil`).
//
// The kinds (each reduces to the canonical Gaussian density d0 exp(-|K (p - c)|^2), K the inverse radii):
//   blob:  bounded on every axis. Blobs shadow each other and are grouped into clusters for culling.
//   layer: a Gaussian in height only (K = (0, 1 / thickness, 0)): ground fog, haze.
//   bank:  a layer whose fog stops at a vertical plane with a soft edge: d0 exp(-|K (p - c)|^2 - (max(0, s(p)) / w)^2),
//          s the signed distance into the clear air. Along a ray s is linear, so on either side of the plane the exponent
//          is still one quadratic (written as a 4D one): two closed-form pieces per ray.
//
// Every number may be an expression of time (centres drift, radii breathe, densities build). The pose pass groups
// every CLUSTER consecutive blobs under a sphere and computes the light reaching each centre from the scene's lamps.
// It also finds the depth of the other blobs toward the sun and zenith from one radius either side of each centre.
// Every table is a GPU map. Media of more than two clusters are sorted along a Morton curve first, for tighter culling spheres.
// The per-pixel shader adds each blob's own self-shadow at the point the ray sees, in closed form.

import { add, and, div, dot, eq, exp, fetch, floor, iterate, length, lib, lift, listOf, lt, mapOf, max, min, mul, ne, or, select, sub, vec3, vec4, type Expr, type In, type LibFn, type TableSource } from "../expr.ts";
import type { V3 } from "../compile/context.ts";
import type { F, P3 } from "../map/types.ts";
import { FAR, GAUSS_WGSL, gaussDepth, gaussTail } from "./gauss.ts";
import type { Light } from "../light.ts";

/** A bounded Gaussian: centre, inverse radii, peak extinction, albedo. */
export type Blob = { readonly kind: "blob"; readonly centre: P3; readonly inv: P3; readonly density: F; readonly colour: P3 };
/** A Gaussian in height: the level of its densest layer, its inverse half-thickness, peak extinction, albedo. */
export type Layer = { readonly kind: "layer"; readonly height: F; readonly inv: number; readonly density: F; readonly colour: P3 };
export type Puff = Blob | Layer;
/** A fog layer with a soft vertical edge: fog where s <= 0, s = n . (p - through) in the xz plane, n toward clear air. */
export type Bank = {
  readonly layer: Layer;
  readonly normal: readonly [number, number];
  readonly through: readonly [number, number];
  readonly softness: number;
};

/** The axis-aligned box a volume is drawn in (and integrated over). */
export type Box = { readonly min: readonly [number, number, number]; readonly max: readonly [number, number, number] };

export type BlobSpec = {
  readonly centre: In<"vec3">;
  /** 1/e radii per axis, world units (the density is e^-1 of its peak at one radius, e^-9 at three). */
  readonly radii: In<"vec3">;
  /** Peak extinction per world unit. */
  readonly density: In<"f32">;
  /** Albedo (display-referred rgb). */
  readonly colour: In<"vec3">;
};

/**
 * One soft puff of a volume: a Gaussian density about a centre, the brick every cloud, plume and puff of smoke in a
 * `medium` is built from.
 *
 * The shape and its law: density(p) = density exp(-|K (p - centre)|^2), K = (1 / rx, 1 / ry, 1 / rz) the inverse radii,
 * so it is e^-1 of its peak one radius out and e^-9 at three. Its optical depth along any ray is closed form (an erf),
 * so a ray through it is integrated exactly, not marched.
 *
 * Parameters: `centre`, `radii`, `density` and `colour` (the albedo) all take expressions, so a blob drifts, breathes,
 * builds and fades.
 *
 * What it cannot do, because of the law:
 * - turn: the radii lie along the world axes, so an ellipsoid tilted off them (a leaning plume) is not one blob.
 * - have an edge: it fades smoothly for ever (cut at 3.6 radii for culling); a crisp silhouette comes only from many
 *   small dense blobs together, or from the drawable's noise detail.
 * - be anything but convex and single-peaked: a hollow, a ring or a long thin streak is many blobs.
 * - shade itself in full: the light it gets from the sun through the other blobs is computed at two points
 *   each frame (one radius either side of its centre), so shading within one large blob is approximate.
 *
 * Going past it: a list of blobs is the shape, so `billows` (boiling and drift for blobs you place) and `cloud` (a
 * shape of revolution filled with tiers of them) are the next steps up; `medium` takes any list. Relatives, other
 * Gaussians whose ray integral is still closed form:
 * - a Gaussian in one coordinate only, unbounded in the others: `layer` (fog in height).
 * - the same, cut off past a plane by a second Gaussian factor: `bank`.
 * - a Gaussian tube along a segment (exp of the squared distance to a line, times a Gaussian along it): a streak of
 *   smoke or a contrail, whose ray integral is a two-dimensional Gaussian and so still closed form; not built here.
 */
export function blob(b: BlobSpec): Blob {
  return { kind: "blob", centre: lift<"vec3">(b.centre), inv: div(vec3(1, 1, 1), lift<"vec3">(b.radii)), density: lift<"f32">(b.density), colour: lift<"vec3">(b.colour) };
}

export type LayerSpec = {
  /** Height of the layer's densest level. */
  readonly height: In<"f32">;
  /** 1/e half-thickness, world units. */
  readonly thickness: number;
  readonly density: In<"f32">;
  readonly colour: In<"vec3">;
};

/**
 * A horizontal fog layer: a density that depends on height alone, unbounded across. Ground fog, haze, a sea of cloud
 * seen from above.
 *
 * The shape and its law: density(p) = density exp(-((p.y - height) / thickness)^2), the same at every x and z; its
 * optical depth along a ray is closed form, so the layer is drawn exactly, from any height, looking up or down.
 *
 * Parameters: `height`, `density` and `colour` take expressions (a fog that rises, thickens, clears); `thickness` is a
 * fixed number.
 *
 * What it cannot do, because of the law:
 * - stop anywhere across: it is the same at every x and z out to the medium's bounds; for an edge use `bank`.
 * - lie on a slope or follow the ground: it is level, a Gaussian in world height.
 * - have structure inside it (wisps, holes): only the drawable's noise detail breaks it up.
 *
 * Relatives:
 * - a layer that stops at a soft vertical wall: `bank`.
 * - a layer whose height is a function of x and z (fog lying in a valley): the exponent is then not quadratic along a
 *   ray, so no longer closed form; approximate with many blobs.
 */
export function layer(l: LayerSpec): Layer {
  if (!(l.thickness > 0)) throw new Error(`layer: thickness must be positive, got ${l.thickness}`);
  return { kind: "layer", height: lift<"f32">(l.height), inv: 1 / l.thickness, density: lift<"f32">(l.density), colour: lift<"vec3">(l.colour) };
}

export type BankSpec = LayerSpec & {
  /** The edge: a vertical plane through `through` (x, z), with clear air toward `clear` (an xz direction). */
  readonly edge: { readonly through: readonly [number, number]; readonly clear: readonly [number, number]; readonly softness: number };
};

/**
 * A fog bank: a horizontal `layer` that ends at a soft vertical wall, clear air on one side.
 *
 * The shape and its law: the layer's density times exp(-(max(0, s) / softness)^2), s = clear . (p - through) the
 * signed distance (in xz) into the clear air. On each side of the wall the exponent is quadratic along a ray, so the
 * bank is integrated exactly in two pieces.
 *
 * Parameters: the layer's (`height`, `density`, `colour` take expressions; `thickness` fixed) and the edge, `through`,
 * `clear` and `softness`, fixed numbers.
 *
 * What it cannot do, because of the law:
 * - move or bend its edge: the wall is a fixed straight vertical plane.
 * - have more than one edge: a strip or a patch of fog is several banks (each a separate wall), or blobs.
 * - lean: the wall is vertical, a fog that rolls over a cliff edge is not one.
 *
 * Relatives: two banks with opposite `clear` directions overlapping make a strip of fog; a bank whose s is the distance
 * to a circle (radial instead of planar) would be a round patch, but s is then not linear along a ray and the closed
 * form is lost.
 */
export function bank(b: BankSpec): Bank {
  const l = Math.hypot(b.edge.clear[0], b.edge.clear[1]);
  if (!(l > 0)) throw new Error("bank: edge.clear must be a nonzero xz direction");
  if (!(b.edge.softness > 0)) throw new Error(`bank: edge.softness must be positive, got ${b.edge.softness}`);
  return { layer: layer(b), normal: [b.edge.clear[0] / l, b.edge.clear[1] / l], through: b.edge.through, softness: b.edge.softness };
}

/** Lanes per blob in the blob table. */
export const BLOB_LANES = 5;
/** Blobs per cluster (one bounding sphere). */
export const CLUSTER = 8;
/**
 * The share of a light that lights the whole medium evenly (light scattered many times): of the most of it that
 * reaches any blob's centre. (It was 4% of a flash's power at its source; a light function has no source to read, and
 * the most that reaches a blob is that where a blob sits near the light.)
 */
const LIGHT_AMBIENT = 0.04;
/** Extent of a blob for culling, in radii: e^-13 of its peak (q = 13). */
export const EXTENT = 3.6;
/**
 * A cluster's sphere covers its blobs out to this many radii: e^-9 of the peak, where a blob's whole-line depth is below
 * the 1e-3 the shader skips it at for every density a cloud uses (peak line depth < 8).
 */
const CLUSTER_EXTENT = 3.0;

const ZENITH: V3 = [0, 1, 0];

/** The tables of a medium (for one sun direction). A kind with no members has no table and no loop in the shaders. */
export type MediumTables = {
  /** entries: one per blob (sorted per frame), then one global entry (the lights' light everywhere). */
  readonly blobs: TableSource;
  /** One bounding sphere per CLUSTER blobs: (centre, radius). */
  readonly clusters: TableSource;
  readonly layers: Optional;
  readonly banks: Optional;
};
/** A table a medium may not need: absent means the medium has none of that kind. */
export type Optional = { readonly kind: "none" } | { readonly kind: "some"; readonly table: TableSource; readonly count: number };

/**
 * A soft volume. `blobs`, `layers` and `banks` are its density; the scene's lights light it from inside. Build it once
 * (its tables are made per sun direction and list of lights on first use) and hand it to `draw.density`; glows that
 * should be seen through it name it as their veil.
 */
export class Medium {
  private readonly made = new Map<string, MediumTables>();
  private readonly lightIds = new WeakMap<readonly Light[], number>();
  readonly blobs: readonly Blob[];
  readonly layers: readonly Layer[];

  constructor(readonly name: string, puffs: readonly Puff[], readonly banks: readonly Bank[]) {
    if (!/^[A-Za-z][A-Za-z0-9_]*$/.test(name)) throw new Error(`medium "${name}": names must be identifiers (they name tables)`);
    this.blobs = puffs.filter((p): p is Blob => p.kind === "blob");
    this.layers = puffs.filter((p): p is Layer => p.kind === "layer");
    // The shaders loop over blob clusters unconditionally; a medium of fog alone still needs one (empty) blob.
    if (this.blobs.length === 0) throw new Error(`medium "${name}": needs at least one blob (fog alone: add a blob of density 0)`);
  }

  /** The density at a point, as an expression (the `Density` signature): the sum the shader integrates in closed form. */
  density(p: P3): F {
    const gauss = (x: Expr<"vec3">, d: F): F => mul(d, exp(mul(dot(x, x), -1)));
    const ofBlob = (b: Blob): F => gauss(mul(b.inv, sub(p, b.centre)), b.density);
    const ofLayer = (l: Layer): F => { const y = mul(sub(p.y, l.height), l.inv); return mul(l.density, exp(mul(mul(y, y), -1))); };
    const ofBank = (b: Bank): F => {
      const s = sub(add(mul(p.x, b.normal[0]), mul(p.z, b.normal[1])), b.normal[0] * b.through[0] + b.normal[1] * b.through[1]);
      const e = mul(max(s, 0), 1 / b.softness);
      return mul(ofLayer(b.layer), exp(mul(mul(e, e), -1)));
    };
    return [...this.blobs.map(ofBlob), ...this.layers.map(ofLayer), ...this.banks.map(ofBank)].reduce((a, b) => add(a, b), lift<"f32">(0));
  }

  /** The tables for a sun direction and the scene's lights (made once per pair). */
  tables(sun: V3, lights: readonly Light[]): MediumTables {
    let id = this.lightIds.get(lights);
    if (id === undefined) this.lightIds.set(lights, (id = this.made.size));
    const key = `${sun.join(",")}|${id}`;
    let t = this.made.get(key);
    if (!t) this.made.set(key, (t = makeTables(this, sun, lights)));
    return t;
  }
}

/**
 * A soft volume: blobs, fog layers and fog banks summed into one density, lit by the sun, the sky and the scene's
 * lights, drawn by `draw.density`; glows behind or inside it name it as their veil and are dimmed by it.
 *
 * The shape and its law: density(p) = sum of the puffs' and banks' Gaussians, so the optical depth along any ray is a
 * sum of closed forms (erf), integrated per pixel without marching. Lighting: each blob gets the other blobs' depth
 * toward the sun and zenith, and its light from each lamp, from a light pass each frame, all GPU maps (more than 16
 * blobs are first sorted along a Morton curve). Per pixel the depth is deposited into bins and composited front to back. `density(p)` gives the same sum as an expression.
 *
 * Parameters: `name` (an identifier: it names tables), the `puffs` (blobs and layers) and `banks`, each with the
 * expressions it takes. The list itself is fixed at build: blobs can move, not appear.
 *
 * What it cannot do, because of the law and the light pass:
 * - be empty of blobs: fog alone needs one blob (of density 0).
 * - have blobs appear, split or vanish: the count is fixed; fade one in or out by its density.
 * - be cheap in large numbers: the light pass is O(N^2) every frame, and each pixel loops over the blob
 *   clusters its ray meets; hundreds of blobs, not tens of thousands.
 * - cast shadows on bodies or the ground.
 * - be lit by a light within a blob: each blob gets one light value from each light, at its centre (a large blob is lit
 *   evenly by a light touching its edge), with the light's transmission through the medium on the way (the same
 *   extinction octaves as sunlight, so a flash glows through thick cloud); 4% of the most of each light reaching any
 *   blob lights the whole medium evenly. Each frame the light pass traces from every blob to every light.
 * - draw outside its box: `draw.density` integrates only within `bounds` (keep it 3 radii past every puff; `cloud`
 *   and `billows` return one, and `unionBox` joins several).
 *
 * Going past it: the puffs are the open slot; `cloud` and `billows` build lists of them. Relatives: any density whose
 * line integral is closed form (a Gaussian in one, two or three coordinates, cut by planes) can join the sum; a density
 * that is not (noise, a function of your own) would need a marched drawable instead.
 */
export function medium(name: string, parts: { readonly puffs?: readonly Puff[]; readonly banks?: readonly Bank[] }): Medium {
  return new Medium(name, parts.puffs ?? [], parts.banks ?? []);
}

function makeTables(m: Medium, sun: V3, lights: readonly Light[]): MediumTables {
  // Two clusters or fewer: a Morton sort buys little culling, so these media keep the order they were written in.
  const { blobs, clusters } = m.blobs.length <= 2 * CLUSTER ? smallMediumTables(m, sun, lights) : sortedMediumTables(m, sun, lights);
  const layers: Optional = m.layers.length === 0 ? { kind: "none" } : (() => {
    const table: TableSource = {
      name: `${m.name}_layers`, lanes: 2, entries: m.layers.length, role: "data",
      program: listOf(m.layers.map((l) => [vec4(0, l.height, 0, l.density), vec4(l.colour, l.inv)])),
    };
    return { kind: "some", table, count: m.layers.length };
  })();
  const banks: Optional = m.banks.length === 0 ? { kind: "none" } : (() => {
    const table: TableSource = {
      name: `${m.name}_banks`, lanes: 3, entries: m.banks.length, role: "data",
      program: listOf(m.banks.map((b) => [
        vec4(0, b.layer.height, 0, b.layer.density), vec4(b.layer.colour, b.layer.inv),
        vec4(b.normal[0], b.normal[1], b.normal[0] * b.through[0] + b.normal[1] * b.through[1], 1 / b.softness),
      ])),
    };
    return { kind: "some", table, count: m.banks.length };
  })();
  return { blobs, clusters, layers, banks };
}
/** The blobs as written: (centre, density), (inverse radii, index), (colour, 0). */
function puffsTable(m: Medium): TableSource {
  return {
    name: `${m.name}_puffs`, lanes: 3, entries: m.blobs.length, role: "data",
    program: listOf(m.blobs.map((q, i) => [vec4(q.centre, q.density), vec4(q.inv, i), vec4(q.colour, 0)])),
  };
}

/**
 * The blob table: entry r is written blob `source(r)` and the light reaching it (as `BLOB_LANES` lanes), then one
 * global entry (the lights' even share). The light pass: each light's transmission to the blob, through every blob,
 * and the other blobs' depth toward the sun and zenith from one radius either side of its centre.
 */
function blobsTable(m: Medium, base: TableSource, source: (r: F) => F, sun: V3, lights: readonly Light[]): TableSource {
  const N = m.blobs.length, zero = vec3(0, 0, 0);
  const depths = (p: P3, d: V3 | P3, reach: In<"f32">, skip: F | number): F =>
    iterate(N, [lift<"f32">(0)], ([tau], j) => {
      const a = fetch(base, j, 0), k = fetch(base, j, 1).xyz;
      const hit = lib<"f32">(LIGHT_DEPTH, mul(k, sub(p, a.xyz)), mul(k, d), reach, a.w);
      return [add(tau, select(ne(j, skip), hit, 0))];
    })[0];
  const toward = (i: F, p: P3, k: P3, d: V3, side: number): F => {
    const r = div(side, max(length(mul(k, d)), 1e-9));
    return depths(add(p, mul(d, r)), d, FAR, i);
  };
  const ambient = lights.reduce((rgb, light) => {
    const most = iterate(N, [zero], ([peak], j) => [max(peak, light(fetch(base, j, 0).xyz).amount)])[0];
    return add(rgb, mul(most, LIGHT_AMBIENT));
  }, zero);
  return {
    name: `${m.name}_blobs`, lanes: BLOB_LANES, entries: N + 1, role: "data",
    program: { kind: "map", runs: [
      { count: N, lanes: (r) => {
        const i = source(r), c = fetch(base, i, 0), pk = fetch(base, i, 1), k = pk.xyz, p = c.xyz;
        const local = lights.reduce((rgb, light) => {
          const a = light(p);
          const tau = depths(p, a.toward, a.distance ?? FAR, -1);
          const vis = div(add(add(exp(mul(tau, -1)), mul(exp(mul(tau, -0.3)), 0.5)), mul(exp(mul(tau, -0.09)), 0.25)), 1.75);
          return add(rgb, mul(a.amount, vis));
        }, zero);
        return [c, pk, fetch(base, i, 2), vec4(local, 0), vec4(toward(i, p, k, sun, -1), toward(i, p, k, sun, 1),
          toward(i, p, k, ZENITH, -1), toward(i, p, k, ZENITH, 1))];
      } },
      { count: 1, lanes: () => [vec4(ambient, 0), vec4(0, 0, 0, 0), vec4(0, 0, 0, 0), vec4(0, 0, 0, 0), vec4(0, 0, 0, 0)] },
    ] },
  };
}

/**
 * Media of more than two clusters: the blobs sorted along a Morton curve of their centres each frame, so that each
 * cluster of CLUSTER consecutive blobs is compact and its sphere culls. All maps: the box of the centres, each blob's
 * key (10 bits per axis in the box, interleaved, as two 15-bit halves so f32 holds them exactly), each blob's rank
 * (keys, ties by index), the sorted blob table, and a sphere per cluster of it.
 */
function sortedMediumTables(m: Medium, sun: V3, lights: readonly Light[]): Pick<MediumTables, "blobs" | "clusters"> {
  const N = m.blobs.length, base = puffsTable(m), centre = (j: In<"f32">): P3 => fetch(base, j, 0).xyz;
  const [lo, hi] = iterate(N, [vec3(BIG, BIG, BIG), vec3(-BIG, -BIG, -BIG)], ([l, h], j) => [min(l, centre(j)), max(h, centre(j))]) as [P3, P3];
  const box: TableSource = { name: `${m.name}_box`, lanes: 2, entries: 1, role: "data", program: listOf([[vec4(lo, 0), vec4(hi, 0)]]) };
  const keys: TableSource = { name: `${m.name}_keys`, lanes: 1, entries: N, role: "data", program: mapOf(N, (j) => {
    const l = fetch(box, 0, 0).xyz, q = min(max(floor(mul(div(sub(centre(j), l), max(sub(fetch(box, 0, 1).xyz, l), 1e-9)), 1023)), 0), 1023);
    // Bit b of a whole number x < 1024, and bits b0 .. b0 + 4 of each axis interleaved (x lowest), as the CPU's `morton`.
    const bit = (x: F, b: number): F => sub(floor(mul(x, 2 ** -b)), mul(floor(mul(x, 2 ** -(b + 1))), 2));
    const half = (b0: number): F => [0, 1, 2, 3, 4].reduce((k, d) => add(k, add(add(mul(bit(q.x, b0 + d), 2 ** (3 * d)), mul(bit(q.y, b0 + d), 2 ** (3 * d + 1))), mul(bit(q.z, b0 + d), 2 ** (3 * d + 2)))), lift<"f32">(0));
    return [vec4(half(5), half(0), 0, 0)];
  }) };
  const ranks: TableSource = { name: `${m.name}_ranks`, lanes: 1, entries: N, role: "data", program: mapOf(N, (j) => {
    const kj = fetch(keys, j, 0);
    const [n] = iterate(N, [lift<"f32">(0)], ([n], k) => {
      const kk = fetch(keys, k, 0);
      const before = or(lt(kk.x, kj.x), and(eq(kk.x, kj.x), or(lt(kk.y, kj.y), and(eq(kk.y, kj.y), lt(k, j)))));
      return [add(n, select(before, 1, 0))];
    });
    return [vec4(n!, 0, 0, 0)];
  }) };
  const source = (r: F): F => iterate(N, [lift<"f32">(0)], ([s], j) => [select(eq(fetch(ranks, j, 0).x, r), j, s)])[0]!;
  const blobs = blobsTable(m, base, source, sun, lights);
  // A sphere per CLUSTER consecutive sorted blobs (the last cluster may hold fewer): their mean centre, and the reach
  // of the farthest out to CLUSTER_EXTENT radii, rounded up a little so the f32 table and the shader never cut a blob.
  const sphere = (c: F, n: number): Expr<"vec4">[] => {
    const at = (j: F): P3 => fetch(blobs, add(mul(c, CLUSTER), j), 0).xyz, inv = (j: F): P3 => fetch(blobs, add(mul(c, CLUSTER), j), 1).xyz;
    const mean = div(iterate(n, [vec3(0, 0, 0)], ([sum], j) => [add(sum, at(j))])[0]! as P3, n);
    const [r] = iterate(n, [lift<"f32">(0)], ([r], j) => { const k = inv(j); return [max(r, add(length(sub(at(j), mean)), div(CLUSTER_EXTENT, min(k.x, min(k.y, k.z)))))]; });
    return [vec4(mean, add(mul(r!, 1.001), 1e-3))];
  };
  const full = Math.floor(N / CLUSTER), rest = N - full * CLUSTER;
  const clusters: TableSource = {
    name: `${m.name}_clusters`, lanes: 1, entries: Math.ceil(N / CLUSTER), role: "data",
    program: { kind: "map", runs: [{ count: full, lanes: (c) => sphere(c, CLUSTER) }, ...(rest === 0 ? [] : [{ count: 1, lanes: () => sphere(lift<"f32">(full), rest) }])] },
  };
  return { blobs, clusters };
}

/** Beyond any centre: the start of the box's min and max. */
const BIG = 1e30;

function smallMediumTables(m: Medium, sun: V3, lights: readonly Light[]): Pick<MediumTables, "blobs" | "clusters"> {
  const N = m.blobs.length, zero = vec3(0, 0, 0), base = puffsTable(m);
  const blobs = blobsTable(m, base, (r) => r, sun, lights);
  const entries = Math.ceil(N / CLUSTER);
  const clusters: TableSource = {
    name: `${m.name}_clusters`, lanes: 1, entries, role: "data",
    program: listOf(Array.from({ length: entries }, (_, i) => {
      const group = m.blobs.slice(i * CLUSTER, (i + 1) * CLUSTER);
      const centre = div(group.reduce((a, b) => add(a, b.centre), zero), group.length);
      const radius = group.reduce((r, b) => max(r, add(length(sub(centre, b.centre)), div(CLUSTER_EXTENT, min(b.inv.x, min(b.inv.y, b.inv.z))))), lift<"f32">(0));
      return [vec4(centre, add(mul(radius, 1.001), 1e-3))];
    })),
  };
  return { blobs, clusters };
}

/** Puffs whose closest approach to a ray is beyond this (in the exponent) are skipped by the shader's integration. */
const CULL_Q = 16;

// ---- WGSL ---------------------------------------------------------------------------------------------------------

/** The closed forms as a library entry, so every module that integrates a medium carries them once. */
export const GAUSS_LIB: LibFn = { name: "loft_gauss_ray", ret: "vec3", wgsl: GAUSS_WGSL, js: "gpu-only" };
const LIGHT_DEPTH: LibFn = {
  name: "loft_medium_light_depth", ret: "f32", deps: [GAUSS_LIB],
  js: (o: number[], d: number[], t: number, density: number) => {
    if (density <= 0) return 0;
    return density * (t >= FAR ? gaussTail(o, d) : gaussDepth([...o, 0], [...d, 0], 0, t));
  },
  wgsl: /* wgsl */ `
fn loft_medium_light_depth(o: vec3<f32>, d: vec3<f32>, t: f32, density: f32) -> f32 {
  if (density <= 0.0) { return 0.0; }
  if (t >= ${FAR}.0) { return density * loft_gauss_tail(o, d); }
  return density * loft_gauss_ray(vec4<f32>(o, 0.0), vec4<f32>(d, 0.0), 0.0, t).x;
}`,
};


/** The edge of a bank along a ray: (inside from, inside to, outside from, outside to), before clipping. */
const BANK_SPLIT_WGSL = /* wgsl */ `
fn loft_bank_split(s0: f32, s1: f32) -> vec4<f32> {
  let par = abs(s1) < 1e-7;
  let tc = -s0 / select(s1, 1.0, par);
  let up = s1 > 0.0;
  let fogAll = s0 <= 0.0;
  let inLo = select(select(tc, -1e30, up), select(1e30, -1e30, fogAll), par);
  let inHi = select(select(1e30, tc, up), select(-1e30, 1e30, fogAll), par);
  let outLo = select(select(-1e30, tc, up), select(-1e30, 1e30, fogAll), par);
  let outHi = select(select(tc, 1e30, up), select(1e30, -1e30, fogAll), par);
  return vec4<f32>(inLo, inHi, outLo, outHi);
}`;
export const BANK_LIB: LibFn = { name: "loft_bank_split", ret: "vec4", wgsl: BANK_SPLIT_WGSL, js: "gpu-only" };

/** WGSL accessors of a medium's tables in one shader module (from `ctx.table`). */
export type MediumAccess = {
  readonly blobs: string;
  readonly clusters: string;
  readonly layers: { readonly kind: "none" } | { readonly kind: "some"; readonly wgsl: string; readonly count: number };
  readonly banks: { readonly kind: "none" } | { readonly kind: "some"; readonly wgsl: string; readonly count: number };
};

/** Accessors for a medium's tables, declaring them in this compilation. */
export function accessMedium(m: Medium, sun: V3, lights: readonly Light[], table: (t: TableSource) => { readonly wgsl: string }): MediumAccess {
  const t = m.tables(sun, lights);
  const opt = (o: Optional): MediumAccess["layers"] => (o.kind === "some" ? { kind: "some", wgsl: table(o.table).wgsl, count: o.count } : { kind: "none" });
  return { blobs: table(t.blobs).wgsl, clusters: table(t.clusters).wgsl, layers: opt(t.layers), banks: opt(t.banks) };
}

/** WGSL: blob cluster c's blobs, if its sphere meets the ray segment: `for (...) { BODY }` with `i` the blob. */
export function clusterLoop(m: Medium, acc: MediumAccess, body: string): string {
  const N = m.blobs.length, NC = Math.ceil(N / CLUSTER);
  return /* wgsl */ `
  for (var c = 0u; c < ${NC}u; c++) {
    let sph = ${acc.clusters}(c);
    let oc = o - sph.xyz;
    let bq = dot(oc, d);
    let tq = clamp(-bq, t0, t1);
    let pq = oc + d * tq;
    if (dot(pq, pq) > sph.w * sph.w) { continue; }
    for (var i = c * ${CLUSTER}u; i < min(c * ${CLUSTER}u + ${CLUSTER}u, ${N}u); i++) {${body}
    }
  }`;
}

/**
 * `fn <fn>(o, d, t0, t1) -> f32`: the medium's whole optical depth along a ray segment (every blob, layer and bank,
 * no lighting). What a glow inside or behind the medium is attenuated by. Reads the medium's tables, so it goes into
 * the module of a draw that calls it, with `libs` before it.
 */
export function tauFunction(m: Medium, fn: string, acc: MediumAccess): { readonly code: string; readonly libs: readonly LibFn[] } {
  const layers = acc.layers.kind === "some" ? /* wgsl */ `
  for (var i = 0u; i < ${acc.layers.count}u; i++) {
    let a0 = ${acc.layers.wgsl}(i * 2u);
    let k = ${acc.layers.wgsl}(i * 2u + 1u).w;
    tau += a0.w * loft_gauss_ray(vec4<f32>(0.0, k * (o.y - a0.y), 0.0, 0.0), vec4<f32>(0.0, k * d.y, 0.0, 0.0), t0, t1).x;
  }` : "";
  const banks = acc.banks.kind === "some" ? /* wgsl */ `
  for (var i = 0u; i < ${acc.banks.count}u; i++) {
    let a0 = ${acc.banks.wgsl}(i * 3u);
    let k = ${acc.banks.wgsl}(i * 3u + 1u).w;
    let e = ${acc.banks.wgsl}(i * 3u + 2u);
    let O = vec3<f32>(0.0, k * (o.y - a0.y), 0.0);
    let D = vec3<f32>(0.0, k * d.y, 0.0);
    let s0 = e.x * o.x + e.y * o.z - e.z;
    let s1 = e.x * d.x + e.y * d.z;
    let cut = loft_bank_split(s0, s1);
    tau += a0.w * loft_gauss_ray(vec4<f32>(O, 0.0), vec4<f32>(D, 0.0), max(t0, cut.x), min(t1, cut.y)).x;
    tau += a0.w * loft_gauss_ray(vec4<f32>(O, e.w * s0), vec4<f32>(D, e.w * s1), max(t0, cut.z), min(t1, cut.w)).x;
  }` : "";
  const code = /* wgsl */ `
fn ${fn}(o: vec3<f32>, d: vec3<f32>, t0: f32, t1: f32) -> f32 {
  var tau = 0.0;${clusterLoop(m, acc, `
      let a0 = ${acc.blobs}(i * 5u);
      let a1 = ${acc.blobs}(i * 5u + 1u);
      let O = a1.xyz * (o - a0.xyz);
      let D = a1.xyz * d;
      let al = max(dot(D, D), 1e-20);
      let tm = clamp(-dot(O, D) / al, t0, t1);
      let pc = O + D * tm;
      if (a0.w <= 0.0 || dot(pc, pc) > ${CULL_Q}.0) { continue; }
      tau += a0.w * loft_gauss_ray(vec4<f32>(O, 0.0), vec4<f32>(D, 0.0), t0, t1).x;`)}${layers}${banks}
  return tau;
}`;
  return { code, libs: acc.banks.kind === "some" ? [GAUSS_LIB, BANK_LIB] : [GAUSS_LIB] };
}

/**
 * The smallest axis-aligned box holding all of `boxes`: the bounds of a medium made of several clouds, each with its
 * own box. It cannot take an empty list (it throws), and it holds the boxes, not what is in them: two far-apart clouds
 * give one large box, every pixel of which runs the medium's shader.
 */
export function unionBox(boxes: readonly Box[]): Box {
  if (boxes.length === 0) throw new Error("unionBox: no boxes");
  const min = [0, 1, 2].map((a) => Math.min(...boxes.map((b) => b.min[a]))) as [number, number, number];
  const max = [0, 1, 2].map((a) => Math.max(...boxes.map((b) => b.max[a]))) as [number, number, number];
  return { min, max };
}
