// A grub whose spots come from reaction and diffusion (Gray-Scott, Pearson 1993) on a 128 x 128 grid that wraps (a
// torus, as the skin wraps round the body). The pattern settles for 150 s before t = 0 and only the last state is
// kept, so the skin is a still texture made by a step: 9,000 steps baked on the GPU, 256 KiB kept.
//   bun src/cli/main.ts render examples/skin.ts --views whole --out out/state/skin.webp
import { add, lt, mix, mul, plane, recur, saturate, scene, select, sin, strand, sub, time, toon, vec2, vec3, vec4, TAU, type F } from "../src/index.ts";
import { hash2 } from "../src/lib/index.ts";

const N = 128;
export const skin = recur({
  name: "skin",
  size: [N, N], // cells; prev() wraps at the edges
  rate: 60, from: -150, until: 0, // 9,000 steps, all before the scene starts
  start: (c) => [vec4(1, select(lt(hash2(c), 0.02), 1, 0), 0, 0)], // u = 1 everywhere, v seeded in 2% of cells
  step: (prev, c) => {
    const [s] = prev(c);
    const n = (dx: number, dy: number) => prev(add(c, [dx, dy]))[0]!;
    const lap = sub(add(add(n(1, 0), n(-1, 0)), add(n(0, 1), n(0, -1))), mul(s!, 4));
    const uvv = mul(mul(s!.x, s!.y), s!.y);
    return [vec4(
      add(add(s!.x, mul(lap.x, 0.16)), add(mul(uvv, -1), mul(sub(1, s!.x), 0.03))),
      add(add(s!.y, mul(lap.y, 0.08)), sub(uvv, mul(s!.y, 0.092))), 0, 0)];
  },
});

const grub = strand({
  name: "grub",
  spine: (m: F) => vec3(mul(m, 4), add(0.6, mul(sin(add(mul(m, 6), mul(time, TAU / 4))), 0.15)), 0), // one wave per loop
  girth: (m: F) => mul(sin(mul(m, Math.PI)), 0.5),
  paint: (m, v) => mix(vec3(0.85, 0.75, 0.55), vec3(0.25, 0.12, 0.08), saturate(mul(skin.on(vec2(m, v))[0]!.y, 3))),
});

export default scene({
  ground: plane, things: [grub], style: toon(), sun: [-0.4, 0.8, 0.45], period: 4,
  cameras: { whole: { kind: "perspective", eye: [2, 2.2, 5], target: [2, 0.5, 0], fovY: 0.6 } },
});
