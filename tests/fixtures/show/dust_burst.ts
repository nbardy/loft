// The dust burst of docs/proposals/2026-11-18_authoring_api.md section 3, written in WGSL text: grains and a dust
// volume share one flight equation, a thin-film Fresnel for pbr, and one raw function (a u32 hash, outside the text
// subset) tinting the grains. `loft show` on it is lane L4's check (tests/show.gpu.test.ts): every statement derived
// from this text carries its line.
import { draw, pbr, plane, scene, wgsl } from "../../../src/index.ts";

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
fn tint(i: f32) -> f32 {
  var x = u32(i) * 747796405u + 2891336453u;
  x = ((x >> ((x >> 28u) + 4u)) ^ x) * 277803737u;
  return f32((x >> 22u) ^ x) / 4294967295.0;
}
fn grain(i: f32) -> Grain {
  let q = flight(i, time);
  let s = hash22(vec2(i, 111.0));
  let coarse = s.x * s.x * s.x;
  let colour = mix(vec3(0.55, 0.18, 0.06), vec3(1.0, 0.56, 0.25), s.y) * (0.85 + 0.3 * tint(i));
  return Grain(q.at, mix(0.002, 0.012, coarse), colour, mix(0.18, 0.65, coarse) * q.fade);
}
fn dust(p: vec3f) -> f32 {
  let core = flight(7.0, 0.8 * time).at;
  let q = (p - core) / vec3(2.2, 1.2, 1.6);
  return 1.8 * (1.0 - smoothstep(0.5, 1.0, length(q))) * smoothstep(-0.1, 0.5, noise3(p * 1.7 - vec3(0.4 * time, 0.0, 0.0)));
}
fn film(v_h: f32, f0: vec3f) -> vec3f {
  let cos_t = sqrt(1.0 - (1.0 - v_h * v_h) / 2.4336);
  let phase = 4.0 * 3.14159265 * 1.56 * 380.0 * cos_t / vec3(650.0, 530.0, 450.0);
  return f0 + 0.25 * (1.0 + cos(phase));
}`;

const cloud = draw.volume("dust", fx.dust!, { bounds: { min: [-2, 0, -5], max: [6, 5, 5] }, steps: 64, g: 0.6, albedo: [0.6, 0.25, 0.1] });
const grains = draw.points("grains", 100_000, fx.grain!, { stretch: 0.004 });

export default scene({
  ground: plane, things: [cloud, grains], sun: [0.55, 0.38, -0.75],
  style: pbr({ fresnel: fx.film }),
  cameras: { game: { kind: "perspective", eye: [4.9, 3, 7.4], target: [0.5, 1.3, 0], fovY: 0.47 } },
});
