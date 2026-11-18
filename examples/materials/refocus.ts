// The copper-sphere study focused on the hammered ball's front face.
// bun src/cli/main.ts render examples/materials/refocus.ts --views close --size 1280x720 --out out/refocus.webp
import { budget, depthOfField, filmGrain, withStyle } from "../../src/index.ts";
import spheres from "./spheres.ts";

const camera = { kind: "perspective", eye: [3.1, 1.55, 3], target: [2.9, 0.5, 0.65], fovY: 0.61 } as const;

export default withStyle({ ...spheres, cameras: { close: camera } }, {
  ...spheres.style,
  post: [...spheres.style.post, depthOfField({ focus: 1.75, focalLength: 0.16, fStop: 1.8 }), filmGrain()],
});

export const checks = [budget({ triangles: 110_000 })];
