// The physical sanity numbers of `pbr` (agent_notes/2026-11-18_pbr_style/NOTE.md), measured on the parts and the style as
// built: the white furnace (twin and GPU), the specular lobe's albedo against the analytic split sum, the highlight's
// width over a roughness sweep, and the colour of a metal's highlight.
//
//   bun tools/pbr_sanity.ts

import { compileJs, lift, mul, variable, vec3, type Expr, type NumTy } from "../src/index.ts";
import { ggx, pbr, smithGgx } from "../src/styles.ts";
import { shadeVariables } from "../src/style.ts";
import type { Lighting } from "../src/light.ts";
import { runGpu, rng, toF32, type Sample } from "../tests/lib.harness.ts";

const f = (x: number) => x.toFixed(3);
const lighting = (dir: [number, number, number], amount: [number, number, number]): Lighting => ({ sun: { kind: "fixed", dir }, amount: { kind: "given", amount }, lights: [], sampling: { kind: "point" }, shadows: new Map() });
const said = (m: Record<string, Expr<NumTy>>) => ({ channels: new Map(Object.entries(m)), scatter: undefined });
const unit = (r: () => number): number[] => { for (;;) { const v = [r() * 2 - 1, r() * 2 - 1, r() * 2 - 1]; const l = Math.hypot(...v); if (l > 0.2 && l < 1) return v.map((x) => x / l); } };

// ---- 1. white furnace ------------------------------------------------------------------------------------------
{
  const style = pbr({ sky: () => vec3(0.5, 0.5, 0.5), tone: (x) => x });
  // The sun at 1 from straight below: the sky keeps its amount and the upper normals sampled get no direct light.
  const ctx = shadeVariables(lighting([0, -1, 0], [1, 1, 1]), style, said({ rough: variable("c_rough", "f32") }));
  const shade = style.shade({ ...ctx, base: lift<"vec3">([1, 1, 1]) });
  const run = compileJs([shade]);
  for (const rough of [0.2, 0.5, 1]) {
    const r = rng(3);
    const samples: Sample[] = [];
    // A sphere's upper half seen from a random eye: normal n, view toward the eye within 85 degrees of it.
    while (samples.length < 2000) {
      const n = unit(r); if (n[1]! < 0) continue;
      const v = unit(r); if (n[0]! * v[0]! + n[1]! * v[1]! + n[2]! * v[2]! < 0.09) continue;
      samples.push(toF32({ c_normal: n, c_view: v, c_world: [0, 1, 0], c_depth: -1, c_rough: rough }));
    }
    const twin = samples.map((s) => (run({ vars: s, inputs: {}, params: {} })[0] as number[])[1]!);
    const gpu = (await runGpu([shade], samples)).map((o) => o[0]![1]!);
    const bytes = gpu.map((x) => Math.round(Math.min(1, Math.max(0, x)) * 255));
    const stat = (xs: number[]) => ({ mean: xs.reduce((a, b) => a + b, 0) / xs.length, lo: Math.min(...xs), hi: Math.max(...xs) });
    const t = stat(twin), g = stat(gpu), b = stat(bytes);
    console.log(`furnace rough ${rough}: twin mean ${t.mean.toFixed(5)} [${t.lo.toFixed(5)}, ${t.hi.toFixed(5)}]  gpu mean ${g.mean.toFixed(5)} [${g.lo.toFixed(5)}, ${g.hi.toFixed(5)}]  8-bit mean ${b.mean.toFixed(2)}/255 [${b.lo}, ${b.hi}]`);
  }
}

