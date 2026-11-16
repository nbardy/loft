// mistake: a flat stroke's paint wraps the style, but a flat look's paint is the final colour: there is no style to wrap
import { draw, mul, scene, vec3, type F } from "../../../src/index.ts";
import { cameras, style, water } from "./_parts.ts";

const line = (u: F | number) => vec3(u, 1.5, 0);
export default scene({
  ground: water,
  things: [draw.curve("stroke", line, { width: () => 0.05, segments: 16, paint: () => ({ base: vec3(1, 1, 1), shade: (_ctx, styled) => mul(styled, 0.5) }) })],
  style, sun: [0.3, 0.8, 0.4], cameras,
});
