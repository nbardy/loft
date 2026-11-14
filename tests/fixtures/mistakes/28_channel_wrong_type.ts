// mistake: a paint gives "gloss" as a colour; toon reads it as one number
import { draw, scene, vec3 } from "../../../src/index.ts";
import { ball, cameras, grid, style, water } from "./_parts.ts";

export default scene({ ground: water, things: [draw.surface("ball", ball, { paint: () => ({ base: vec3(0.9, 0.4, 0.2), gloss: vec3(0.8, 0.8, 0.8) }), grid, sides: "one", outline: "hull" })], style, sun: [0.3, 0.8, 0.4], cameras });
