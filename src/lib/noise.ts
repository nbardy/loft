// Hashing and noise. CPU and GPU agree because the hash is integer arithmetic (a PCG hash on u32), never
// `fract(sin(x) * big)`, whose result depends on the precision of `sin`.
//
// Lattice coordinates are `floor(p)` cast through i32 (then reinterpreted as u32), so negative cells hash the same on
// both sides. The CPU twin uses `Math.imul` and `>>> 0` for the u32 wrap-around. Coordinates beyond +-2^31 are out of
// range (WGSL saturates the cast, JavaScript wraps); nothing in a scene is that far from the origin.
import loftPcg from "../wgsl/loft_pcg.wgsl" with { type: "text" };
import loftUnit from "../wgsl/loft_unit.wgsl" with { type: "text" };
import loftLat from "../wgsl/loft_lat.wgsl" with { type: "text" };
import loftH2 from "../wgsl/loft_h2.wgsl" with { type: "text" };
import loftH3 from "../wgsl/loft_h3.wgsl" with { type: "text" };
import loftGrad2 from "../wgsl/loft_grad2.wgsl" with { type: "text" };
import loftGrad3 from "../wgsl/loft_grad3.wgsl" with { type: "text" };
import loftCorner3 from "../wgsl/loft_corner3.wgsl" with { type: "text" };
import loftHash2 from "../wgsl/loft_hash2.wgsl" with { type: "text" };
import loftHash3 from "../wgsl/loft_hash3.wgsl" with { type: "text" };
import loftHash22 from "../wgsl/loft_hash22.wgsl" with { type: "text" };
import loftHash33 from "../wgsl/loft_hash33.wgsl" with { type: "text" };
import loftNoise2 from "../wgsl/loft_noise2.wgsl" with { type: "text" };
import loftNoise3 from "../wgsl/loft_noise3.wgsl" with { type: "text" };
import loftCellular2 from "../wgsl/loft_cellular2.wgsl" with { type: "text" };
import loftCellular3 from "../wgsl/loft_cellular3.wgsl" with { type: "text" };
import { assertWgslNumbers } from "../wgsl/assert_numbers.ts";
import { add, div, f, lift, lib, mul, TAU, type Expr, type In, type LibFn } from "../expr.ts";

// ---- integer hash, twice ------------------------------------------------------------------------------------------

const pcg = (v: number): number => {
  const s = (Math.imul(v, 747796405) + 2891336453) >>> 0;
  const w = Math.imul((s >>> ((s >>> 28) + 4)) ^ s, 277803737);
  return ((w >>> 22) ^ w) >>> 0;
};
/** The top 24 bits of a hash as a float in [0, 1): exactly representable in f32, so CPU and GPU agree bit for bit. */
const unit = (h: number): number => (h >>> 8) / 16777216;
const lat = (x: number): number => (Math.floor(x) | 0) >>> 0;
const h2u = (x: number, y: number): number => pcg((x + pcg(y)) >>> 0);
const h3u = (x: number, y: number, z: number): number => pcg((x + pcg((y + pcg(z)) >>> 0)) >>> 0);

// Helper functions the public ones call. They are never called from an expression, so their CPU twin is "gpu-only".
const helper = (name: string, ret: LibFn["ret"], wgsl: string, deps: readonly LibFn[] = []): LibFn => ({ name, ret, wgsl, js: "gpu-only", deps });

