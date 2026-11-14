// A rope on a moving hand: 40 points hang from an anchor that swings twice in the 8 s loop. The step is a stiff spring
// to each neighbour, gravity and a little damping, 240 steps a second. Run for three loops before t = 0, the damped
// rope settles into the swing's periodic answer, so the loop closes by physics, and `loft check` measures the seam.
//   bun src/cli/main.ts clip examples/rope.ts --view whole --seconds 8 --out out/state/rope.mp4
import { add, dot, eq, f, iterate, length, lt, max, mul, recur, scene, plane, select, sin, strand, sub, time, toon, vec3, vec4, TAU, type Check, type F, type Probe } from "../src/index.ts";

const anchor = vec3(mul(sin(mul(time, TAU / 4)), 1.5), 5, 0); // whole turns in 8 s
const N = 40, REST = 0.1, W2 = 150 * 150, dt = 1 / 240;
export const rope = recur({
  name: "rope", size: N, rate: 240, from: -24, until: 8,
  start: (i) => [vec4(0, sub(5, mul(i, REST)), 0, 0), vec4(0)], // position, velocity
  step: (prev, i) => {
    const [p, v] = prev(i);
    const pull = (j: F) => { // a stiff spring to entry j
      const d = sub(prev(j)[0]!.xyz, p!.xyz), l = max(length(d), 1e-6);
      return mul(d, mul(sub(l, REST), W2).div(l));
    };
    // Index -1 wraps to entry 39; the pinned entry 0 ignores its force.
    const force = add(add(pull(sub(i, 1)), select(lt(i, N - 1), pull(add(i, 1)), vec3(0))), vec3(0, -9.8, 0));
    const v2 = mul(add(v!.xyz, mul(force, dt)), 0.995);
    const pinned = eq(i, 0);
    return [vec4(select(pinned, anchor, add(p!.xyz, mul(v2, dt))), 0), vec4(select(pinned, vec3(0), v2), 0)];
  },
});

const cord = strand({ name: "cord", spine: (m: F) => rope.on(m)[0]!.xyz, girth: 0.03, tips: "open", paint: [0.8, 0.3, 0.2] });

// Energy per unit mass: the bug it catches is an explicit step that blows up (stiffness times dt past 2).
const energy = iterate(N, [f(0)] as const, ([e], j) => {
  const [p, v] = rope.at(j);
  return [add(e, add(mul(dot(v!.xyz, v!.xyz), 0.5), mul(p!.y, 9.8)))] as const;
})[0];
export const checks: Check[] = [{
  name: "the rope's energy stays inside its first second's band",
  run: (probe: Probe) => {
    const es = Array.from({ length: 81 }, (_, k) => probe.scalar(energy, k * 0.1));
    const lo = Math.min(...es.slice(0, 10)), hi = Math.max(...es.slice(0, 10)), worst = Math.max(...es);
    const limit = hi + 0.2 * (hi - lo + 1);
    return { name: "rope energy", pass: worst <= limit, value: worst, measured: `most ${worst.toFixed(2)}`, limit: `<= ${limit.toFixed(2)}` };
  },
}];

export default scene({
  ground: plane, things: [cord], style: toon(), sun: [-0.4, 0.8, 0.45], period: 8,
  cameras: { whole: { kind: "perspective", eye: [0, 3.2, 10], target: [0, 2.8, 0], fovY: 0.6 } },
});
