// The first lantern at night: the flame inside lights the floor round it; the paper glows (`emits`, how the surface
// looks; not a light). One `const` says where the flame is, and the lamp lights from it. Draw it with
//   bun src/cli/main.ts render examples/light/lantern_night.ts --views three,low --out out/light/lantern.webp
import { lamp, mul, plane, scene, strand, toon, vec3 } from "../../src/index.ts";
import { cameras, paper, radius } from "../first/lantern.ts";

const FLAME = vec3(0, 0.9, 0); // inside the paper, half way up

const lantern = strand({
  name: "lantern",
  spine: (m) => vec3(0, m.mul(1.8).add(0.04), 0),
  girth: radius,
  tips: "open",
  paint: (m) => ({ base: paper(m), emits: mul(paper(m), 0.9) }), // lit from inside: the paper's own light, not a light
});

export const flame = lamp(FLAME, { colour: [1.6, 1.0, 0.5], reach: 0.9 });

export default scene({
  ground: plane,
  things: [lantern],
  style: toon(),
  sun: { toward: [-0.55, 0.75, 0.35], amount: [0.06, 0.07, 0.14] }, // the moon: a dim blue key
  lights: [flame],
  cameras,
});
