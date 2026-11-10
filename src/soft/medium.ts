// A medium: a soft volume made of Gaussian puffs (blobs, fog layers) and fog banks, lit by the sun, the sky and any
// flashes inside it. It is a per-frame table plus the closed forms of `src/soft/gauss.ts`; `draw.density` draws it,
// and other soft things (glows, flares) read the same table to be seen *through* it (`veil`).
//
// Canonical form (what every constructor below reduces to, and what the table holds):
//   puff:  density d0 exp(-|K (p - c)|^2), K = inverse radii per axis (0 = unbounded: a fog layer is a puff with
//          K = (0, 1 / thickness, 0)).
//   bank:  a layer whose fog stops at a vertical plane with a soft edge: d0 exp(-|K (p - c)|^2 - (max(0, s(p)) / w)^2),
//          s the signed distance into the clear air. Along a ray s is linear, so on either side of the plane the exponent
//          is still one quadratic (written as a 4D one): two closed-form pieces per ray.
//   flash: a point light inside the medium (lightning), with a power that is an expression of time.
//
// Every number may be an expression of time (centres drift, radii breathe, densities build); the pose pass evaluates
// them, then runs the light pass on the CPU: for each puff, the optical depth of all the OTHER puffs toward the sun and
// toward the zenith from its centre, and the light reaching its centre from each flash through the medium. O(N^2) per
// frame, culled by closest approach; the per-pixel shader adds each puff's own self-shadow at the point the ray sees
// (closed form too), so a big puff is still lit on one side and dark on the other.

import { add, div, dot, exp, lift, max, mul, sub, vec3, vec4, type Expr, type In, type LibFn, type TableSource } from "../expr.ts";
import type { V3 } from "../compile/context.ts";
import type { F, P3 } from "../map/types.ts";
import { FAR, GAUSS_WGSL, gaussRay, gaussTail } from "./gauss.ts";

