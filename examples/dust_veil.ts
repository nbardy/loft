// A burst of clay grains inside a dust cloud they kicked up: the grains (draw.points) are veiled by the cloud
// (draw.volume), so the far side of the fan reads through the dust. The maths is WGSL text; every slot takes it.
import { draw, pbr, plane, scene, wgsl } from "../src/index.ts";

const fx = wgsl`
struct Flight { at: vec3f, fade: f32, h: f32 }
struct Grain { at: vec3f, radius: f32, colour: vec3f, opacity: f32 }
fn edge(u: f32) -> f32 { return smoothstep(0.0, 0.19, u * (1.0 - u)); }
fn flight(i: f32, t: f32) -> Flight {
  let h = hash22(vec2(i, 31.0));
  let k = hash22(vec2(i, 73.0));
  let age = max(0.0, t - 0.19 * k.x);
  let angle = mix(-1.25, 0.93, h.x);
  let reach = mix(0.7, 4.6, h.y) * (1.0 - exp(-2.2 * age));
  let height = mix(0.7, 4.3, k.y) * (1.0 - exp(-2.8 * age)) - 1.65 * age * age;
  let fade = edge(h.x) * edge(h.y) * smoothstep(0.0, 0.075, age) * (1.0 - smoothstep(0.72, 1.4, age));
  return Flight(vec3(cos(angle) * reach - 0.5, height + 0.06, sin(angle) * reach), fade, h.y);
}
fn grain(i: f32) -> Grain {
  let q = flight(i, time);
  let s = hash22(vec2(i, 111.0));
  let coarse = s.x * s.x * s.x;
  return Grain(q.at, mix(0.002, 0.012, coarse), mix(vec3(0.55, 0.18, 0.06), vec3(1.0, 0.56, 0.25), s.y),
               mix(0.18, 0.65, coarse) * q.fade);
}
fn dust(p: vec3f) -> f32 {
  let core = flight(7.0, 0.8 * time).at;
  let q = (p - core) / vec3(2.2, 1.2, 1.6);
  return 1.8 * (1.0 - smoothstep(0.5, 1.0, length(q))) * smoothstep(-0.1, 0.5, noise3(p * 1.7 - vec3(0.4 * time, 0.0, 0.0)));
}`;

const cloud = draw.volume("dust", fx.dust!, { bounds: { min: [-2, 0, -5], max: [6, 5, 5] }, steps: 64, g: 0.6, albedo: [0.6, 0.25, 0.1] });
const grains = draw.points("grains", 100_000, fx.grain!, { stretch: 0.004, veil: cloud });

export default scene({
  ground: plane, things: [cloud, grains], sun: { toward: [0.55, 0.38, -0.75], amount: [2.2, 1.8, 1.5] },
  style: pbr(),
  cameras: { game: { kind: "perspective", eye: [4.9, 3, 7.4], target: [0.5, 1.3, 0], fovY: 0.47 } },
});
