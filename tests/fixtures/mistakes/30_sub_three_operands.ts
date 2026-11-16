// mistake: sub given three operands; only add, mul, min and max take more than two
import { draw, scene, sub, vec3, type F } from "../../../src/index.ts";
import { ball, cameras, grid, style, water } from "./_parts.ts";

// @ts-expect-error: tsc refuses this too; the fixture checks what render says
const paint = (u: F) => vec3(sub(1, u, 0.2), 0.4, 0.2);
export default scene({ ground: water, things: [draw.surface("ball", ball, { paint, grid, sides: "one", outline: "hull" })], style, sun: [0.3, 0.8, 0.4], cameras });
