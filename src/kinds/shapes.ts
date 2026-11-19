import { kind } from "../text/kind.ts";

export const washer = kind(`
struct Washer { r0:f32, r1:f32 }
fn bound(s:Washer)->Box {return Box(vec3f(-s.r1,-s.r1,-0.001),vec3f(s.r1,s.r1,0.001));}
fn hit(s:Washer,r:Ray)->Hit {
  if r.d.z==0 {return miss;}
  let t=-r.o.z/r.d.z; let p=r.o.xy+t*r.d.xy; let q=dot(p,p);
  if !on_ray(r,t) || q<s.r0*s.r0 || q>s.r1*s.r1 {return miss;}
  return Hit(t,vec3f(0,0,1),p);
}`);

export const sphere = kind(`
struct Sphere { radius:f32 }
fn bound(s:Sphere)->Box {return Box(vec3f(-s.radius),vec3f(s.radius));}
fn hit(s:Sphere,r:Ray)->Hit {
  let b=dot(r.o,r.d); let h=b*b-dot(r.o,r.o)+s.radius*s.radius;
  if h<0 {return miss;}
  let q=sqrt(h); let t=select(-b+q,-b-q,on_ray(r,-b-q));
  if !on_ray(r,t) {return miss;}
  let n=normalize(r.o+t*r.d);return Hit(t,n,n.xy);
}`);

// All four boundary events are candidates. A parallel ray contributes no cylinder roots.
export const bead = kind(`
struct Bead { radius:f32, bore:f32, margin:f32 }
fn bound(s:Bead)->Box {return Box(vec3f(-s.radius-s.margin),vec3f(s.radius+s.margin));}
fn hit(s:Bead,r:Ray)->Hit {
  let b=dot(r.o,r.d); let h=b*b-dot(r.o,r.o)+s.radius*s.radius;
  if h<0 {return miss;}
  var best=r.t1; var n=vec3f(0);
  let roots=vec2f(-b-sqrt(h),-b+sqrt(h));
  for(var i=0u;i<2u;i++) {
    let t=roots[i];let p=r.o+t*r.d;
    if t>=r.t0 && t<best && dot(p.xz,p.xz)>=s.bore*s.bore {best=t;n=normalize(p);}
  }
  let a=dot(r.d.xz,r.d.xz);let e=dot(r.o.xz,r.d.xz);
  let hc=e*e-a*(dot(r.o.xz,r.o.xz)-s.bore*s.bore);
  if a>0 && hc>=0 {
    let wall=(vec2f(-sqrt(hc),sqrt(hc))-e)/a;
    for(var i=0u;i<2u;i++) {
      let t=wall[i];let p=r.o+t*r.d;
      if t>=r.t0 && t<best && t>=roots.x && t<=roots.y {best=t;n=normalize(vec3f(-p.x,0,-p.z));}
    }
  }
  if best==r.t1 {return miss;} return Hit(best,n,n.xz);
}
fn gap(s:Bead,r:Ray)->vec2f {
  let t=max(r.t0,-dot(r.o,r.d));let p=r.o+t*r.d;
  return vec2f(t,max(1e-6,max(length(p)-s.radius,s.bore-length(p.xz))));
}`);

export const box = kind(`
struct Block { half:vec3f }
fn bound(s:Block)->Box {return Box(-s.half,s.half);}
fn hit(s:Block,r:Ray)->Hit {
  let v=span(bound(s),r); if v.x>v.y {return miss;}
  let t=select(v.y,v.x,on_ray(r,v.x)); if !on_ray(r,t) {return miss;}
  let p=r.o+t*r.d; let q=abs(p)/s.half;
  let face=select(select(vec3f(0,0,1),vec3f(0,1,0),q.y>=q.z),vec3f(1,0,0),q.x>=max(q.y,q.z));
  return Hit(t,face*sign(p),p.xz);
}`);

export const plane = kind(`
struct Plane { extent:f32 }
fn bound(s:Plane)->Box {return Box(vec3f(-s.extent,-0.001,-s.extent),vec3f(s.extent,0.001,s.extent));}
fn hit(s:Plane,r:Ray)->Hit {
  if r.d.y==0 {return miss;}
  let t=-r.o.y/r.d.y;let p=r.o.xz+t*r.d.xz;
  if !on_ray(r,t) || any(abs(p)>vec2f(s.extent)) {return miss;}
  return Hit(t,vec3f(0,1,0),p);
}`);

export const capsule = kind(`
struct Capsule { radius:f32, halfHeight:f32 }
fn bound(s:Capsule)->Box {let e=vec3f(s.radius,s.radius+s.halfHeight,s.radius);return Box(-e,e);}
fn hit(s:Capsule,r:Ray)->Hit {
  var t=r.t1;var n=vec3f(0);
  let a=dot(r.d.xz,r.d.xz);let b=dot(r.d.xz,r.o.xz);
  let h=b*b-a*(dot(r.o.xz,r.o.xz)-s.radius*s.radius);
  if a>0 && h>=0 {
    let roots=(vec2f(-sqrt(h),sqrt(h))-b)/a;
    for(var i=0u;i<2u;i++) {let v=roots[i];let p=r.o+v*r.d;
      if v>=r.t0 && v<t && abs(p.y)<=s.halfHeight {t=v;n=normalize(vec3f(p.x,0,p.z));}
    }
  }
  for(var j=0u;j<2u;j++) {let side=2*f32(j)-1;let o=r.o-vec3f(0,side*s.halfHeight,0);
    let e=dot(o,r.d);let disc=e*e-dot(o,o)+s.radius*s.radius;
    if disc<0 {continue;}
    let roots=vec2f(-sqrt(disc),sqrt(disc))-e;
    for(var i=0u;i<2u;i++) {let v=roots[i];let p=o+v*r.d;
      if v>=r.t0 && v<t && p.y*side>=0 {t=v;n=normalize(p);}
    }
  }
  if t==r.t1 {return miss;} return Hit(t,n,n.xy);
}`);
