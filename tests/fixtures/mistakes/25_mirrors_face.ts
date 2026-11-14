// mistake: two mirrors that face each other: each mirror's picture holds the other mirror, so neither can be drawn first
import { draw, pass, scene, spacing, surface, vec3, type F } from "../../../src/index.ts";
import { cameras, style, water } from "./_parts.ts";

const left = pass({ name: "mirrorLeft", size: { scale: 0.5 }, camera: { kind: "mirror", normal: [1, 0, 0], offset: -3 } });
const right = pass({ name: "mirrorRight", size: { scale: 0.5 }, camera: { kind: "mirror", normal: [-1, 0, 0], offset: -3 } });
const wall = (x: number) => surface((u: F, v: F) => vec3(x, u.mul(2), v.mul(2).sub(1)), { u: ["free", "free"], v: "free" });
const grid = { u: spacing.even(2), v: spacing.even(2) };

export default scene({
  ground: water, style, sun: [0.3, 0.8, 0.4], cameras,
  things: [
    draw.surface("leftGlass", wall(-3), { paint: () => left.at([0.5, 0.5]).xyz, grid, sides: "two", outline: "none" }),
    draw.surface("rightGlass", wall(3), { paint: () => right.at([0.5, 0.5]).xyz, grid, sides: "two", outline: "none" }),
  ],
});
