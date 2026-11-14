// mistake: a recurrence's step reads a pass (a state stepped from a rendered picture is frame history by the back door)
import { add, draw, pass, recur, scene, surface, vec4, type F } from "../../../src/index.ts";
import { ball, cameras, grid, paint, style, water } from "./_parts.ts";

const above = pass({ name: "above", size: [64, 64], keep: "depth", camera: { kind: "look", eye: [0, 10, 0], target: [0, 0, 0], lens: { halfHeight: 3 } } });
const drift = recur({ name: "drift", size: 1, rate: 30, until: 4, start: () => [vec4(0)], step: (prev, i) => [add(prev(i)[0]!, vec4(above.at([0.5, 0.5]).x, 0, 0, 0))] });
const moved = surface((u: F, v: F) => add(ball.point(u, v), drift.at(0)[0]!.xyz), ball.topology);
export default scene({ ground: water, things: [draw.surface("ball", moved, { paint, grid, sides: "one", outline: "hull" })], style, sun: [0.3, 0.8, 0.4], cameras });
