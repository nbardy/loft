// A still pool in a plane floor that mirrors what stands round it (`pass`, a mirror camera). Inside 2 units of the
// origin the ground is a dark mirror, rippled by a closed form of time; the mirrored picture is the view's own camera
// reflected in y = 0, half the view's size, drawn with the scene's own style. The ground reads the picture, so the
// ground is left out of it (a pass never draws a reader of itself): the pool does not show the pool.
import { add, bound, cos, div, draw, length, lt, max, mix, mul, pass, plane, scene, select, sin, strand, sub, time, toon, vec2, vec3, type F, type P3, type Style } from "../src/index.ts";

const mirrored = pass({ name: "mirrored", size: { scale: 0.5 }, camera: { kind: "mirror", normal: [0, 1, 0], offset: 0 } });

const base = toon();
const pooled: Style = {
  ...base,
  name: "pooled",
  ground: (ctx) => {
    const r = length(vec2(ctx.world.x, ctx.world.z));
    // Rings running outward, h = a sin(9 r - 2 t). The water tilts by the slope dh/dr, and a tilted mirror moves the
    // image along the rings, so the point looked up is moved along the radius by the slope times a reach (world
    // units, so the ripple stays the same size on the water at any distance). A sideways shift in screen units would
    // drag round reflections into boxes wherever it changes sign from one row to the next.
    const slope = mul(cos(sub(mul(r, 9), mul(time, 2))), 0.05);
    const outward = div(vec3(ctx.world.x, 0, ctx.world.z), max(r, 1e-3));
    const seen = mirrored.at(mirrored.project(add(ctx.world, mul(outward, slope))).xy).xyz;
    return select(lt(r, 2), mix([0.05, 0.08, 0.1], seen, 0.85), base.ground(ctx));
  },
};

/** Posts round the pool, striped, each leaning a little and swaying on its own clock. */
const post = (k: number) => {
  const a = (k / 7) * Math.PI * 2 + 0.3, at: readonly [number, number] = [Math.cos(a) * 2.5, Math.sin(a) * 2.5];
  return strand({
    name: `post${k}`,
    spine: (m: F) => vec3(add(at[0], mul(m, mul(sin(add(time, k)), 0.15))), mul(m, 1.6 + (k % 3) * 0.5), at[1]),
    girth: (m: F) => add(0.16, mul(m, -0.06)),
    tips: ["open", "round"],
    paint: (m: F, v: F) => select(lt(sin(mul(add(m, v), 18)), 0), [0.9, 0.35, 0.25], [0.95, 0.88, 0.7]),
  });
};

/** An arch over the pool. */
const arch = strand({
  name: "arch",
  spine: (m: F) => vec3(mul(cos(mul(m, Math.PI)), 2.2), mul(sin(mul(m, Math.PI)), 2.4), -0.6),
  girth: 0.14,
  paint: [0.3, 0.55, 0.85],
});

/** A stone that floats over the water, traced per pixel, bobbing. */
const ORB: [number, number, number] = [-0.6, 0.9, -0.4];
const orb = draw.field("orb", (p: P3) => sub(length(sub(p, vec3(ORB[0], add(ORB[1], mul(sin(mul(time, 1.5)), 0.15)), ORB[2]))), 0.35), {
  bounds: bound.box([ORB[0] - 0.45, ORB[1] - 0.65, ORB[2] - 0.45], [ORB[0] + 0.45, ORB[1] + 0.65, ORB[2] + 0.45]),
  paint: () => vec3(0.95, 0.75, 0.2), steps: 48, outline: "silhouette", schedule: "trace",
});

export default scene({
  ground: plane, things: [...[0, 1, 2, 3, 4, 5, 6].map(post), arch, orb], style: pooled, sun: [-0.4, 0.8, 0.45],
  cameras: { whole: { kind: "perspective", eye: [5, 3, 6], target: [0, 0.8, 0], fovY: 0.6 } },
});
