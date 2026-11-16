// mistake: "wrap" is not a topology word; v is "periodic" (closed) or "free" (open)
import { cos, draw, mul, scene, sin, surface, vec3, type F } from "../../../src/index.ts";
import { cameras, grid, paint, style, water } from "./_parts.ts";

const tube = surface((u: F, v: F) => vec3(mul(cos(mul(v, 2 * Math.PI)), 0.4), mul(u, 2), mul(sin(mul(v, 2 * Math.PI)), 0.4)),
  // @ts-expect-error: tsc refuses this too; the fixture checks what render says
  { u: ["free", "free"], v: "wrap" });
export default scene({ ground: water, things: [draw.surface("tube", tube, { paint, grid, sides: "two", outline: "border" })], style, sun: [0.3, 0.8, 0.4], cameras });
