// Balls by roughness, left to right 0.1, 0.3, 0.5, 0.7, 0.9: copper in front (metal 1), the same colour as a
// dielectric behind (metal 0), and a hammered copper ball at the side whose dents are a bump (`normal`, the gradient of
// a height). Written for `pbr`; in any other style the words it does not read are noted.
//   bun src/cli/main.ts render examples/materials/spheres.ts --style pbr --views whole,close --out out/pbr/spheres.webp
import { dot, gradient, mul, noise3, normalize, pbr, plane, scene, sqrt, strand, sub, vec3, type F, type Painted, type PaintAt } from "../../src/index.ts";

const COPPER = [0.95, 0.64, 0.54] as const;
const ROUGH = [0.1, 0.3, 0.5, 0.7, 0.9];

/** A ball of radius r centred at (x, r, z): a short strand whose girth is a circle's half-chord. */
const ball = (name: string, x: number, z: number, r: number, paint: (u: F, v: F, at: PaintAt) => Painted) => strand({
  name,
  spine: (m: F) => vec3(x, r, mul(sub(m, 0.5), 2 * r)).add(vec3(0, 0, z)),
  girth: (m: F) => mul(sqrt(mul(m, sub(1, m))), 2 * r),
  stations: 24,
  paint,
});

const metals = ROUGH.map((rough, i) => ball(`metal${i}`, (i - 2) * 1.2, 1, 0.5, () => ({ base: COPPER, metal: 1, rough })));
const plastics = ROUGH.map((rough, i) => ball(`plastic${i}`, (i - 2) * 1.2, -0.4, 0.5, () => ({ base: COPPER, rough })));
const hammered = ball("hammered", 3.6, 1, 0.5, (_u, _v, at) => {
  // Dents about 0.08 across and a centimetre deep: the bump is the height's gradient in the world less its part along the normal.
  const g = gradient(mul(noise3(mul(at.world, 12)), 0.01), at.world);
  return { base: COPPER, metal: 1, rough: 0.3, normal: normalize(sub(at.normal, sub(g, mul(at.normal, dot(at.normal, g))))) };
});

export default scene({
  ground: plane,
  things: [...metals, ...plastics, hammered],
  style: pbr(),
  sun: [-0.4, 0.8, 0.45],
  cameras: {
    whole: { kind: "perspective", eye: [0.6, 4.5, 10], target: [0.6, 0.4, 0.3], fovY: 0.75 },
    close: { kind: "perspective", eye: [3.2, 1.3, 2.6], target: [3.4, 0.5, 1], fovY: 0.45 },
  },
});
