// A heron wading at night under one lamp, which casts its own soft shadow (`shadow: true`, DESIGN.md 17): the bird
// shades the mud round its feet and its neck and body shade its legs. The lamp hangs over the middle of the heron's
// circle; the moon is a dim blue key. Draw it with
//   bun src/cli/main.ts render examples/light/heron_lamp.ts --views close,wide --out out/light/heron_lamp.webp
import { lamp, plane, scene, toon } from "../../src/index.ts";
import heron from "../second/heron.ts";

/** Over the middle of the walk, high enough that the whole circle is under it. `size` is the bulb's radius: the edge's softness. */
export const bulb = lamp([0, 4.5, 0], { colour: [2.4, 1.7, 0.9], reach: 3.2, size: 0.3, shadow: true });

export default scene({
  ground: plane,
  // The bird as examples/second/heron.ts writes it, without the scene's own sky, ground and shadows (scene() adds them).
  things: heron.things.filter((t) => !["sky", "ground", "shadows"].includes(t.name)),
  style: toon({ bands: 3, outline: 0.03 }),
  sun: { toward: [-0.4, 0.8, 0.45], amount: [0.05, 0.06, 0.12] },
  lights: [bulb],
  cameras: {
    close: { kind: "perspective", eye: [5.5, 3.2, 3.5], target: [0.8, 0.8, -1.2], fovY: 0.75 },
    wide: { kind: "perspective", eye: [7, 8.5, 7], target: [0, 0.6, 0], fovY: 0.7 },
  },
});
