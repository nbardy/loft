// mistake: a recurrence's step reads a host input (a baked state cannot follow what a game changes after the bake)
import { add, draw, input, recur, scene, surface, vec4, type F } from "../../../src/index.ts";
import { ball, cameras, grid, paint, style, water } from "./_parts.ts";

const push = input("push", "f32", [0]);
const drift = recur({ name: "drift", size: 1, rate: 30, until: 4, start: () => [vec4(0)], step: (prev, i) => [add(prev(i)[0]!, vec4(push, 0, 0, 0))] });
const moved = surface((u: F, v: F) => add(ball.point(u, v), drift.at(0)[0]!.xyz), ball.topology);
export default scene({ ground: water, things: [draw.surface("ball", moved, { paint, grid, sides: "one", outline: "hull" })], style, sun: [0.3, 0.8, 0.4], cameras });
