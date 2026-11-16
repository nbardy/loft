// mistake: a style's outline is the ink width (a number); "hull" is a drawable's outline word
import { draw, scene, toon } from "../../../src/index.ts";
import { ball, cameras, grid, paint, water } from "./_parts.ts";

const style = { ...toon({ bands: 3 }), outline: "hull" } as never; // tsc refuses this; a scene run without tsc reaches the compiler
export default scene({ ground: water, things: [draw.surface("ball", ball, { paint, grid, sides: "one", outline: "hull" })], style, sun: [0.3, 0.8, 0.4], cameras });