/** A Gaussian puff in canonical form: centre, inverse radii (0 = unbounded on that axis), peak extinction, albedo. */
export type Puff = { readonly centre: P3; readonly inv: P3; readonly density: F; readonly colour: P3 };
/** A fog layer with a soft vertical edge: fog where s <= 0, s = n . (p - through) in the xz plane, n toward clear air. */
export type Bank = {
  readonly layer: Puff;
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
export function blob(b: BlobSpec): Puff {
  return { centre: lift<"vec3">(b.centre), inv: div(vec3(1, 1, 1), lift<"vec3">(b.radii)), density: lift<"f32">(b.density), colour: lift<"vec3">(b.colour) };
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
export function layer(l: LayerSpec): Puff {
  if (!(l.thickness > 0)) throw new Error(`layer: thickness must be positive, got ${l.thickness}`);
  return { centre: vec3(0, l.height, 0), inv: lift<"vec3">([0, 1 / l.thickness, 0]), density: lift<"f32">(l.density), colour: lift<"vec3">(l.colour) };
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

/** Lanes per puff in the puff table. */
export const PUFF_LANES = 5;
/** Lanes per bank in the bank table. */
export const BANK_LANES = 4;
/** The share of each flash's power that lights the whole medium evenly (light scattered many times). */
const FLASH_AMBIENT = 0.04;

const ZENITH: V3 = [0, 1, 0];

/**
 * A soft volume. `puffs` and `banks` are its density; `flashes` light it from inside. Build it once (its tables are
 * made per sun direction on first use) and hand it to `draw.density`; glows that should be seen through it name it
 * as their veil.
 */
export class Medium {
  private readonly made = new Map<string, MediumTables>();

  constructor(readonly name: string, readonly puffs: readonly Puff[], readonly banks: readonly Bank[], readonly flashes: readonly Flash[]) {
    if (!/^[A-Za-z][A-Za-z0-9_]*$/.test(name)) throw new Error(`medium "${name}": names must be identifiers (they name tables)`);
    if (puffs.length + banks.length === 0) throw new Error(`medium "${name}": no puffs and no banks`);
  }

  /** The density at a point, as an expression (the `Density` signature): the sum the shader integrates in closed form. */
  density(p: P3): F {
    const puff = (q: Puff): F => { const x = mul(q.inv, sub(p, q.centre)); return mul(q.density, exp(mul(dot(x, x), -1))); };
    const banked = (b: Bank): F => {
      const s = sub(add(mul(p.x, b.normal[0]), mul(p.z, b.normal[1])), b.normal[0] * b.through[0] + b.normal[1] * b.through[1]);
      const e = mul(max(s, 0), 1 / b.softness);
      return mul(puff(b.layer), exp(mul(mul(e, e), -1)));
    };
    return [...this.puffs.map(puff), ...this.banks.map(banked)].reduce((a, b) => add(a, b), lift<"f32">(0));
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
 * The puff table (entries: one per puff, then one global entry) and the bank table (absent when there are no banks:
 * `{ kind: "none" }`, so the shader has no bank loop at all).
 *   puff lanes: (centre, density) (inverse radii, 0) (albedo, 0) (flash light, 0) (tau of the others toward the sun from
 *               one radius behind and in front of the centre along it, the same toward the zenith)
 *   global entry, lane 0: (flash light everywhere, 0)
 *   bank lanes: (centre, density) (inverse radii, 0) (albedo, 0) (edge normal x, z, offset, 1 / softness)
 */
export type MediumTables = {
  readonly puffs: TableSource;
  readonly banks: { readonly kind: "none" } | { readonly kind: "some"; readonly table: TableSource };
};

type CpuPuff = { c: number[]; k: number[]; d: number; col: number[] };

function makeTables(m: Medium, sun: V3): MediumTables {
  const NP = m.puffs.length, NB = m.banks.length;
  const puffRoots = m.puffs.flatMap((q) => [vec4(q.centre, q.density), vec4(q.inv, 0), vec4(q.colour, 0)]);
  const flashRoots = m.flashes.map((f) => vec4(f.at, f.power));
  const puffs: TableSource = {
    name: `${m.name}_puffs`, lanes: PUFF_LANES, entries: NP + 1, role: "data",
    prepare: (p) => {
      const run = p.compile([...puffRoots, ...flashRoots]);
      return (frame, out) => {
        const v = run(frame) as number[][];
        const ps: CpuPuff[] = Array.from({ length: NP }, (_, i) => ({ c: v[3 * i].slice(0, 3), d: v[3 * i][3], k: v[3 * i + 1].slice(0, 3), col: v[3 * i + 2].slice(0, 3) }));
        const fl = m.flashes.map((f, j) => ({ at: v[3 * NP + j].slice(0, 3), power: v[3 * NP + j][3], colour: f.colour, reach: f.reach }));
        lightPass(ps, fl, sun, out);
      };
    },
  };
  if (NB === 0) return { puffs, banks: { kind: "none" } };
  const bankRoots = m.banks.flatMap((b) => [vec4(b.layer.centre, b.layer.density), vec4(b.layer.inv, 0), vec4(b.layer.colour, 0)]);
  const banks: TableSource = {
    name: `${m.name}_banks`, lanes: BANK_LANES, entries: NB, role: "data",
    prepare: (p) => {
      const run = p.compile(bankRoots);
      return (frame, out) => {
        const v = run(frame) as number[][];
        m.banks.forEach((b, i) => {
          out.set(v[3 * i], i * 16);
          out.set(v[3 * i + 1], i * 16 + 4);
          out.set(v[3 * i + 2], i * 16 + 8);
          out.set([b.normal[0], b.normal[1], b.normal[0] * b.through[0] + b.normal[1] * b.through[1], 1 / b.softness], i * 16 + 12);
        });
      };
    },
  };
  return { puffs, banks: { kind: "some", table: banks } };
}

/** Puffs whose contribution along a ray is below this (in optical depth) are skipped by the light pass. */
const CULL_Q = 16;

/** Optical depth of the other puffs along a ray from a puff's centre: toward infinity (`len` FAR) or to a point. */
function depthThrough(ps: readonly CpuPuff[], skip: number, o: readonly number[], d: readonly number[], len: number): number {
  let tau = 0;
  const O = [0, 0, 0, 0], D = [0, 0, 0, 0];
  for (let j = 0; j < ps.length; j++) {
    const q = ps[j];
    if (j === skip || q.d <= 0) continue;
    for (let a = 0; a < 3; a++) { O[a] = q.k[a] * (o[a] - q.c[a]); D[a] = q.k[a] * d[a]; }
    // Cull by closest approach (in the puff's own units) before paying for erfc.
    const al = Math.max(D[0] * D[0] + D[1] * D[1] + D[2] * D[2], 1e-20);
    const tm = -(O[0] * D[0] + O[1] * D[1] + O[2] * D[2]) / al;
    const tc = Math.min(Math.max(tm, 0), len);
    const qx = O[0] + D[0] * tc, qy = O[1] + D[1] * tc, qz = O[2] + D[2] * tc;
    if (qx * qx + qy * qy + qz * qz > CULL_Q) continue;
    tau += q.d * (len >= FAR ? gaussTail(O, D) : gaussRay(O, D, 0, len).tau);
  }
  return tau;
}

/** Fill the puff table: the evaluated puffs, the light reaching each from the sun, the sky and the flashes. */
export function lightPass(ps: readonly CpuPuff[], flashes: readonly { at: number[]; power: number; colour: readonly number[]; reach: number }[], sun: V3, out: Float32Array): void {
  const ambient = [0, 0, 0];
  for (const f of flashes) for (let a = 0; a < 3; a++) ambient[a] += FLASH_AMBIENT * Math.max(f.power, 0) * f.colour[a];
  ps.forEach((q, i) => {
    // The others' depth toward a light, one radius (along the light) behind and in front of the centre: the shader
    // interpolates between them by where in the puff the eye sees it, so a surface puff is lit on its sunward side
    // even when its centre is shadowed by its neighbours.
    const toward = (u: V3): [number, number] => {
      const ku = Math.hypot(q.k[0] * u[0], q.k[1] * u[1], q.k[2] * u[2]);
      const r = ku > 1e-9 ? 1 / ku : 0;
      const at = (sgn: number) => depthThrough(ps, i, [q.c[0] + sgn * r * u[0], q.c[1] + sgn * r * u[1], q.c[2] + sgn * r * u[2]], u, FAR);
      return [at(-1), at(1)];
    };
    const [sunBack, sunFront] = toward(sun);
    const [upBack, upFront] = toward(ZENITH);
    const light = [0, 0, 0];
    for (const f of flashes) {
      if (!(f.power > 1e-4)) continue;
      const v = [f.at[0] - q.c[0], f.at[1] - q.c[1], f.at[2] - q.c[2]];
      const dist = Math.hypot(v[0], v[1], v[2]);
      const u = dist > 1e-6 ? v.map((x) => x / dist) : [0, 1, 0];
      // The path to the flash crosses this puff's own half too: count every puff.
      const tau = depthThrough(ps, -1, q.c, u, dist);
      const fall = (f.reach * f.reach) / (f.reach * f.reach + dist * dist);
      for (let a = 0; a < 3; a++) light[a] += f.power * f.colour[a] * fall * Math.exp(-tau);
    }
    out.set([q.c[0], q.c[1], q.c[2], q.d, q.k[0], q.k[1], q.k[2], 0, q.col[0], q.col[1], q.col[2], 0, light[0], light[1], light[2], 0, sunBack, sunFront, upBack, upFront], i * 20);
  });
  out.set([ambient[0], ambient[1], ambient[2], 0], ps.length * 20);
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

/**
 * `fn <fn>(o, d, t0, t1) -> f32`: the medium's whole optical depth along a ray segment (every puff and bank, no
 * lighting). What a glow inside or behind the medium is attenuated by. Reads the medium's tables (`acc` gives their
 * accessors), so it goes into the module of a draw that calls it, with `libs` before it.
 */
export function tauFunction(m: Medium, fn: string, acc: { readonly puffs: string; readonly banks: { readonly kind: "none" } | { readonly kind: "some"; readonly wgsl: string } }): { readonly code: string; readonly libs: readonly LibFn[] } {
  const NP = m.puffs.length;
  const banks = acc.banks.kind === "some" ? /* wgsl */ `
  for (var i = 0u; i < ${m.banks.length}u; i++) {
    let a0 = ${acc.banks.wgsl}(i * 4u);
    let a1 = ${acc.banks.wgsl}(i * 4u + 1u);
    let e = ${acc.banks.wgsl}(i * 4u + 3u);
    let O = a1.xyz * (o - a0.xyz);
    let D = a1.xyz * d;
    let s0 = e.x * o.x + e.y * o.z - e.z;
    let s1 = e.x * d.x + e.y * d.z;
    let cut = loft_bank_split(s0, s1);
    tau += a0.w * loft_gauss_ray(vec4<f32>(O, 0.0), vec4<f32>(D, 0.0), max(t0, cut.x), min(t1, cut.y)).x;
    tau += a0.w * loft_gauss_ray(vec4<f32>(O, e.w * s0), vec4<f32>(D, e.w * s1), max(t0, cut.z), min(t1, cut.w)).x;
  }` : "";
  const code = /* wgsl */ `
fn ${fn}(o: vec3<f32>, d: vec3<f32>, t0: f32, t1: f32) -> f32 {
  var tau = 0.0;
  for (var i = 0u; i < ${NP}u; i++) {
    let a0 = ${acc.puffs}(i * 5u);
    let a1 = ${acc.puffs}(i * 5u + 1u);
    let O = a1.xyz * (o - a0.xyz);
    let D = a1.xyz * d;
    let al = max(dot(D, D), 1e-20);
    let tm = clamp(-dot(O, D) / al, t0, t1);
    let pc = O + D * tm;
    if (a0.w <= 0.0 || dot(pc, pc) > ${CULL_Q}.0) { continue; }
    tau += a0.w * loft_gauss_ray(vec4<f32>(O, 0.0), vec4<f32>(D, 0.0), t0, t1).x;
  }${banks}
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

export type { Expr };
