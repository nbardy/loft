// One island. +Y is up. Each segment points down its local +Y axis.
// Plan is immutable. The eight slots are capacity; dims.w is the number in use.
struct Chain { root: vec4f, home: vec4f, lengths: vec4f }
struct Plan { dims: vec4f, laws: vec4f, chain: array<Chain,8> }
struct Pin { at: vec4f, origin: vec4f, aim: vec4f, shape: vec4f }
struct State {
  motion: vec4f, drive: vec4f, tilt: vec4f, spin: vec4f,
  support: vec4f, reach: vec4f, deck: vec4f, before: vec4f,
  old_tilt: vec4f, stamps: vec4f, pin: array<Pin,8>,
}
struct Frame { t:vec4f, q:vec4f }
struct Pose { joints:array<Frame,25>, margins:array<vec4f,8> }

fn rot(q:vec4f,v:vec3f)->vec3f {return v+2.0*cross(q.xyz,cross(q.xyz,v)+q.w*v);}
fn lean_q(a:vec2f)->vec4f {
  let h=a*0.5; let x=sin(h.y); let z=-sin(h.x); let cx=cos(h.y); let cz=cos(h.x);
  return vec4f(x*cz,-x*z,cx*z,cx*cz);
}
fn axis_frame(at:vec3f,end:vec3f)->Frame {
  let dir=normalize(end-at);
  // Use the better-conditioned hemisphere. Snapping a nearly vertical axis to -Y displaced the drawn site by millimetres.
  var q=vec4f(dir.z,0,-dir.x,1.0+dir.y);
  if dir.y<0.0 {q=vec4f(1.0-dir.y,dir.x,0,dir.z);}
  q=normalize(q);
  return Frame(vec4f(at,1),q);
}
fn root_at(b:ptr<function,Plan>,i:u32,pos:vec2f,angle:vec2f)->vec3f {
  return vec3f(pos.x,(*b).laws.x,pos.y)+rot(lean_q(angle),(*b).chain[i].root.xyz);
}
