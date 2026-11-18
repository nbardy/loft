// The beetle of beetle.ts in `pbr`, with one part swapped: its Fresnel is a thin film, so the shell reflects the sky
// and the sun magenta face on, green at a slant and blue at the rim, while distribution, visibility, the sky and the
// tone map stay pbr's own. The ink is overridden as a plain field of the Style record.
//   bun src/cli/main.ts render examples/materials/thin_film.ts --views whole,shell --out out/pbr/thin_film.webp
import { add, cos, div, mul, pbr, plane, scene, sin, sqrt, strand, sub, vec3, type F, type P3, type Style } from "../../src/index.ts";

/**
 * Two-beam interference in a film of index 1.56 and `nm` thick, at 650, 530 and 450 nm, as a Fresnel law: f0 plus
 * 0.5 (0.5 + 0.5 cos(4 pi n d cos(theta_t) / lambda)), theta_t from Snell's law. The body under it keeps its own f0.
 */
const film = (nm: number) => (v_h: F, f0: P3): P3 => {
  const n = 1.56;
  const cosT = sqrt(sub(1, div(sub(1, mul(v_h, v_h)), n * n)));
  const k = (l: number) => (4 * Math.PI * n * nm) / l;
  return add(f0, mul(add(mul(cos(mul(vec3(k(650), k(530), k(450)), cosT)), 0.5), 0.5), 0.5));
};

const beetle = strand({
  name: "beetle",
  spine: (m: F) => vec3(mul(sub(m, 0.5), 2.2), 0.35, 0),
  girth: (m: F) => add(0.12, mul(sin(mul(m, Math.PI)), 0.55)),
  section: { aspect: 0.55 },
  paint: () => ({ base: [0.05, 0.06, 0.04], rough: 0.3 }),
});

const style: Style = { ...pbr({ fresnel: film(380) }), ink: () => vec3(0.05, 0.06, 0.08) };

export default scene({
  ground: plane, things: [beetle], style, sun: [-0.4, 0.8, 0.45],
  cameras: {
    whole: { kind: "perspective", eye: [3.5, 2.5, 4], target: [0, 0.35, 0], fovY: 0.6 },
    shell: { kind: "perspective", eye: [0.8, 1.6, 1.4], target: [0, 0.4, 0], fovY: 0.5 },
  },
});
