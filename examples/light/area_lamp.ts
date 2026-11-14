// A lamp with a size over a floor and a post: the light function casts its own shadow ray (`castRay`) toward the lamp.
// A frame lights from the lamp's centre: a hard shadow, the same picture as the lamp with no size. A still
// (`--samples 64`) lights each sample from a point on the lamp's disc (`xi`): the shadow and the pool's rings come out
// soft. Draw both:
//   bun src/cli/main.ts render examples/light/area_lamp.ts --views above --out out/light/area_frame.webp
//   bun src/cli/main.ts render examples/light/area_lamp.ts --views above --samples 64 --out out/light/area_still.webp
import { castRay, f, lamp, mul, plane, scene, smoothstep, strand, toon, vec3, type Light } from "../../src/index.ts";

export const SIZE = Number(process.env.LAMP_SIZE ?? 0.2);
const bulb = lamp([0.6, 2.2, 0], { colour: [1.5, 1.2, 0.8], reach: 1.6, size: SIZE });

/** The bulb, unseen where the post stands between it and the point (a ray of a hair's width: each sample hard). */
const shaded: Light = (p, xi) => {
  const a = bulb(p, xi);
  const hit = castRay({ o: p, d: a.toward, t0: f(0.03), t1: a.distance as ReturnType<typeof f>, width: () => f(0.002) });
  return { ...a, amount: mul(a.amount, smoothstep(0, 0.002, hit.gap)) };
};

const post = strand({ name: "post", spine: (m) => vec3(0, mul(m, 1.6), 0), girth: 0.12, tips: "round", paint: [0.75, 0.55, 0.35] });

export default scene({
  ground: plane,
  things: [post],
  style: toon(),
  sun: { toward: [-0.4, 0.8, 0.3], amount: [0.05, 0.06, 0.1] },
  lights: [shaded],
  cameras: {
    above: { kind: "perspective", eye: [-1.5, 5.5, 3.5], target: [-0.6, 0, 0], fovY: 0.75 },
  },
});
