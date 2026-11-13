// The same reeds with their own "whole": that camera is used, nothing is derived (tests/cli.host.test.ts).
import { scene } from "../../../src/index.ts";
import base from "./no_whole.ts";

export default scene({
  ground: base.ground,
  things: base.things.filter((t) => t.name === "left" || t.name === "right"),
  style: base.style,
  sun: [-0.4, 0.8, 0.45],
  cameras: { whole: { kind: "perspective", eye: [6, 4, 8], target: [0, 2, 0], fovY: 0.6 } },
});
