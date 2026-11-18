// mistake: two holds of a solve that no curve meets together (planted until 1.3 s, but on the deck at 1.2 s)
import { add, always, draw, equal, scene, solve, spline, surface, time, vec3, type F } from "../../../src/index.ts";
import { ball, cameras, grid, paint, style, water } from "./_parts.ts";

const hop = solve({
  over: [0, 2], vary: { y: spline(8) },
  hold: ({ y }) => [always([time, 0, 1.3], equal(y, 0)), always([time, 1.2, 1.2], equal(y, 0.7))],
  prefer: { jerk: 1 },
});
const moved = surface((u: F, v: F) => add(ball.point(u, v), vec3(0, hop.curves.y, 0)), ball.topology);
export default scene({ ground: water, things: [draw.surface("ball", moved, { paint, grid, sides: "one", outline: "hull" })], style, sun: [0.3, 0.8, 0.4], cameras });
