// Warm light catches a fan of red clay behind a boot. The coarse dust and fine grains share their flight equation.
import {
  add, blob, cos, design, draw, exp, hash22, lamp, max, medium, min, mix, mul, plane, scene, sin, smoothstep,
  noise3, sub, time, toon, vec2, vec3, type F, type In, type P3,
} from "../src/index.ts";
import { sdCapsule3, sdRoundBox } from "../src/lib/sdf3.ts";
import { smin } from "../src/lib/util.ts";

function flight(i: In<"f32">, t: F) {
  const h = hash22(vec2(i, 31)), k = hash22(vec2(i, 73));
  const age = max(0, sub(t, mul(k.x, 0.19)));
  const angle = mix(-1.25, 0.93, h.x);
  const reach = mul(mix(0.7, 4.6, h.y), sub(1, exp(mul(age, -2.2))));
  const rise = mul(mix(0.7, 4.3, k.y), add(0.60, mul(0.40, sin(mul(h.x, Math.PI)))));
  const height = sub(mul(rise, sub(1, exp(mul(age, -2.8)))), mul(mul(age, age), 1.65));
  const edge = (u: F) => smoothstep(0, 0.19, mul(u, sub(1, u)));
  const taper = mul(edge(h.x), mul(edge(h.y), edge(k.y)));
  return { at: vec3(add(-0.5, mul(cos(angle), reach)), add(0.06, height), mul(sin(angle), reach)),
    fade: mul(taper, mul(smoothstep(0, 0.075, age), sub(1, smoothstep(0.72, 1.4, age)))), h: h.y, k: k.y };
}

function burst(count: number) {
  // Pick at indices spread across the same sequence the grains use, with a larger radius and slower fade.
  const dust = medium("clay", { puffs: Array.from({ length: 12 }, (_, i) => {
    const q = flight(i * 7919, mul(time, 0.78));
    const r = add(0.17, mul(smoothstep(0, 1.2, time), 0.28));
    return blob({ centre: q.at, radii: vec3(mul(r, 1.4), r, r), density: mul(q.fade, mix(0.7, 2.3, q.h)),
      colour: mix(vec3(0.27, 0.066, 0.027), vec3(0.58, 0.22, 0.091), q.k) });
  }) });
  const cloud = draw.density("dust", dust, {
    bounds: { min: [-2, 0, -5.5], max: [6, 4.8, 5.5] },
    sun: [0.85, 0.65, 0.52], sky: 0.3, forward: 0.42, shadowing: 0.75,
  });
  const grains = draw.points("grains", count, (i) => {
    const q = flight(i, time);
    const size = hash22(vec2(i, 111));
    const coarse = mul(size.x, mul(size.x, size.x));
    return {
      at: q.at,
      radius: mix(0.0019, 0.012, coarse),
      colour: mix(vec3(0.55, 0.18, 0.063), vec3(1.0, 0.56, 0.25), size.y),
      opacity: mul(mix(0.18, 0.65, coarse), q.fade),
    };
  }, { veil: cloud, stretch: 0.004 });

  const heel = vec3(-0.88, add(0.46, mul(smoothstep(0, 1.15, time), 1.55)), 0);
  const local = (p: P3): P3 => {
    const q = sub(p, heel);
    return vec3(add(mul(q.x, 0.896), mul(q.y, 0.444)), sub(mul(q.y, 0.896), mul(q.x, 0.444)), q.z);
  };
  const laces = (q: P3) => [0.16, 0.29, 0.42, 0.55].map((x) =>
    sdCapsule3(q, [x - 0.025, 0.157, -0.21], [x + 0.025, 0.157, 0.21], 0.022)).reduce((a, b) => min(a, b));
  const boot = draw.field("boot", (p) => {
    const q = local(p);
    const sole = sdRoundBox(sub(q, [0.32, -0.29, 0]), [0.99, 0.09, 0.39], 0.085);
    const upper = sdRoundBox(sub(q, [0.40, -0.08, 0]), [0.88, 0.23, 0.35], 0.20);
    const ankle = sdRoundBox(sub(q, [-0.28, 0.20, 0]), [0.38, 0.50, 0.35], 0.23);
    const cleats = [-0.43, 0.30, 0.95].map((x) =>
      sdRoundBox(vec3(sub(q.x, x), add(q.y, 0.40), sub(max(q.z, mul(q.z, -1)), 0.245)), [0.10, 0.12, 0.10], 0.035));
    return min(min(sole, smin(upper, ankle, 0.16)), min(laces(q), cleats.reduce((a, b) => min(a, b))));
  }, {
    bounds: { kind: "box", min: [-2.1, -0.2, -0.6], max: [0.8, 3.1, 0.6] }, steps: 48, outline: "silhouette", schedule: "trace",
    paint: ({ p }) => {
      const q = local(p);
      const leather = mix(vec3(0.24, 0.16, 0.09), vec3(0.061, 0.075, 0.084), smoothstep(-0.27, -0.16, q.y));
      return mix(vec3(0.34, 0.30, 0.24), leather, smoothstep(0.003, 0.012, laces(q)));
    },
  });

  const style = toon({ bands: 5, outline: 0.007, sky: { horizon: [0.18, 0.10, 0.085], zenith: [0.061, 0.041, 0.049] } });
  return scene({
    ground: plane, things: [boot, cloud, grains],
    style: { ...style, ground: ({ world, shadow }) => {
      const near = exp(mul(add(mul(world.x, world.x), mul(world.z, world.z)), -0.017));
      const grit = noise3(mul(world, 23));
      return mul(mix(vec3(0.11, 0.052, 0.039), vec3(0.31, 0.109, 0.058), near),
        mul(add(0.88, mul(grit, 0.12)), sub(1, mul(shadow, 0.55))));
    } },
    sun: { toward: [0.55, 0.38, -0.75], amount: [0.48, 0.32, 0.24] },
    lights: [lamp([1.2, 3.2, -3.1], { colour: [2.4, 1.32, 0.72], reach: 5.3 })],
    cameras: {
      game: { kind: "perspective", eye: [4.9, 3.05, 7.4], target: [0.55, 1.30, 0], fovY: 0.47 },
      side: { kind: "orthographic", eye: [0.7, 2.4, 10], target: [0.7, 1.22, 0], height: 4.2 },
    },
  });
}

const counts = { count: 100_000 };
const dustBurst = design({
  name: "spray_burst", numbers: counts, slots: {}, compare: ["game", "side"],
  variants: { million: { about: "one million grains in the same fan", numbers: { count: 1_000_000 } } },
  build: (_, { count }) => ({ scene: burst(count), checks: [] }),
});
export { dustBurst as design };
