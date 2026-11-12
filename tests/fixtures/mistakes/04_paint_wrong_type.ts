// mistake: paint returns a number, not a colour (vec3)
import { draw, scene } from "../../../src/index.ts";
import { ball, cameras, grid, style, water } from "./_parts.ts";

const grey = (u: never) => u;
export default scene({ sea: water, things: [draw.surface("ball", ball, { paint: grey, grid, sides: "one", outline: "hull" })], style, sun: [0.3, 0.8, 0.4], cameras });
