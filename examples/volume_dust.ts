// A backlit bank of windblown dust, and a fog of recursively folded seams.
import {
  abs, add, design as makeDesign, div, dot, draw, lamp, length, mix, mul, noise3, nothing, scene, sin,
  smoothstep, sub, time, toon, vec3, type P3, type VolumeOptions,
} from "../src/index.ts";

const bounds = { min: [-3.7, 0.15, -2.2], max: [3.7, 3.9, 2.2] } as const;
const local = (p: P3) => sub(p, vec3(0, 2, 0));

const dust = (p: P3) => {
  const q = local(p);
  const drift = sub(q, vec3(mul(time, 0.35), mul(time, 0.08), 0));
  const shape = length(div(q, [3.65, 1.85, 2.18]));
  const billow = noise3(mul(drift, [0.75, 1.2, 1.0]));
  const wisps = noise3(mul(drift, [2.7, 4.9, 3.8]));
  const body = sub(1, smoothstep(0.58, 0.99, shape));
  return mul(2.1, body, smoothstep(-0.14, 0.48, add(billow, mul(wisps, 0.24))));
};

const folded = (p: P3) => {
  const q = div(local(p), [3.6, 1.85, 2.14]);
  let r = q;
  for (let i = 0; i < 4; i++) {
    const x = abs(r);
    r = sub(mul(vec3(add(x.y, mul(x.z, 0.38)), add(x.z, mul(x.x, 0.38)), add(x.x, mul(x.y, 0.38))), 1.62), [0.7, 0.62, 0.54]);
  }
  const seam = abs(add(dot(r, [0.7, -0.46, 0.55]), mul(sin(mul(time, 0.7)), 0.09)));
  return mul(1.2, sub(1, smoothstep(0.035, 0.20, seam)), sub(1, smoothstep(0.70, 0.99, length(q))));
};

const view = { kind: "perspective", eye: [0, 2.55, 12.5], target: [0, 2.0, 0], fovY: 0.49 } as const;
const look = { ...toon(), sky: () => vec3(0.028, 0.040, 0.068), clear: [0.028, 0.040, 0.068] as const };
const build = (field: "dust" | "folded", steps: number, shadow: NonNullable<VolumeOptions["shadow"]>) => scene({
  ground: nothing,
  things: [field === "dust"
    ? draw.volume("dust", dust, { bounds, steps, shadow, g: 0.64, albedo: [0.98, 0.72, 0.46] })
    : draw.volume("foldedFog", folded, {
      bounds, steps, shadow, g: 0.32,
      albedo: (p) => mix(vec3(0.27, 0.57, 0.96), vec3(0.74, 0.98, 0.93), smoothstep(0.7, 3.2, p.y)),
      emission: (p) => mul(vec3(0.045, 0.15, 0.24), sub(1, smoothstep(0.0, 1.25, abs(sub(p.y, 1.65))))),
    })],
  sun: { toward: [-0.26, 0.17, -1], amount: [3.6, 3.05, 2.55] },
  lights: field === "dust" ? [] : [lamp([0.9, 2.5, 0.7], { colour: [1.7, 5.4, 7.0], reach: 1.4 })],
  cameras: { game: view },
  style: look,
});

const unshadowed = { kind: "unshadowed" } as const;
const march = { kind: "march", steps: 6 } as const;
export const design = makeDesign({
  name: "volumeDust",
  numbers: { steps: 64 },
  slots: {
    field: { base: { about: "Windblown dust", kind: "dust" }, folded: { about: "Recursive mirror folds", kind: "folded" } } as const,
    shadow: { base: { about: "Six light-ray samples", value: march }, off: { about: "Unshadowed scattering", value: unshadowed } },
  },
  variants: {
    dust128: { about: "128 view steps, six shadow steps", numbers: { steps: 128 } },
    dust64Clear: { about: "64 view steps, no shadow march", pick: { shadow: "off" } },
    dust128Clear: { about: "128 view steps, no shadow march", pick: { shadow: "off" }, numbers: { steps: 128 } },
    folded: { about: "Folded fog with a blue lamp", pick: { field: "folded" }, numbers: { steps: 128 } },
  },
  compare: ["game"],
  build: ({ field, shadow }, { steps }) => ({
    scene: build(field.kind, steps, shadow.value),
    checks: [],
  }),
});

export default design.build(design.resolve({ variant: "base", picks: [] })).scene;
