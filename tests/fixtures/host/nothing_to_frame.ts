// A scene whose only thing has no shape on the CPU (a glow), and no "whole": `--view whole` refuses, naming the fix.
import { glow, plane, scene, toon, vec3, type F } from "../../../src/index.ts";

export default scene({
  ground: plane,
  things: [glow("spark", (u: F) => vec3(u, 1, 0), { width: 0.05, halo: 4, colour: [1, 0.8, 0.3], core: [1, 1, 1], veil: { kind: "clear" }, intensity: 1, segments: 4 })],
  style: toon(),
  sun: [-0.4, 0.8, 0.45],
});
