// Two strands and no camera named "whole": `--view whole` derives one (tests/cli.host.test.ts).
import { add, mul, plane, scene, sin, strand, time, toon, vec3, type F } from "../../../src/index.ts";

const reed = (name: string, x: number) => strand({
  name,
  spine: (m: F) => vec3(add(x, mul(mul(m, m), mul(sin(time), 0.6))), mul(m, 4), 0),
  girth: (m: F) => add(0.25, mul(m, -0.2)),
  tips: ["open", "point"],
  paint: [0.85, 0.55, 0.2],
  stations: 8,
  detail: { along: 16, around: 8 },
});

export default scene({
  ground: plane,
  things: [reed("left", -1), reed("right", 1)],
  style: toon(),
  sun: [-0.4, 0.8, 0.45],
  cameras: { tip: { kind: "perspective", eye: [0, 4, 3], target: [0, 4, 0], fovY: 0.5 } },
});
