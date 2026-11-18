import { budget, compute, drawSpec, nothing, scene, toon, type Drawable } from "../src/index.ts";

const volume = compute("volume", {
  when: { kind: "build" },
  writes: { sdf: { kind: "texture", size: [48, 48, 48], format: "r32float" } },
  entry: "fill",
  dispatch: [12, 12, 12],
  wgsl: /* wgsl */ `
@compute @workgroup_size(4, 4, 4)
fn fill(@builtin(global_invocation_id) id: vec3<u32>) {
  let p = vec3<f32>(id) / 47.0 * 2.0 - 1.0;
  let angle = atan2(p.z, p.x);
  let r = length(p.xz) - (0.51 + 0.07 * cos(5.0 * angle));
  let d = length(vec2<f32>(r, p.y + 0.10 * sin(3.0 * angle))) - 0.19;
  textureStore(sdf, vec3<i32>(id), vec4<f32>(d, 0, 0, 1));
}`,
});

const ring: Drawable = {
  name: "ring",
  skeletons: [],
  forms: [],
  draws: () => ({
    shaders: { ring: { libs: [], code: /* wgsl */ `
struct RingVertex { @builtin(position) clip: vec4<f32>, @location(0) p: vec2<f32> };
@vertex fn vs(@builtin(vertex_index) i: u32) -> RingVertex {
  let p = array(vec2<f32>(-1,-1), vec2<f32>(3,-1), vec2<f32>(-1,3))[i];
  return RingVertex(vec4<f32>(p, 0.4, 1), p);
}
fn sdf_at(p: vec3<f32>) -> f32 {
  let q = vec3<i32>(round(clamp(p * 0.5 + 0.5, vec3<f32>(0), vec3<f32>(1)) * 47.0));
  return textureLoad(${volume.sdf.wgsl}, q, 0).x;
}
@fragment fn fs(in: RingVertex) -> @location(0) vec4<f32> {
  let uv = in.p * vec2<f32>(frame.viewport.x / frame.viewport.y, 1);
  if (any(abs(uv) > vec2<f32>(0.9))) { discard; }
  let a = vec3<f32>(uv.x, uv.y * 0.78 + 0.63, uv.y * 0.63 - 0.78);
  let direction = vec3<f32>(0, -0.63, 0.78);
  var p = a;
  for (var i = 0; i < 64; i++) {
    let d = sdf_at(p);
    if (d < 0.015) {
      let dx = vec3<f32>(0.043, 0, 0);
      let dy = dx.yxz;
      let dz = dx.yzx;
      let n = normalize(vec3<f32>(sdf_at(p+dx)-sdf_at(p-dx), sdf_at(p+dy)-sdf_at(p-dy), sdf_at(p+dz)-sdf_at(p-dz)));
      let key = max(dot(n, normalize(vec3<f32>(-0.5, 0.9, -0.8))), 0.0);
      return vec4<f32>(vec3<f32>(0.12, 0.33, 0.34) + key * vec3<f32>(0.46, 0.56, 0.43), 1);
    }
    p += direction * max(d * 0.82, 0.015);
    if (distance(p, a) > 2.0) { break; }
  }
  discard;
  return vec4<f32>(0);
}` } },
    draws: [drawSpec({ label: "ring", shader: "ring", vertices: 3 })],
  }),
};

export default scene({
  compute: [volume], ground: nothing, things: [ring], style: toon(), sun: [-1, 2, -1],
  cameras: { front: { kind: "perspective", eye: [0, 2, 5], target: [0, 0, 0], fovY: 38 * Math.PI / 180 } },
});

export const checks = [budget({ triangles: 20000 })];
