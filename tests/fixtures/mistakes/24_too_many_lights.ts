// mistake: eight lamps besides the sun, past the cap of seven (each is a term in every lit fragment)
import { draw, lamp, scene } from "../../../src/index.ts";
import { ball, cameras, grid, paint, style, water } from "./_parts.ts";

const lights = Array.from({ length: 8 }, (_, i) => lamp([i - 4, 2, 0], { colour: [1, 0.8, 0.5], reach: 1 }));
export default scene({ ground: water, things: [draw.surface("ball", ball, { paint, grid, sides: "one", outline: "hull" })], style, sun: [0.3, 0.8, 0.4], lights, cameras });
