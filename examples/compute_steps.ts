import { budget, compute, draw, plane, scene, toon, wgsl } from "../src/index.ts";

const sim = compute("dust", {
  when: { kind: "steps", start: "seed", count: 120, rate: 60, keep: 4 },
  writes: { state: { kind: "buffer", entries: 2048, lanes: 2 } },
  dispatch: [32],
  wgsl: wgsl`
const dt: f32 = 1.0 / 60.0;
fn hash(i: u32) -> f32 {
  var n = i * 747796405u + 2891336453u;
  n = ((n >> ((n >> 28u) + 4u)) ^ n) * 277803737u;
  return f32((n >> 22u) ^ n) / 4294967295.0;
}
@compute @workgroup_size(64) fn seed(@builtin(global_invocation_id) id: vec3<u32>) {
  let i = id.x * 2u;
  state[i] = vec4<f32>((hash(i)*2.0-1.0)*1.65, 1.15 + hash(i+1u)*1.6, (hash(i+2u)*2.0-1.0)*0.94, hash(i+3u));
  state[i+1u] = vec4<f32>((hash(i+4u)-0.5)*0.9, 0.0, (hash(i+5u)-0.5)*0.55, 0.0);
}
@compute @workgroup_size(64) fn settle(@builtin(global_invocation_id) id: vec3<u32>) {
  let i = id.x * 2u;
  let old = prev_state[i];
  var v = prev_state[i+1u].xyz * vec3<f32>(0.989, 1, 0.989) - vec3<f32>(0, 3.6 * dt, 0);
  var p = old.xyz + v * dt;
  var ground = 0.025;
  if (abs(p.x) < 0.46 && abs(p.z) < 0.39 && old.y >= 0.70) { ground = 0.70; }
  if (p.y < ground) {
    p.y = ground;
    v = vec3<f32>(v.x * 0.97, -v.y * 0.22, v.z * 0.97);
  }
  state[i] = vec4<f32>(p, old.w);
  state[i+1u] = vec4<f32>(v, 0);
}`,
});

// Each grain is a quad facing the camera, placed where the stepped state had it at this frame's time.
const dust = draw.shader("dust", wgsl`
struct Grain { @builtin(position) clip: vec4<f32>, @location(0) q: vec2<f32>, @location(1) shade: f32 };
@vertex fn vs(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> Grain {
  let p = ${sim.state}(f32(i), 0.0);
  let corner = array(vec2<f32>(-1,-1), vec2<f32>(1,-1), vec2<f32>(-1,1), vec2<f32>(-1,1), vec2<f32>(1,-1), vec2<f32>(1,1))[v];
  let h = frame.view_proj * vec4<f32>(p.xyz, 1);
  return Grain(vec4<f32>(h.xy + corner * frame.viewport.zw * h.w * (2.5 + 2.0 * p.w), h.zw), corner, p.w);
}
@fragment fn fs(in: Grain) -> @location(0) vec4<f32> {
  let a = 1.0 - smoothstep(0.1, 1.0, dot(in.q, in.q));
  return vec4<f32>(mix(vec3<f32>(0.64, 0.32, 0.10), vec3<f32>(1, 0.86, 0.47), in.shade), a * 0.82);
}`, { vertices: 6, instances: 2048, phase: "transparent", depthWrite: false, blend: "alpha", cull: "none" });

const obstacle = draw.shader("obstacle", wgsl`
struct Wall { @builtin(position) clip: vec4<f32>, @location(0) rgb: vec3<f32> };
@vertex fn vs(@builtin(vertex_index) i: u32) -> Wall {
  let q = array(vec2<f32>(0,0), vec2<f32>(1,0), vec2<f32>(0,1), vec2<f32>(0,1), vec2<f32>(1,0), vec2<f32>(1,1))[i%6u];
  let face = i/6u;
  let x = mix(-0.46, 0.46, q.x);
  let z = mix(-0.39, 0.39, q.y);
  var p = vec3<f32>(x, 0.70, z);
  var c = vec3<f32>(0.47, 0.40, 0.31);
  if (face == 1u) { p = vec3<f32>(x, q.y*0.70, 0.39); c *= 0.72; }
  if (face == 2u) { p = vec3<f32>(0.46, q.y*0.70, mix(-0.39, 0.39, q.x)); c *= 0.53; }
  return Wall(frame.view_proj * vec4<f32>(p, 1), c);
}
@fragment fn fs(in: Wall) -> @location(0) vec4<f32> { return vec4<f32>(in.rgb, 1); }`, { vertices: 18, cull: "none" });

export default scene({
  compute: [sim], ground: plane, things: [obstacle, dust], style: toon(), sun: [-1, 2, -1],
  cameras: { front: { kind: "perspective", eye: [3.0, 2.7, 5.1], target: [0, 0.80, 0], fovY: 35 * Math.PI / 180 } },
});

export const checks = [budget({ triangles: 20000 })];