const PCG = helper("loft_pcg", "f32", loftPcg);
const UNIT = helper("loft_unit", "f32", loftUnit);
const LAT = helper("loft_lat", "f32", loftLat);
const H2 = helper("loft_h2", "f32", loftH2, [PCG]);
const H3 = helper("loft_h3", "f32", loftH3, [PCG]);
const GRAD2 = helper("loft_grad2", "vec2", loftGrad2, [UNIT]);
// A direction uniform on the sphere: z uniform in [-1, 1], azimuth uniform. (1-z)(1+z) rather than 1-z*z keeps the
// radius accurate in f32 near the poles, where 1-z*z cancels.
const GRAD3 = helper("loft_grad3", "vec3", loftGrad3, [UNIT, PCG]);
const grad2 = (h: number): [number, number] => { const a = unit(h) * TAU; return [Math.cos(a), Math.sin(a)]; };
const grad3 = (h: number): [number, number, number] => {
  const z = 1 - 2 * unit(h), a = unit(pcg(h)) * TAU, r = Math.sqrt((1 - z) * (1 + z));
  return [r * Math.cos(a), r * Math.sin(a), z];
};

// ---- hashes -------------------------------------------------------------------------------------------------------

const HASH2 = {
  name: "loft_hash2", ret: "f32", deps: [H2, LAT, UNIT],
  wgsl: loftHash2,
  js: (p: number[]) => unit(h2u(lat(p[0]), lat(p[1]))),
} satisfies LibFn;
const HASH3 = {
  name: "loft_hash3", ret: "f32", deps: [H3, LAT, UNIT],
  wgsl: loftHash3,
  js: (p: number[]) => unit(h3u(lat(p[0]), lat(p[1]), lat(p[2]))),
} satisfies LibFn;
const HASH22 = {
  name: "loft_hash22", ret: "vec2", deps: [H2, LAT, UNIT, PCG],
  wgsl: loftHash22,
  js: (p: number[]) => { const h = h2u(lat(p[0]), lat(p[1])); return [unit(h), unit(pcg(h))]; },
} satisfies LibFn;
const HASH33 = {
  name: "loft_hash33", ret: "vec3", deps: [H3, LAT, UNIT, PCG],
  wgsl: loftHash33,
  js: (p: number[]) => { const h = h3u(lat(p[0]), lat(p[1]), lat(p[2])), h1 = pcg(h); return [unit(h), unit(h1), unit(pcg(h1))]; },
} satisfies LibFn;

/** A hash of the integer cell containing p, uniform in [0, 1). Constant inside each unit cell. */
export const hash2 = (p: In<"vec2">): Expr<"f32"> => lib<"f32">(HASH2, p);
/** A hash of the integer cell containing p, uniform in [0, 1). Constant inside each unit cube. */
export const hash3 = (p: In<"vec3">): Expr<"f32"> => lib<"f32">(HASH3, p);
/** Two independent hashes of the integer cell containing p, each in [0, 1). */
export const hash22 = (p: In<"vec2">): Expr<"vec2"> => lib<"vec2">(HASH22, p);
/** Three independent hashes of the integer cell containing p, each in [0, 1). */
export const hash33 = (p: In<"vec3">): Expr<"vec3"> => lib<"vec3">(HASH33, p);

// ---- gradient noise -----------------------------------------------------------------------------------------------

// With unit gradients the 2D maximum is sqrt(2)/2 and the 3D maximum is sqrt(3)/2; these scales bring both to 1.
const SCALE2 = Math.SQRT2;
const SCALE3 = 2 / Math.sqrt(3);
assertWgslNumbers("loft_noise2.wgsl", loftNoise2, `u.y) * ${SCALE2};`);
assertWgslNumbers("loft_noise3.wgsl", loftNoise3, `u.z) * ${SCALE3};`);
const quintic = (t: number): number => t * t * t * (t * (t * 6 - 15) + 10);
const lerp = (a: number, b: number, t: number): number => a + (b - a) * t;

const NOISE2 = {
  name: "loft_noise2", ret: "f32", deps: [H2, LAT, GRAD2],
  wgsl: loftNoise2,
  js: (p: number[]) => {
    const ix = lat(p[0]), iy = lat(p[1]);
    const x = p[0] - Math.floor(p[0]), y = p[1] - Math.floor(p[1]);
    const corner = (cx: number, cy: number): number => {
      const g = grad2(h2u((ix + cx) >>> 0, (iy + cy) >>> 0));
      return g[0] * (x - cx) + g[1] * (y - cy);
    };
    const ux = quintic(x), uy = quintic(y);
    return lerp(lerp(corner(0, 0), corner(1, 0), ux), lerp(corner(0, 1), corner(1, 1), ux), uy) * SCALE2;
  },
} satisfies LibFn;

