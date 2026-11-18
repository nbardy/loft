// `pbr` and its parts (src/style/pbr.ts, src/style/pbr_parts.ts) on both backends. Bugs these catch: a part written
// with something only one backend has (a derivative, raw WGSL) or emitted differently by the two (an operator's
// argument order, a pow of 0); the GGX peak losing its precision in f32 at the roughness floor; and energy made or lost
// by the environment's split sum or the sky's projection (the white furnace: a white body under a uniform sky of 0.5
// with no sun must show 0.5 at every normal and roughness). And the review's D9 (2026-11-18): the environment's
// specular weight was a fit to the default lobe, kept when the lobe was swapped (a lobe of 0 still reflected the sky);
// a sky that read time was projected at time 0 without a word; a sky the CPU cannot evaluate crashed the constructor
// with a plain Error even beside an `environment` that never projects it; the sky's scaling by the sun was untested.
import { expect, test } from "bun:test";
import { add, div, dot, lamp, lib, lift, max, mul, normalize, saturate, sub, time, variable, vec3, type Expr, type NumTy } from "../src/index.ts";
import { aces, ggx, lambert, pbr, schlick, skyLight, smithGgx, daylight } from "../src/styles.ts";
import { shadeVariables } from "../src/style.ts";
import { foldAt, type Arrived, type Lighting } from "../src/light.ts";
import { PbrError } from "../src/style/pbr_parts.ts";
import { evaluate } from "../src/expr.ts";
import { compare, rng, type Sample } from "./lib.harness.ts";

const unit = (r: () => number): number[] => {
  for (;;) {
    const v = [r() * 2 - 1, r() * 2 - 1, r() * 2 - 1];
    const l = Math.hypot(v[0]!, v[1]!, v[2]!);
    if (l > 0.2 && l < 1) return v.map((x) => x / l);
  }
};
/** A unit vector within 85 degrees of n. */
const facing = (r: () => number, n: number[]): number[] => {
  for (;;) { const v = unit(r); if (v[0]! * n[0]! + v[1]! * n[1]! + v[2]! * n[2]! > 0.09) return v; }
};

const SUN = [-0.4, 0.8, 0.45].map((x) => x / Math.hypot(0.4, 0.8, 0.45)) as [number, number, number];
const lighting = (amount: readonly [number, number, number], lights: Lighting["lights"], dir = SUN): Lighting => ({
  sun: { kind: "fixed", dir },
  amount: { kind: "given", amount: [...amount] }, lights, sampling: { kind: "point" }, shadows: new Map(),
});
const channels = (m: Record<string, Expr<NumTy>>) => ({ channels: new Map(Object.entries(m)), scatter: undefined });

test("pbr's parts and its body term agree between the twin and the GPU", async () => {
  const n = variable("n", "vec3"), v = variable("v", "vec3"), l = variable("l", "vec3"), rough = variable("rough", "f32"), c = variable("c", "vec3");
  const h = normalize(add(l, v));
  const n_l = saturate(dot(n, l)), n_v = max(dot(n, v), 1e-4), n_h = saturate(dot(n, h)), v_h = saturate(dot(v, h));
  const style = pbr();
  const ctx = shadeVariables(lighting([1, 1, 1], [lamp([0.5, 1.5, 0.5], { colour: [1.6, 1.0, 0.5], reach: 1 })]), style,
    channels({ rough: variable("c_rough", "f32"), metal: variable("c_metal", "f32") }));
  const env = skyLight({ normal: n, view: v, sun: vec3(0, 1, 0) }, rough, daylight);
  const exprs: Expr[] = [
    ggx(n_h, rough), smithGgx(n_l, n_v, rough), schlick(v_h, c), lambert(c, n_l, n_v, v_h, rough), aces(mul(c, 3)),
    env.diffuse, env.specular, style.shade(ctx), style.ink(ctx),
  ];
  const r = rng(7);
  const samples: Sample[] = Array.from({ length: 256 }, () => {
    const nn = unit(r), vv = facing(r, nn), w = [r() * 4 - 2, r() * 2, r() * 4 - 2];
    return {
      n: nn, v: vv, l: facing(r, nn), rough: 0.05 + r() * 0.95, c: [r(), r(), r()],
      c_normal: nn, c_view: vv, c_world: w, c_base: [r(), r(), r()], c_depth: r() - 0.8, c_rough: r(), c_metal: r() < 0.5 ? 0 : 1,
    };
  });
  const cmp = await compare(exprs, samples, "pbr");
  // |gpu - cpu| / max(1, |cpu|): the GGX peak reaches 5e4, so this is relative there and absolute on colours.
  expect(cmp.worst).toBeLessThan(1e-3);
});

