// Reeds and a stone on a floor, toon, with shadows from a depth picture taken from the sun (`pass`, keep "depth"):
// every shape the sun sees casts, not only the skeletons' capsules. A reed shades its neighbour and the stone (a
// distance field, traced per pixel) shades the floor by its own depth entry. The picture covers a 12-unit square round
// the origin; the shadow part is a style, so `--style woodblock` drops the pass with the style that read it.
import { add, bound, draw, f, ge, le, length, mix, mul, normalize, pass, plane, scene, select, sin, strand, sub, time, toon, vec2, vec3, type F, type P3, type Style } from "../src/index.ts";

const SUN: readonly [number, number, number] = [-0.4, 0.8, 0.45];
const TEXELS = 2048;
const fromSun = pass({
  name: "fromSun",
  size: [TEXELS, TEXELS],
  keep: "depth", // only how near the sun each texel's first surface is
  camera: { kind: "look", eye: mul(normalize(vec3(...SUN)), 20), target: vec3(0, 0, 0), lens: { halfHeight: 6 } },
});

/**
 * 1 where the sun reaches p, 0 where a nearer surface covers it; nine taps soften the edge over one texel. Beyond the
 * picture's far plane (z above 1: the floor far off) nothing was drawn, so it is lit, as outside the picture (`at` is 1).
 */
function sunlit(p: P3, normal: P3): F {
  const q = fromSun.project(add(p, mul(normal, 0.02))); // xy: where p lands in the picture (0..1); z: its depth there
  let lit: F = f(0);
  for (const dx of [-1, 0, 1]) for (const dy of [-1, 0, 1]) {
    const nearest = fromSun.at(add(q.xy, vec2(dx / TEXELS, dy / TEXELS))).x;
    lit = add(lit, select(le(q.z, add(nearest, 0.002)), 1, 0));
  }
  return select(ge(q.z, 1), 1, mul(lit, 1 / 9));
}

const base = toon();
const shadowed: Style = {
  ...base,
  name: "shadowed",
  shade: (ctx) => mix(mul(base.shade(ctx), 0.55), base.shade(ctx), sunlit(ctx.world, ctx.normal)),
  ground: (ctx) => base.ground({ ...ctx, shadow: sub(1, sunlit(ctx.world, vec3(0, 1, 0))) }),
};

const reed = (k: number) => strand({
  name: `reed${k}`,
  spine: (m: F) => vec3(add(k * 0.7 - 2, mul(mul(m, m), mul(sin(add(time, k)), 0.5))), mul(m, 3.5), k % 2 ? 0.3 : -0.3),
  girth: (m: F) => add(0.2, mul(m, -0.17)),
  tips: ["open", "point"],
  paint: [0.85, 0.55, 0.2],
});

const STONE: [number, number, number] = [1.6, 0.45, 1.3];
const stone = draw.field("stone", (p: P3) => sub(length(sub(p, vec3(...STONE))), 0.55), {
  bounds: bound.sphere(STONE, 0.6), paint: () => vec3(0.55, 0.6, 0.65), steps: 48, outline: "silhouette", schedule: "trace",
});

export default scene({
  ground: plane, things: [...[0, 1, 2, 3, 4, 5].map(reed), stone], style: shadowed, sun: SUN,
  cameras: { whole: { kind: "perspective", eye: [6, 4, 8], target: [0, 1.5, 0], fovY: 0.6 } },
});
