// A medium: a soft volume made of Gaussian blobs, fog layers and fog banks, lit by the sun, the sky and any flashes
// inside it. It is a few per-frame tables plus the closed forms of `src/soft/gauss.ts`; `draw.density` draws it, and
// other soft things (glows, flares) read the same tables to be seen *through* it (`veil`).
//
// The kinds (each reduces to the canonical Gaussian density d0 exp(-|K (p - c)|^2), K the inverse radii):
//   blob:  bounded on every axis. Blobs shadow each other and are grouped into clusters for culling.
//   layer: a Gaussian in height only (K = (0, 1 / thickness, 0)): ground fog, haze.
//   bank:  a layer whose fog stops at a vertical plane with a soft edge: d0 exp(-|K (p - c)|^2 - (max(0, s(p)) / w)^2),
//          s the signed distance into the clear air. Along a ray s is linear, so on either side of the plane the exponent
//          is still one quadratic (written as a 4D one): two closed-form pieces per ray.
//   flash: a point light inside the medium (lightning), with a power that is an expression of time.
//
// Every number may be an expression of time (centres drift, radii breathe, densities build). The pose pass evaluates
// them and then, on the CPU:
//   - sorts the blobs along a Morton curve of their centres and groups every CLUSTER consecutive ones under a bounding
//     sphere, so a ray tests a sphere before the blobs in it (per pixel on the GPU, and in the light pass);
//   - runs the light pass: for each blob, the optical depth of all the OTHER blobs toward the sun and toward the
//     zenith, from one radius behind and one in front of its centre (the shader interpolates between them by where in
//     the blob the eye sees it), and the light reaching its centre from each flash through the medium.
// The per-pixel shader adds each blob's own self-shadow at the point the ray sees, in closed form.

import { div, dot, exp, lift, max, mul, sub, add, vec3, vec4, type Expr, type In, type LibFn, type TableSource } from "../expr.ts";
import type { V3 } from "../compile/context.ts";
import type { F, P3 } from "../map/types.ts";
import { FAR, GAUSS_WGSL, gaussDepth, gaussTail } from "./gauss.ts";

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
/** A light inside the medium: where, how bright (an expression of time; 0 is off), its colour and reach. */
export type Flash = { readonly at: P3; readonly power: F; readonly colour: readonly [number, number, number]; readonly reach: number };

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

/** A Gaussian blob. */
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

/** A horizontal fog layer: a Gaussian in height, unbounded across (ground fog, haze). */
export function layer(l: LayerSpec): Layer {
  if (!(l.thickness > 0)) throw new Error(`layer: thickness must be positive, got ${l.thickness}`);
  return { kind: "layer", height: lift<"f32">(l.height), inv: 1 / l.thickness, density: lift<"f32">(l.density), colour: lift<"vec3">(l.colour) };
}

export type BankSpec = LayerSpec & {
  /** The edge: a vertical plane through `through` (x, z), with clear air toward `clear` (an xz direction). */
  readonly edge: { readonly through: readonly [number, number]; readonly clear: readonly [number, number]; readonly softness: number };
};

/** A fog bank: a layer that ends at a soft vertical wall. */
export function bank(b: BankSpec): Bank {
  const l = Math.hypot(b.edge.clear[0], b.edge.clear[1]);
  if (!(l > 0)) throw new Error("bank: edge.clear must be a nonzero xz direction");
  if (!(b.edge.softness > 0)) throw new Error(`bank: edge.softness must be positive, got ${b.edge.softness}`);
  return { layer: layer(b), normal: [b.edge.clear[0] / l, b.edge.clear[1] / l], through: b.edge.through, softness: b.edge.softness };
}

/** A flash inside the medium. */
export function flash(f: { readonly at: In<"vec3">; readonly power: In<"f32">; readonly colour: readonly [number, number, number]; readonly reach: number }): Flash {
  if (!(f.reach > 0)) throw new Error(`flash: reach must be positive, got ${f.reach}`);
  return { at: lift<"vec3">(f.at), power: lift<"f32">(f.power), colour: f.colour, reach: f.reach };
}

/** Lanes per blob in the blob table. */
export const BLOB_LANES = 5;
/** Blobs per cluster (one bounding sphere). */
export const CLUSTER = 8;
/** The share of each flash's power that lights the whole medium evenly (light scattered many times). */
const FLASH_AMBIENT = 0.04;
/** Beyond this many reaches a flash lights nothing (its 1 / (1 + d^2) falloff is 6% there, faded smoothly to 0). */
const FLASH_CUT = 4;
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
  /** entries: one per blob (sorted per frame), then one global entry (flash light everywhere). */
  readonly blobs: TableSource;
  /** One bounding sphere per CLUSTER blobs: (centre, radius). */
  readonly clusters: TableSource;
  readonly layers: Optional;
  readonly banks: Optional;
};
/** A table a medium may not need: absent means the medium has none of that kind. */
export type Optional = { readonly kind: "none" } | { readonly kind: "some"; readonly table: TableSource; readonly count: number };