test("white furnace: a white body under a uniform sky of 0.5 and no sun shows 0.5 at every normal and roughness", async () => {
  const style = pbr({ sky: () => vec3(0.5, 0.5, 0.5), tone: (x) => x });
  // pbr scales the sky by the sun's amount (a moonless night has a dark sky), so the sun stays at 1 but shines from
  // straight below: on the upper half of the normals sampled it adds exactly nothing (n.l clamps to 0).
  const ctx = shadeVariables(lighting([1, 1, 1], [], [0, -1, 0]), style, channels({ rough: variable("c_rough", "f32") }));
  const shade = style.shade({ ...ctx, base: lift<"vec3">([1, 1, 1]) });
  const r = rng(11);
  const samples: Sample[] = [0.05, 0.2, 0.5, 1].flatMap((rough) => Array.from({ length: 64 }, () => {
    let nn = unit(r);
    while (nn[1]! < 0) nn = unit(r);
    return { c_normal: nn, c_view: facing(r, nn), c_world: [0, 1, 0], c_depth: -1, c_rough: rough };
  }));
  const cmp = await compare([shade, sub(shade, 0.5)], samples, "furnace");
  expect(cmp.worst).toBeLessThan(1e-4);
  for (const out of cmp.cpu) for (const ch of out[0]!) expect(Math.abs(ch - 0.5)).toBeLessThan(1e-3);
});

const UNIFORM = () => vec3(0.5, 0.5, 0.5);
const RADIANCE = (x: Parameters<typeof aces>[0]) => x;
/** Normals in the upper half (the sun shines from straight below and adds nothing there), a view within 85 degrees. */
const upper = (r: () => number, rough: number, n = 48): Sample[] => Array.from({ length: n }, () => {
  let nn = unit(r);
  while (nn[1]! < 0) nn = unit(r);
  return { c_normal: nn, c_view: facing(r, nn), c_world: [0, 1, 0], c_depth: -1, c_rough: rough };
});

test("a lobe of 0 reflects no sky: environment specular 0 in the furnace", async () => {
  const style = pbr({ distribution: () => lift<"f32">(0), sky: UNIFORM, tone: RADIANCE });
  const ctx = shadeVariables(lighting([1, 1, 1], [], [0, -1, 0]), style, channels({ rough: variable("c_rough", "f32") }));
  // A black dielectric: no diffuse, so all that shows is the sky's specular.
  const shade = style.shade({ ...ctx, base: lift<"vec3">([0, 0, 0]) });
  const r = rng(13);
  const cmp = await compare([shade], [0.1, 0.5, 1].flatMap((rough) => upper(r, rough)), "zero lobe");
  for (const side of [cmp.cpu, cmp.gpu]) for (const out of side) for (const ch of out[0]!) expect(Math.abs(ch)).toBeLessThan(1e-4);
});

test("a swapped visibility reflects the sky by its own albedo: a white metal in the furnace within 0.02", async () => {
  // Neumann's visibility, whose albedo differs from smithGgx's by up to 0.37 (at n.v 0.05, rough 1).
  const neumann = (n_l: Expr<"f32">, n_v: Expr<"f32">) => div(0.25, max(max(n_l, n_v), 1e-4));
  const style = pbr({ visibility: neumann, sky: UNIFORM, tone: RADIANCE });
  const ctx = shadeVariables(lighting([1, 1, 1], [], [0, -1, 0]), style, channels({ rough: variable("c_rough", "f32"), metal: lift<"f32">(1) }));
  const shade = style.shade({ ...ctx, base: lift<"vec3">([1, 1, 1]) });
  const r = rng(17);
  const samples = [0.3, 0.6, 1].flatMap((rough) => upper(r, rough, 24));
  const cmp = await compare([shade], samples, "furnace, Neumann");
  // The reference: 0.5 x the lobe's albedo with F = 1 (f0 of a white metal), by a quadrature over light directions
  // (not the build's half-vector one), D = GGX, Vis = Neumann.
  const albedo = (mu: number, rough: number): number => {
    const a2 = Math.max(rough, 0.05) ** 4, v = [Math.sqrt(1 - mu * mu), 0, mu], N = 400;
    let sum = 0;
    for (let i = 0; i < N; i++) for (let j = 0; j < N; j++) {
      const ct = (i + 0.5) / N, st = Math.sqrt(1 - ct * ct), phi = (2 * Math.PI * (j + 0.5)) / N;
      const l = [st * Math.cos(phi), st * Math.sin(phi), ct];
      const h = [l[0]! + v[0]!, l[1]!, l[2]! + v[2]!], hl = Math.hypot(h[0]!, h[1]!, h[2]!);
      const nh = h[2]! / hl, d = a2 / (Math.PI * (nh * nh * (a2 - 1) + 1) ** 2);
      sum += d * (0.25 / Math.max(ct, mu)) * ct * (1 / N) * ((2 * Math.PI) / N);
    }
    return sum;
  };
  samples.forEach((s, k) => {
    const n = s.c_normal as number[], v = s.c_view as number[];
    const want = 0.5 * albedo(n[0]! * v[0]! + n[1]! * v[1]! + n[2]! * v[2]!, s.c_rough as number);
    for (const side of [cmp.cpu, cmp.gpu]) for (const ch of side[k]![0]!) expect(Math.abs(ch - want)).toBeLessThan(0.02);
  });
});