const CORNER3 = helper("loft_corner3", "f32", loftCorner3, [H3, GRAD3]);
const NOISE3 = {
  name: "loft_noise3", ret: "f32", deps: [CORNER3, LAT],
  wgsl: loftNoise3,
  js: (p: number[]) => {
    const ix = lat(p[0]), iy = lat(p[1]), iz = lat(p[2]);
    const x = p[0] - Math.floor(p[0]), y = p[1] - Math.floor(p[1]), z = p[2] - Math.floor(p[2]);
    const corner = (cx: number, cy: number, cz: number): number => {
      const g = grad3(h3u((ix + cx) >>> 0, (iy + cy) >>> 0, (iz + cz) >>> 0));
      return g[0] * (x - cx) + g[1] * (y - cy) + g[2] * (z - cz);
    };
    const ux = quintic(x), uy = quintic(y), uz = quintic(z);
    const z0 = lerp(lerp(corner(0, 0, 0), corner(1, 0, 0), ux), lerp(corner(0, 1, 0), corner(1, 1, 0), ux), uy);
    const z1 = lerp(lerp(corner(0, 0, 1), corner(1, 0, 1), ux), lerp(corner(0, 1, 1), corner(1, 1, 1), ux), uy);
    return lerp(z0, z1, uz) * SCALE3;
  },
} satisfies LibFn;

/** Smooth gradient noise (quintic interpolation): range [-1, 1], exactly 0 at integer lattice points. */
export const noise2 = (p: In<"vec2">): Expr<"f32"> => lib<"f32">(NOISE2, p);
/** Smooth gradient noise (quintic interpolation): range [-1, 1], exactly 0 at integer lattice points. */
export const noise3 = (p: In<"vec3">): Expr<"f32"> => lib<"f32">(NOISE3, p);

/**
 * Fractal sum controls. `octaves` is a compile-time count (the sum is unrolled); each octave multiplies the frequency
 * by `lacunarity` and the amplitude by `gain`.
 */
export type Fbm = { readonly octaves: number; readonly lacunarity: In<"f32">; readonly gain: In<"f32"> };

// Each octave is also shifted by a distinct dyadic offset. Gradient noise is 0 on every integer lattice point, and with
// lacunarity 2 the lattices of all octaves coincide, so without the shift the sum would be 0 along whole lines of
// the plane and share its structure between octaves. Dyadic offsets keep the f32 arithmetic exact for lacunarity 2.
const OFFSET = [37.5, 19.25, 11.125] as const;

function octavesOf(spec: Fbm): number {
  if (!Number.isInteger(spec.octaves) || spec.octaves < 1) throw new Error(`fbm: octaves must be a positive integer, got ${spec.octaves}`);
  return spec.octaves;
}

/** Fractal gradient noise, normalised by the total amplitude so the range stays within [-1, 1]. */
export function fbm2(p: In<"vec2">, spec: Fbm): Expr<"f32"> {
  const n = octavesOf(spec);
  let q: Expr<"vec2"> = lift<"vec2">(p);
  let amp: Expr<"f32"> = f(1);
  let sum: Expr<"f32"> = f(0), total: Expr<"f32"> = f(0);
  for (let i = 0; i < n; i++) {
    sum = add(sum, mul(noise2(q), amp));
    total = add(total, amp);
    amp = mul(amp, spec.gain);
    q = add(mul(q, spec.lacunarity), lift<"vec2">([OFFSET[0], OFFSET[1]]));
  }
  return div(sum, total);
}

