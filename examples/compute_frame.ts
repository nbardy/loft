import { always, compute, ge, time, draw, nothing, param, scene, spacing, surface, toon, vec2, vec3, sub, mul } from "../src/index.ts";

const palette = compute("palette", {
  when: { kind: "load" },
  writes: { tones: { kind: "buffer", entries: 2, lanes: 1 } },
  entry: "fill",
  dispatch: [1],
  wgsl: /* wgsl */ `
@compute @workgroup_size(1) fn fill() {
  tones[0] = vec4<f32>(0.13, 0.53, 0.58, 1);
  tones[1] = vec4<f32>(0.98, 0.55, 0.20, 1);
}`,
});

const pattern = compute("pattern", {
  when: { kind: "frame" },
  reads: { tones: palette.tones },
  values: { speed: param("speed", 2) },
  writes: { colour: { kind: "texture", size: [64, 64], format: "rgba8unorm" } },
  entry: "fill",
  dispatch: [8, 8],
  wgsl: /* wgsl */ `
@compute @workgroup_size(8, 8) fn fill(@builtin(global_invocation_id) id: vec3<u32>) {
  let p = vec2<f32>(id.xy) / 63.0 * 2.0 - 1.0;
  let a = atan2(p.y, p.x);
  let r = length(p);
  let spokes = 0.5 + 0.5 * cos(10.0 * a - 20.0 * r + loft_time() * speed());
  textureStore(colour, vec2<i32>(id.xy), mix(tones[0], tones[1], smoothstep(0.15, 0.95, spokes)));
}`,
});

const tile = surface((u, v) => vec3(sub(mul(u, 3), 1.5), mul(v, 3), 0), { u: ["free", "free"], v: "free" });
export default scene({
  compute: [pattern, palette], ground: nothing, style: toon(), sun: [0, 1, 2],
  things: [draw.surface("tile", tile, { grid: { u: spacing.even(2), v: spacing.even(2) }, sides: "two", outline: "none", paint: (u, v) => pattern.colour.sample(vec2(u, v)).xyz })],
  cameras: { front: { kind: "perspective", eye: [0, 1.5, 5.7], target: [0, 1.5, 0], fovY: 37 * Math.PI / 180 } },
});

export const checks = [always([time, 0, 2], ge(pattern.colour.sample(vec2(0.5, 0.5)).x, 0), { label: "paint red channel" })];
