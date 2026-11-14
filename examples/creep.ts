// A pattern that creeps: reaction and diffusion (Gray-Scott) on a 256 x 256 grid, seeded in a patch at the centre, grows
// across a mat for 30 s. Every state would be 944 MB, so the compiler keeps every 15th (64 MB of checkpoints) and each
// frame replays at most 15 steps from the nearest: exact on the GPU that baked. The mat's height and paint read it.
//   bun src/cli/main.ts clip examples/creep.ts --view whole --seconds 30 --out out/state/creep.mp4
import { add, dot, draw, lt, mix, mul, plane, recur, saturate, scene, select, smoothstep, spacing, sub, surface, toon, vec2, vec3, vec4, type F } from "../src/index.ts";
import { hash2 } from "../src/lib/index.ts";

const N = 256, SIDE = 8;
export const creep = recur({
  name: "creep", size: [N, N], rate: 30, until: 30,
  start: (c) => {
    // A patch of radius 10 cells at the centre, u = 1/2 and v = 1/4 there, with a little noise (Pearson's seed).
    const d = sub(c, [N / 2, N / 2]), seeded = lt(dot(d, d), 100), wobble = mul(sub(hash2(c), 0.5), 0.1);
    return [vec4(select(seeded, add(0.5, wobble), 1), select(seeded, add(0.25, wobble), 0), 0, 0)];
  },
  step: (prev, c) => {
    const [s] = prev(c);
    const n = (dx: number, dy: number) => prev(add(c, [dx, dy]))[0]!;
    const lap = sub(add(add(n(1, 0), n(-1, 0)), add(n(0, 1), n(0, -1))), mul(s!, 4));
    const uvv = mul(mul(s!.x, s!.y), s!.y);
    return [vec4(
      add(add(s!.x, mul(lap.x, 0.2)), add(mul(uvv, -1), mul(sub(1, s!.x), 0.037))),
      add(add(s!.y, mul(lap.y, 0.1)), sub(uvv, mul(s!.y, 0.097))), 0, 0)];
  },
});

const height = (u: F, v: F): F => mul(smoothstep(0.1, 0.35, creep.on(vec2(u, v))[0]!.y), 0.25);
const mat = surface((u: F, v: F) => vec3(mul(sub(u, 0.5), SIDE), add(0.02, height(u, v)), mul(sub(v, 0.5), SIDE)), { u: ["free", "free"], v: "free" });

export default scene({
  ground: plane, style: toon(), sun: [-0.4, 0.8, 0.45],
  things: [draw.surface("mat", mat, {
    paint: (u, v) => mix(vec3(0.2, 0.3, 0.32), vec3(0.95, 0.62, 0.3), saturate(mul(creep.on(vec2(u, v))[0]!.y, 3))),
    grid: { u: spacing.even(N), v: spacing.even(N) }, sides: "two", outline: "none",
  })],
  cameras: { whole: { kind: "perspective", eye: [0, 7.5, 7.5], target: [0, 0, 0.3], fovY: 0.75 } },
});
