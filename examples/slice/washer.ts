import { scene, thing } from "../../src/render/record.ts";
import { washer, box } from "../../src/kinds/shapes.ts";
import { matte } from "../../src/kinds/materials.ts";
import { inspect } from "../../src/engines/inspect.ts";
export const ring = { ...thing(washer(0.5, 1), matte([0.8, 0.3, 0.1]), [0, 0, 1]), id: 7 };
export const card = { ...thing(box([4, 4, 0.01]), matte([0.7, 0.7, 0.7]), [0, 0, -0.01]), id: 9 };
export default scene({
  things: [ring, card],
  cameras: { front: { kind: "orthographic", eye: [0, 0, 3], target: [0, 0, 0], height: 8 } },
  engine: inspect(),
});
