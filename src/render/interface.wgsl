struct Frame { vp: mat4x4f, inv: mat4x4f, eye: vec4f, forward: vec4f, viewport: vec4f, time: vec4f, sun: vec4f }
@group(0) @binding(0) var<uniform> frame: Frame;
struct Row { refs: vec4u, origin: vec3f, id: u32, worlds: u32, ink: f32, pad: vec2u }
@group(1) @binding(0) var<storage,read> rows: array<Row>;
@group(1) @binding(1) var<storage,read> words: array<u32>;
struct Ray { o: vec3f, d: vec3f, t0: f32, t1: f32, w: vec2f }
struct Hit { t: f32, n: vec3f, uv: vec2f }
struct Box { lo: vec3f, hi: vec3f }
struct Point { p: vec3f, n: vec3f, uv: vec2f, t: f32, thing: u32 }
const miss = Hit(-1,vec3f(0),vec2f(0));
fn unproject(px: vec2f, z: f32) -> vec3f {
  let h = frame.inv * vec4f(px * frame.viewport.zw * vec2f(2,-2) + vec2f(-1,1), z, 1);
  return h.xyz/h.w;
}
fn camera_ray(px: vec2f) -> Ray {
  let near = unproject(px,0);
  let eye = select(near,frame.eye.xyz,frame.eye.w!=0);
  let dir = select(frame.forward.xyz,normalize(near-eye),frame.eye.w!=0);
  let scale = 2.0 / (frame.viewport.y * length(vec3f(frame.vp[0].y,frame.vp[1].y,frame.vp[2].y)));
  return Ray(eye,dir,0,1e30,select(vec2f(scale,0),vec2f(0,scale*dot(frame.forward.xyz,dir)),frame.eye.w!=0));
}
fn local(r: Ray, pos: vec3f) -> Ray { return Ray(r.o-pos,r.d,r.t0,r.t1,r.w); }
fn on_ray(r: Ray,t: f32) -> bool { return t>=r.t0 && t<r.t1; }
fn span(b: Box,r: Ray) -> vec2f {
  let parallel = r.d == vec3f(0);
  if any(parallel & ((r.o < b.lo) | (r.o > b.hi))) { return vec2f(1,-1); }
  let a = select((b.lo-r.o)/r.d,vec3f(-1e30),parallel);
  let z = select((b.hi-r.o)/r.d,vec3f(1e30),parallel);
  let lo=min(a,z); let hi=max(a,z);
  return vec2f(max(lo.x,max(lo.y,lo.z)),min(hi.x,min(hi.y,hi.z)));
}