/**
 * A soft volume. `blobs`, `layers` and `banks` are its density; `flashes` light it from inside. Build it once (its
 * tables are made per sun direction on first use) and hand it to `draw.density`; glows that should be seen through it
 * name it as their veil.
 */
export class Medium {
  private readonly made = new Map<string, MediumTables>();
  readonly blobs: readonly Blob[];
  readonly layers: readonly Layer[];

  constructor(readonly name: string, puffs: readonly Puff[], readonly banks: readonly Bank[], readonly flashes: readonly Flash[]) {
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

  /** The tables for a sun direction (made once per direction). */
  tables(sun: V3): MediumTables {
    const key = sun.join(",");
    let t = this.made.get(key);
    if (!t) this.made.set(key, (t = makeTables(this, sun)));
    return t;
  }
}

/** Build a medium. */
export function medium(name: string, parts: { readonly puffs?: readonly Puff[]; readonly banks?: readonly Bank[]; readonly flashes?: readonly Flash[] }): Medium {
  return new Medium(name, parts.puffs ?? [], parts.banks ?? [], parts.flashes ?? []);
}

/**
 * Blobs evaluated for one frame, in flat arrays (the light pass reads them O(N^2) times): centre, inverse radii,
 * density, albedo, seed (the blob's index as authored, so per-blob detail stays with it when the order changes).
 */
type Frame = { n: number; c: Float64Array; k: Float64Array; d: Float64Array; col: Float64Array; seed: Float64Array; cc: Float64Array; cr: Float64Array };

function makeTables(m: Medium, sun: V3): MediumTables {
  const N = m.blobs.length, NC = Math.ceil(N / CLUSTER);
  const blobRoots = m.blobs.flatMap((q) => [vec4(q.centre, q.density), vec4(q.inv, 0), vec4(q.colour, 0)]);
  const flashRoots = m.flashes.map((f) => vec4(f.at, f.power));
  const blobs: TableSource = {
    name: `${m.name}_blobs`, lanes: BLOB_LANES, entries: N + 1, role: "data",
    prepare: (p) => {
      const run = p.compile([...blobRoots, ...flashRoots]);
      return (frame, out) => {
        const v = run(frame) as number[][];
        const fl = m.flashes.map((f, j) => ({ at: v[3 * N + j].slice(0, 3), power: v[3 * N + j][3], colour: f.colour, reach: f.reach }));
        lightPass(sortAndCluster(v, N), fl, sun, out);
      };
    },
  };
  // The spheres are recomputed from the (sorted) blob table, which this fill reads: the same sort, no shared state.
  const clusters: TableSource = {
    name: `${m.name}_clusters`, lanes: 1, entries: NC, role: "data",
    prepare: () => (frame, out) => {
      const b = frame.table(blobs);
      const sph = spheres(N, (j, a) => b[j * 20 + a], (j, a) => b[j * 20 + 4 + a]);
      out.set(sph);
    },
  };
  const layers: Optional = m.layers.length === 0 ? { kind: "none" } : (() => {
    const roots = m.layers.flatMap((l) => [vec4(0, l.height, 0, l.density), vec4(l.colour, l.inv)]);
    const table: TableSource = {
      name: `${m.name}_layers`, lanes: 2, entries: m.layers.length, role: "data",
      prepare: (p) => { const run = p.compile(roots); return (frame, out) => run(frame).forEach((x, i) => out.set(x as number[], i * 4)); },
    };
    return { kind: "some", table, count: m.layers.length };
  })();
  const banks: Optional = m.banks.length === 0 ? { kind: "none" } : (() => {
    const roots = m.banks.flatMap((b) => [vec4(0, b.layer.height, 0, b.layer.density), vec4(b.layer.colour, b.layer.inv)]);
    const table: TableSource = {
      name: `${m.name}_banks`, lanes: 3, entries: m.banks.length, role: "data",
      prepare: (p) => {
        const run = p.compile(roots);
        return (frame, out) => {
          const v = run(frame) as number[][];
          m.banks.forEach((b, i) => {
            out.set(v[2 * i], i * 12);
            out.set(v[2 * i + 1], i * 12 + 4);
            out.set([b.normal[0], b.normal[1], b.normal[0] * b.through[0] + b.normal[1] * b.through[1], 1 / b.softness], i * 12 + 8);
          });
        };
      },
    };
    return { kind: "some", table, count: m.banks.length };
  })();
  return { blobs, clusters, layers, banks };
}

/** Interleave the low 10 bits of x, y, z. */
function morton(x: number, y: number, z: number): number {
  const spread = (v: number): number => {
    v = (v | (v << 16)) & 0x030000ff;
    v = (v | (v << 8)) & 0x0300f00f;
    v = (v | (v << 4)) & 0x030c30c3;
    return (v | (v << 2)) & 0x09249249;
  };
  return spread(x) | (spread(y) << 1) | (spread(z) << 2);
}

/** The evaluated blobs, sorted along a Morton curve of their centres, with a bounding sphere per CLUSTER of them. */
function sortAndCluster(v: readonly number[][], N: number): Frame {
  const lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
  for (let i = 0; i < N; i++) for (let a = 0; a < 3; a++) { lo[a] = Math.min(lo[a], v[3 * i][a]); hi[a] = Math.max(hi[a], v[3 * i][a]); }
  const q = (i: number, a: number) => Math.min(1023, Math.max(0, Math.floor(((v[3 * i][a] - lo[a]) / Math.max(hi[a] - lo[a], 1e-9)) * 1023)));
  const order = Array.from({ length: N }, (_, i) => ({ i, key: morton(q(i, 0), q(i, 1), q(i, 2)) })).sort((a, b) => a.key - b.key || a.i - b.i);
  const NC = Math.ceil(N / CLUSTER);
  const fr: Frame = { n: N, c: new Float64Array(3 * N), k: new Float64Array(3 * N), d: new Float64Array(N), col: new Float64Array(3 * N), seed: new Float64Array(N), cc: new Float64Array(3 * NC), cr: new Float64Array(NC) };
  order.forEach(({ i }, j) => {
    for (let a = 0; a < 3; a++) { fr.c[3 * j + a] = v[3 * i][a]; fr.k[3 * j + a] = v[3 * i + 1][a]; fr.col[3 * j + a] = v[3 * i + 2][a]; }
    fr.d[j] = v[3 * i][3];
    fr.seed[j] = i;
  });
  const sph = spheres(N, (j, a) => fr.c[3 * j + a], (j, a) => fr.k[3 * j + a]);
  for (let c = 0; c < NC; c++) { fr.cc.set(sph.subarray(4 * c, 4 * c + 3), 3 * c); fr.cr[c] = sph[4 * c + 3]; }
  return fr;
}

/** A bounding sphere (centre, radius) per CLUSTER consecutive blobs, covering each blob out to EXTENT radii. */
function spheres(N: number, centre: (j: number, a: number) => number, inv: (j: number, a: number) => number): Float32Array {
  const NC = Math.ceil(N / CLUSTER);
  const out = new Float32Array(4 * NC);
  for (let c = 0; c < NC; c++) {
    const a0 = c * CLUSTER, a1 = Math.min(N, a0 + CLUSTER);
    const m = [0, 0, 0];
    for (let j = a0; j < a1; j++) for (let a = 0; a < 3; a++) m[a] += centre(j, a) / (a1 - a0);
    let r = 0;
    for (let j = a0; j < a1; j++) {
      const reach = CLUSTER_EXTENT / Math.min(inv(j, 0), inv(j, 1), inv(j, 2));
      r = Math.max(r, Math.hypot(centre(j, 0) - m[0], centre(j, 1) - m[1], centre(j, 2) - m[2]) + reach);
    }
    // Rounded up a little: the f32 table and the shader's arithmetic must not cut a blob's edge.
    out.set([m[0], m[1], m[2], r * 1.001 + 1e-3], 4 * c);
  }
  return out;
}

/** Puffs whose closest approach to a ray is beyond this (in the exponent) are skipped by the light pass. */
const CULL_Q = 16;

const O4 = [0, 0, 0, 0], D4 = [0, 0, 0, 0];
/** Optical depth of the other blobs along a ray from a point: toward infinity (`len` FAR) or to a point `len` away. */
function depthThrough(fr: Frame, skip: number, o: readonly number[], d: readonly number[], len: number): number {
  let tau = 0;
  const NC = fr.cr.length;
  for (let c = 0; c < NC; c++) {
    // The cluster's sphere against the ray segment.
    const ox = o[0] - fr.cc[3 * c], oy = o[1] - fr.cc[3 * c + 1], oz = o[2] - fr.cc[3 * c + 2];
    const b = ox * d[0] + oy * d[1] + oz * d[2];
    const tc = Math.min(Math.max(-b, 0), len);
    const px = ox + d[0] * tc, py = oy + d[1] * tc, pz = oz + d[2] * tc;
    if (px * px + py * py + pz * pz > fr.cr[c] * fr.cr[c]) continue;
    const j1 = Math.min(fr.n, (c + 1) * CLUSTER);
    for (let j = c * CLUSTER; j < j1; j++) {
      if (j === skip || fr.d[j] <= 0) continue;
      for (let a = 0; a < 3; a++) { O4[a] = fr.k[3 * j + a] * (o[a] - fr.c[3 * j + a]); D4[a] = fr.k[3 * j + a] * d[a]; }
      // Cull by closest approach (in the blob's own units) before paying for erfc.
      const al = Math.max(D4[0] * D4[0] + D4[1] * D4[1] + D4[2] * D4[2], 1e-20);
      const tm = Math.min(Math.max(-(O4[0] * D4[0] + O4[1] * D4[1] + O4[2] * D4[2]) / al, 0), len);
      const qx = O4[0] + D4[0] * tm, qy = O4[1] + D4[1] * tm, qz = O4[2] + D4[2] * tm;
      if (qx * qx + qy * qy + qz * qz > CULL_Q) continue;
      tau += fr.d[j] * (len >= FAR ? gaussTail(O4, D4) : gaussDepth(O4, D4, 0, len));
    }
  }
  return tau;
}

/** Fill the blob table: the evaluated blobs (sorted), the light reaching each from the sun, the sky and the flashes. */
function lightPass(fr: Frame, flashes: readonly { at: number[]; power: number; colour: readonly number[]; reach: number }[], sun: V3, out: Float32Array): void {
  const ambient = [0, 0, 0];
  for (const f of flashes) for (let a = 0; a < 3; a++) ambient[a] += FLASH_AMBIENT * Math.max(f.power, 0) * f.colour[a];
  const c = [0, 0, 0], at = [0, 0, 0];
  for (let i = 0; i < fr.n; i++) {
    for (let a = 0; a < 3; a++) c[a] = fr.c[3 * i + a];
    // The others' depth toward a light, one radius (along the light) behind and in front of the centre.
    const toward = (u: V3, sgn: number): number => {
      const ku = Math.hypot(fr.k[3 * i] * u[0], fr.k[3 * i + 1] * u[1], fr.k[3 * i + 2] * u[2]);
      const r = ku > 1e-9 ? 1 / ku : 0;
      for (let a = 0; a < 3; a++) at[a] = c[a] + sgn * r * u[a];
      return depthThrough(fr, i, at, u, FAR);
    };
    const sunBack = toward(sun, -1), sunFront = toward(sun, 1), upBack = toward(ZENITH, -1), upFront = toward(ZENITH, 1);
    const light = [0, 0, 0];
    for (const f of flashes) {
      if (!(f.power > 1e-4)) continue;
      const v = [f.at[0] - c[0], f.at[1] - c[1], f.at[2] - c[2]];
      const dist = Math.hypot(v[0], v[1], v[2]);
      if (dist > FLASH_CUT * f.reach) continue;
      const u = dist > 1e-6 ? v.map((x) => x / dist) : [0, 1, 0];
      // The path to the flash crosses this blob's own half too: count every blob.
      const tau = depthThrough(fr, -1, c, u, dist);
      // Through the same octaves of extinction the shader uses for sunlight (multiple scattering): a flash inside a
      // thick cloud lights it from within instead of only the few blobs in direct sight of the channel.
      const through = (Math.exp(-tau) + 0.5 * Math.exp(-0.3 * tau) + 0.25 * Math.exp(-0.09 * tau)) / 1.75;
      const x = dist / (FLASH_CUT * f.reach);
      const fall = ((f.reach * f.reach) / (f.reach * f.reach + dist * dist)) * (1 - x * x) * (1 - x * x);
      for (let a = 0; a < 3; a++) light[a] += f.power * f.colour[a] * fall * through;
    }
    out.set([c[0], c[1], c[2], fr.d[i], fr.k[3 * i], fr.k[3 * i + 1], fr.k[3 * i + 2], fr.seed[i], fr.col[3 * i], fr.col[3 * i + 1], fr.col[3 * i + 2], 0, light[0], light[1], light[2], 0, sunBack, sunFront, upBack, upFront], i * 20);
  }
  out.set([ambient[0], ambient[1], ambient[2], 0], fr.n * 20);
}

// ---- WGSL ---------------------------------------------------------------------------------------------------------

/** The closed forms as a library entry, so every module that integrates a medium carries them once. */
export const GAUSS_LIB: LibFn = { name: "loft_gauss_ray", ret: "vec3", wgsl: GAUSS_WGSL, js: "gpu-only" };

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
export function accessMedium(m: Medium, sun: V3, table: (t: TableSource) => { readonly wgsl: string }): MediumAccess {
  const t = m.tables(sun);
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

/** The union of boxes. */
export function unionBox(boxes: readonly Box[]): Box {
  if (boxes.length === 0) throw new Error("unionBox: no boxes");
  const min = [0, 1, 2].map((a) => Math.min(...boxes.map((b) => b.min[a]))) as [number, number, number];
  const max = [0, 1, 2].map((a) => Math.max(...boxes.map((b) => b.max[a]))) as [number, number, number];
  return { min, max };
}
