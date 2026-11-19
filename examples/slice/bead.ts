import { scene, thing } from "../../src/render/record.ts";
import { bead, plane } from "../../src/kinds/shapes.ts";
import { matte } from "../../src/kinds/materials.ts";
import { flat, print } from "../../src/engines/print.ts";
import { hash } from "../../src/text/kind.ts";

const clay = matte([0.8, 0.3, 0.2]);
export default scene({
  things: [
    { ...thing(bead(1, 0.35, 0.06), clay, [0, 1, 0]), ink: 0.04, id: hash("bead") },
    { ...thing(plane(1000), clay), id: hash("ground") },
  ],
  cameras: {
    whole: { kind: "perspective", eye: [2.5, 3.5, 3.5], target: [0, 1, 0], fovY: 0.6 },
    through: { kind: "perspective", eye: [0.05, 4.2, 0.08], target: [0, 1, 0], fovY: 0.6 },
  },
  engine: print({
    shade: flat,
    background: `fn background(r:Ray)->vec4f {
    let id=${hash("sky")}u;return vec4f(vec3f(vec3u(id>>16u,id>>8u,id)&vec3u(255))/255,1);
  }`,
    finish: "fn finish(c:vec4f,t:f32)->vec4f {return vec4f(c.rgb,min(1,log2(1+t)/10));}",
  }),
});
