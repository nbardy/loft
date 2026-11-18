// `pbr` and its parts (src/style/pbr.ts, src/style/pbr_parts.ts) on both backends. Bugs these catch: a part written
// with something only one backend has (a derivative, raw WGSL) or emitted differently by the two (an operator's
// argument order, a pow of 0); the GGX peak losing its precision in f32 at the roughness floor; and energy made or lost
// by the environment's split sum or the sky's projection (the white furnace: a white body under a uniform sky of 0.5
// with no sun must show 0.5 at every normal and roughness).
import { expect, test } from "bun:test";
import { add, dot, lamp, lift, max, mul, normalize, saturate, sub, variable, vec3, type Expr, type NumTy } from "../src/index.ts";
import { aces, ggx, lambert, pbr, schlick, skyLight, smithGgx, daylight } from "../src/styles.ts";
import { shadeVariables } from "../src/style.ts";
import type { Lighting } from "../src/light.ts";
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