/** Fractal gradient noise, normalised by the total amplitude so the range stays within [-1, 1]. */
export function fbm3(p: In<"vec3">, spec: Fbm): Expr<"f32"> {
  const n = octavesOf(spec);
  let q: Expr<"vec3"> = lift<"vec3">(p);
  let amp: Expr<"f32"> = f(1);
  let sum: Expr<"f32"> = f(0), total: Expr<"f32"> = f(0);
  for (let i = 0; i < n; i++) {
    sum = add(sum, mul(noise3(q), amp));
    total = add(total, amp);
    amp = mul(amp, spec.gain);
    q = add(mul(q, spec.lacunarity), lift<"vec3">([OFFSET[0], OFFSET[1], OFFSET[2]]));
  }
  return div(sum, total);
}

// ---- cellular noise -----------------------------------------------------------------------------------------------

// One jittered feature point per unit cell, found by searching the 3x3 (or 3x3x3) neighbourhood. That search can miss the
// true second-nearest point when the jitter is extreme; both backends search identically, so they still agree.
const CELLULAR2 = {
  name: "loft_cellular2", ret: "vec4", deps: [H2, LAT, UNIT, PCG],
  wgsl: loftCellular2,
  js: (p: number[]) => {
    const ix = lat(p[0]), iy = lat(p[1]);
    const x = p[0] - Math.floor(p[0]), y = p[1] - Math.floor(p[1]);
    let d1 = 8, d2 = 8, nx = 0, ny = 0;
    for (let j = -1; j <= 1; j++) for (let i = -1; i <= 1; i++) {
      const h = h2u((ix + i) >>> 0, (iy + j) >>> 0);
      const sx = i + unit(h), sy = j + unit(pcg(h));
      const rx = sx - x, ry = sy - y;
      const d = rx * rx + ry * ry;
      if (d < d1) { d2 = d1; d1 = d; nx = Math.floor(p[0]) + sx; ny = Math.floor(p[1]) + sy; } else if (d < d2) d2 = d;
    }
    return [Math.sqrt(d1), Math.sqrt(d2), nx, ny];
  },
} satisfies LibFn;

const CELLULAR3 = {
  name: "loft_cellular3", ret: "vec2", deps: [H3, LAT, UNIT, PCG],
  wgsl: loftCellular3,
  js: (p: number[]) => {
    const ix = lat(p[0]), iy = lat(p[1]), iz = lat(p[2]);
    const x = p[0] - Math.floor(p[0]), y = p[1] - Math.floor(p[1]), z = p[2] - Math.floor(p[2]);
    let d1 = 8, d2 = 8;
    for (let k = -1; k <= 1; k++) for (let j = -1; j <= 1; j++) for (let i = -1; i <= 1; i++) {
      const h = h3u((ix + i) >>> 0, (iy + j) >>> 0, (iz + k) >>> 0), h1 = pcg(h);
      const rx = i + unit(h) - x, ry = j + unit(h1) - y, rz = k + unit(pcg(h1)) - z;
      const d = rx * rx + ry * ry + rz * rz;
      if (d < d1) { d2 = d1; d1 = d; } else if (d < d2) d2 = d;
    }
    return [Math.sqrt(d1), Math.sqrt(d2)];
  },
} satisfies LibFn;

/**
 * Distance to the nearest (x) and second-nearest (y) jittered feature point, one per unit cell, both in [0, ~1.5];
 * and the nearest point itself (zw, in p's coordinates), the same bits for every p in its cell: `hash22(c.zw)` is one
 * random pair per cell, to colour each its own way. (zw added for friction S9, 2026-11-16; `.x` and `.y` are as before.)
 */
export const cellular2 = (p: In<"vec2">): Expr<"vec4"> => lib<"vec4">(CELLULAR2, p);
/** Distance to the nearest (x) and second-nearest (y) jittered feature point, one per unit cube: both in [0, ~1.7]. */
export const cellular3 = (p: In<"vec3">): Expr<"vec2"> => lib<"vec2">(CELLULAR3, p);