// ---- 2. the lobe's albedo against the analytic split sum ----------------------------------------------------------
{
  const nh = variable("nh", "f32"), nl = variable("nl", "f32"), nv = variable("nv", "f32"), rough = variable("rough", "f32");
  const lobe = compileJs([mul(ggx(nh, rough), smithGgx(nl, nv, rough))]);
  /** E(mu) = integral over l of D Vis n.l (F = 1), in half-vector space: d omega_l = 4 (v.h) d omega_h. */
  const albedo = (mu: number, r: number): number => {
    const v = [Math.sqrt(1 - mu * mu), 0, mu];
    const T = 3000, P = 256;
    let sum = 0;
    for (let i = 0; i < T; i++) {
      const th = ((i + 0.5) / T) * (Math.PI / 2), st = Math.sin(th), ct = Math.cos(th);
      for (let j = 0; j < P; j++) {
        const ph = ((j + 0.5) / P) * 2 * Math.PI;
        const h = [st * Math.cos(ph), st * Math.sin(ph), ct];
        const vh = v[0]! * h[0]! + v[2]! * h[2]!;
        if (vh <= 0) continue;
        const lz = 2 * vh * h[2]! - v[2]!;
        if (lz <= 0) continue;
        const val = lobe({ vars: { nh: ct, nl: lz, nv: mu, rough: r }, inputs: {}, params: {} })[0] as number;
        sum += val * lz * 4 * vh * st * (Math.PI / 2 / T) * (2 * Math.PI / P);
      }
    }
    return sum;
  };
  /** e from pbr's fitted lobe albedo (src/style/pbr.ts, `lobeAlbedo`): the split sum's A + B. */
  const fitted = (mu0: number, r0: number): number => {
    const mu = Math.max(mu0, 0.05), r = Math.max(r0, 0.05), p = (a: number, b: number, c: number) => a + mu * (b + mu * c);
    return Math.min(1, Math.max(0, 1 + r * r * (p(-2.407, 5.998, -2.676) + r * (p(5.343, -18.319, 9.583) + r * p(-3.119, 11.39, -6.479)))));
  };
  /** Karis's analytic A + B (2014), the usual choice, for comparison. */
  const karis = (mu: number, r0: number): number => {
    const r = Math.max(r0, 0.05);
    const rx = 1 - r, ry = -0.0275 * r + 0.0425, rz = -0.572 * r + 1.04, rw = 0.022 * r - 0.04;
    const t = Math.min(rx * rx, 2 ** (-9.28 * mu)) * rx + ry;
    return (-1.04 * t + rz) + (1.04 * t + rw);
  };
  for (const r of [0.25, 0.5, 0.75, 1]) {
    const row = [1, 0.5, 0.1].map((mu) => { const e = albedo(mu, r); return `E(${mu}) ${f(e)} (fit ${f(fitted(mu, r))}, Karis ${f(karis(mu, r))})`; });
    console.log(`lobe albedo rough ${r}: ${row.join("  ")}`);
  }
}

// ---- 3. roughness sweep -----------------------------------------------------------------------------------------
{
  // Light and eye together along z, the normal tilted by theta: n.h = n.l = n.v = cos theta, v.h = 1 (F = f0).
  const c = variable("c", "f32"), rough = variable("rough", "f32");
  const spec = compileJs([mul(mul(ggx(c, rough), smithGgx(c, c, rough)), mul(c, Math.PI * 0.04))]);
  let lastW = 0, lastPeak = Infinity;
  for (const r of [0.1, 0.3, 0.5, 0.7, 0.9]) {
    const at = (deg: number) => spec({ vars: { c: Math.cos((deg * Math.PI) / 180), rough: r }, inputs: {}, params: {} })[0] as number;
    const peak = at(0);
    let deg = 0;
    while (deg < 89.99 && at(deg) > peak / 2) deg += 0.005;
    const w = 2 * deg, a = r * r, predicted = (2 * Math.atan(0.644 * a) * 180) / Math.PI;
    // At grazing in this geometry the term tends to alpha^3 of its peak (D falls to alpha^4, Vis n.l rises by 1 / alpha):
    // above one half from rough 0.89 on, so the highlight never halves.
    const width = deg >= 89.99 ? `none (stays above half to grazing; floor ${(at(89.99) / peak).toFixed(2)} of the peak)` : `${w.toFixed(2)} deg (GGX alone ${predicted.toFixed(2)}, ratio ${(w / predicted).toFixed(3)})`;
    console.log(`sweep rough ${r}: FWHM ${width}  peak ${peak.toExponential(3)}  ${w > lastW && peak < lastPeak ? "monotone" : "NOT MONOTONE"}`);
    lastW = w; lastPeak = peak;
  }
}

// ---- 4. metal tints its highlight --------------------------------------------------------------------------------
{
  // No sky; the sun overhead at 1; the eye at angle a on one side of the normal and the sun at a on the other.
  const style = pbr({ sky: () => vec3(0, 0, 0), tone: (x) => x });
  const metal = variable("c_metal", "f32");
  const ctx = shadeVariables(lighting([0, 1, 0], [1, 1, 1]), style, said({ rough: lift<"f32">(0.3), metal }));
  const run = compileJs([style.shade(ctx)]);
  const copper = [0.95, 0.64, 0.54];
  for (const deg of [0, 80]) {
    const a = (deg * Math.PI) / 180;
    // Normal tilted by a toward x from the sun's (0, 1, 0); the eye mirrored about it, so h = n and v.h = cos a.
    const n = [Math.sin(a), Math.cos(a), 0], v = [Math.sin(2 * a), Math.cos(2 * a), 0];
    const at = (base: number[], m: number) => run({ vars: { c_base: base, c_normal: n, c_view: v, c_world: [0, 1, 0], c_depth: -1, c_metal: m }, inputs: {}, params: {} })[0] as number[];
    const met = at(copper, 1), die = at([0, 0, 0], 0), dieBody = at(copper, 0);
    console.log(`metal at v.h ${f(Math.cos(a))}: copper ${met.map(f).join(", ")}  G/R ${f(met[1]! / met[0]!)} B/R ${f(met[2]! / met[0]!)}  dielectric highlight ${die.map(f).join(", ")} (${f(die[0]! / met[0]!)} of the metal's red)  dielectric copper whole ${dieBody.map(f).join(", ")}`);
  }
}
