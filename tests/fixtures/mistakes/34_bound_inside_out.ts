// mistake: bound.box given its corners the wrong way round (max below min on y)
import { bound, draw, length, scene, vec3, type P3 } from "../../../src/index.ts";
import { cameras, style, water } from "./_parts.ts";

const blob = (p: P3) => length(p.add(vec3(0, -1.5, 0))).add(-1);
export default scene({
  ground: water, style, sun: [0.3, 0.8, 0.4], cameras,
  things: [draw.field("blob", blob, { bounds: bound.box([-1.2, 2.7, -1.2], [1.2, 0.3, 1.2]), paint: () => vec3(0.8, 0.3, 0.3), steps: 64, outline: "silhouette", schedule: "trace" })],
});
