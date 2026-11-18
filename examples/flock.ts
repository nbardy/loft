// A flock: 200 motes keep apart, close in and fly with their neighbours within 2 units, and chase a lure that is a
// closed form of time. Every step is kept (30 s at 30 steps a second, 5.8 MB), so each mote is a closed form over a
// static table: its copy reads `flock.at(i)` like any other expression. Chaotic, so it never comes round: no `period`.
//   bun src/cli/main.ts clip examples/flock.ts --view whole --seconds 10 --out out/state/flock.mp4
import { add, and, cos, div, dot, draw, f, iterate, lt, max, mul, ne, nothing, points, recur, scene, sin, select, strand, sub, through, time, toon, vec3, vec4, type F } from "../src/index.ts";
import { hash33 } from "../src/lib/index.ts";

const n = 200, dt = 1 / 30;
// Drawn below too. Inside the step, `time` is the step's own time, so the flock chases the lure where it was then.
const lure = vec3(mul(sin(mul(time, 0.4)), 6), 3, mul(cos(mul(time, 0.3)), 6));
export const flock = recur({
  name: "flock", size: n, rate: 30, until: 30,
  start: (i) => [vec4(mul(sub(hash33(vec3(i, 1, 2)), 0.5), 8), 0), vec4(0)], // lanes: position, velocity
  step: (prev, i) => {
    const [p, v] = prev(i);
    // Separation, the neighbours' centre and their mean velocity, over every other mote within 2 units.
    const [sep, mid, vel, k] = iterate(n, [vec3(0), vec3(0), vec3(0), f(0)] as const, ([s, c, a, m], j) => {
      const [q, w] = prev(j);
      const d = sub(q!.xyz, p!.xyz), d2 = add(dot(d, d), 1e-4), near = select(and(lt(d2, 4), ne(j, i)), 1, 0);
      return [sub(s, mul(d, div(mul(near, 0.3), d2))), add(c, mul(d, near)), add(a, mul(w!.xyz, near)), add(m, near)] as const;
    });
    const k1 = max(k, 1);
    // Apart, together, along with the neighbours, toward the lure, and a little drag so the chase does not overshoot.
    const acc = add(add(sep, mul(div(mid, k1), 0.4)), add(add(mul(sub(div(vel, k1), v!.xyz), 0.6), mul(sub(lure, p!.xyz), 0.6)), mul(v!.xyz, -0.4)));
    const v2 = add(v!.xyz, mul(acc, dt));
    return [vec4(add(p!.xyz, mul(v2, dt)), 0), vec4(v2, 0)];
  },
});

const mote = strand({
  name: "mote", spine: (m: F) => through(m, [[-0.1, 0, 0], [0.1, 0, 0]]), stations: 4,
  girth: (m: F) => mul(sin(mul(m, Math.PI)), 0.08), paint: [0.95, 0.85, 0.6], detail: { along: 6, around: 8 },
});
const bait = strand({ name: "bait", spine: (m: F) => add(lure, vec3(mul(sub(m, 0.5), 0.5), 0, 0)), girth: (m: F) => mul(sin(mul(m, Math.PI)), 0.25), paint: [0.9, 0.25, 0.15] });

export default scene({
  ground: nothing, style: toon(), sun: [-0.4, 0.8, 0.45],
  things: [draw.posed("motes", points(n, (i) => ({ at: flock.at(i)[0]!.xyz, scale: 1 })), mote), bait],
  cameras: { whole: { kind: "perspective", eye: [0, 8, 24], target: [0, 3, 0], fovY: 0.75 } },
});
