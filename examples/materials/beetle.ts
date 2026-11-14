// A beetle with a thin-film shell: magenta face on, green at a slant, blue at the rim. The film is the shell's answer
// to each light (`scatter`), at the light's half vector, as a film really answers; the style still bands it, shadows
// it and reflects the sky by its `gloss`.
import { add, cos, div, dot, max, mul, normalize, plane, scene, sin, sqrt, strand, sub, toon, vec3, type F, type P3 } from "../../src/index.ts";

/**
 * Two-beam interference in a film of index 1.56 and thickness `nm`, at 650, 530 and 450 nm: 0.5 + 0.5 cos(4 pi n d
 * cos(theta_t) / lambda), theta_t from Snell's law. The constant phase lost on reflection is dropped.
 */
function film(cosI: F, nm: F): P3 {
  const n = 1.56;
  const cosT = sqrt(sub(1, div(sub(1, mul(cosI, cosI)), n * n)));
  const k = (l: number) => (4 * Math.PI * n) / l;
  return add(mul(cos(mul(vec3(k(650), k(530), k(450)), mul(nm, cosT))), 0.5), 0.5);
}

const beetle = strand({
  name: "beetle",
  spine: (m: F) => vec3(mul(sub(m, 0.5), 2.2), 0.35, 0),
  girth: (m: F) => add(0.12, mul(sin(mul(m, Math.PI)), 0.55)),
  section: { aspect: 0.55 },
  paint: (m: F) => ({
    base: [0.08, 0.1, 0.06],
    gloss: 0.85,
    // The film's colour at each light's half vector, times the light's cosine.
    scatter: (ctx, l) => mul(film(dot(ctx.normal, normalize(add(l, ctx.view))), add(400, mul(m, 60))), max(dot(ctx.normal, l), 0)),
  }),
});

export default scene({
  ground: plane, things: [beetle], style: toon(), sun: [-0.4, 0.8, 0.45],
  cameras: {
    whole: { kind: "perspective", eye: [3.5, 2.5, 4], target: [0, 0.35, 0], fovY: 0.6 },
    shell: { kind: "perspective", eye: [0.8, 1.6, 1.4], target: [0, 0.4, 0], fovY: 0.5 },
  },
});
