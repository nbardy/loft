// One scene, two precision choices.
//   bun src/cli/main.ts render examples/mixed_precision.ts --precision mixed --views three --out out/mixed.webp
//   bun src/cli/main.ts build examples/mixed_precision.ts --precision mixed --out out/mixed
import { mul, nothing, pbr, scene, sqrt, strand, sub, tabulate, vec3 } from "../src/index.ts";
import { budget } from "../src/checks.ts";
import { bloom } from "../src/style/post.ts";

const copper = tabulate("copper_paint", (x) => vec3(x.mul(0.42).add(0.47), x.mul(0.38).add(0.22), x.mul(0.29).add(0.17)), 256, "static-shading");
const ball = strand({
  name: "copper",
  spine: (u) => vec3(0, mul(sub(u, 0.5), 2), 0),
  girth: (u) => sqrt(u.mul(sub(1, u))).mul(2),
  paint: (u) => ({ base: copper(u), metal: 0.74, rough: 0.36 }),
});

const look = pbr();
export default scene({
  ground: nothing,
  things: [ball],
  style: { ...look, post: [bloom({ gain: 0.35 })] },
  sun: [-0.45, 0.8, 0.5],
  cameras: { three: { kind: "perspective", eye: [2.8, 1.3, 3.6], target: [0, 0, 0], fovY: 0.65 } },
});

export const checks = [budget({ triangles: 20_000 })];