test("a sky that reads time or a param is refused by name; a GPU-only sky builds beside its own environment and clear", () => {
  const moving = (dir: Expr<"vec3">) => mul(vec3(0.5, 0.5, 0.5), add(saturate(dir.y), time));
  expect(() => pbr({ sky: moving })).toThrow(PbrError);
  expect(() => pbr({ sky: moving })).toThrow(/sky that reads time/);
  // With a clear colour given, the refusal comes where the sky is projected.
  const style = pbr({ sky: moving, clear: [0.5, 0.6, 0.7] });
  const ctx = shadeVariables(lighting([1, 1, 1], []), style, channels({}));
  expect(() => style.shade(ctx)).toThrow(/sky that reads time/);

  const glow = { name: "test_glow", ret: "vec3", wgsl: "fn test_glow(d: vec3<f32>) -> vec3<f32> { return d * 0.5 + 0.5; }", js: "gpu-only" } as const;
  const gpuSky = (dir: Expr<"vec3">) => lib<"vec3">(glow, dir);
  expect(() => pbr({ sky: gpuSky })).toThrow(PbrError);
  expect(() => pbr({ sky: gpuSky })).toThrow(/pass `clear/);
  const own = pbr({ sky: gpuSky, clear: [0.2, 0.3, 0.4], environment: () => ({ diffuse: vec3(0.3, 0.3, 0.3), specular: vec3(0.3, 0.3, 0.3) }) });
  expect(own.clear).toEqual([0.2, 0.3, 0.4]);
});

test("the sky is radiance per unit of the sun's amount: background and environment scale with it", async () => {
  const style = pbr({ sky: UNIFORM, tone: RADIANCE });
  const at = (amount: number) => {
    const ctx = shadeVariables(lighting([amount, amount, amount], [], [0, -1, 0]), style, channels({ rough: variable("c_rough", "f32") }));
    return [style.shade({ ...ctx, base: lift<"vec3">([1, 1, 1]) }), ctx.sky(normalize(vec3(0.3, 0.8, 0.2)))];
  };
  const cmp = await compare([...at(0), ...at(0.5), ...at(1)], upper(rng(19), 0.5, 16), "sun amount");
  for (const side of [cmp.cpu, cmp.gpu]) for (const out of side) {
    for (const [k, want] of [[0, 0], [1, 0], [2, 0.25], [3, 0.25], [4, 0.5], [5, 0.5]] as const) for (const ch of out[k]!) expect(Math.abs(ch - want)).toBeLessThan(1e-4);
  }
});

test("a fold hands a style the unshadowed arrival and the visibility beside their product", () => {
  const light = lamp([0, 2, 0], { colour: [1, 0.5, 0.25], reach: 1, shadow: true });
  const l: Lighting = { ...lighting([1, 1, 1], [light]), shadows: new Map([[light, () => lift<"f32">(0.25)]]) };
  const fold = foldAt(l, { kind: "point", at: lift<"vec3">([0, 1, 0]) });
  const parts = (pick: (a: Arrived) => Expr<"vec3">) => evaluate(fold("sum", (a, which) => (which === "sun" ? vec3(0, 0, 0) : pick(a)))) as number[];
  const unshadowed = parts((a) => lift<"vec3">(a.unshadowed));
  expect(parts((a) => mul(lift<"vec3">(a.unshadowed), a.visibility))).toEqual(parts((a) => lift<"vec3">(a.amount)));
  expect(parts((a) => mul(vec3(1, 1, 1), a.visibility))).toEqual([0.25, 0.25, 0.25]);
  // The lamp's own law at one reach: colour x (1 - 1/16)^2 x 1/2.
  expect(unshadowed[0]!).toBeCloseTo((15 / 16) ** 2 / 2, 6);
});
