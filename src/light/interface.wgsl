const PI=3.141592653589793;
struct Scatter {wi:vec3f,weight:vec3f,pdf:f32,delta:bool}
struct Lit {wi:vec3f,L:vec3f,pdf:f32,p:vec3f}
struct Sigma {s:vec3f,t:vec3f}
fn medium(p:vec3f)->Sigma {return Sigma(vec3f(0),vec3f(0));}
fn material(x:Point)->vec2u {return rows[x.thing].refs.zw;}
fn eval(m:vec2u,wo:vec3f,wi:vec3f)->vec3f {return read_eval(m.x,m.y,wo,wi);}
fn emit(m:vec2u,wo:vec3f)->vec3f {return read_emit(m.x,m.y,wo);}
fn albedo(m:vec2u)->vec3f {return read_albedo(m.x,m.y);}
fn sample(m:vec2u,wo:vec3f,u:vec2f)->Scatter {return read_sample(m.x,m.y,wo,u);}
fn sample_eval(tag:u32,at:u32,wo:vec3f,u:vec2f)->Scatter {
  let wi=cosine(u);let pdf=wi.z/PI;
  return Scatter(wi,read_eval(tag,at,wo,wi)/pdf,pdf,false);
}
struct Emitter { pos:vec3f,radius:f32,L:vec3f,cdf:f32,row:u32,prob:f32,pad:vec2u }
@group(1) @binding(2) var<storage,read> emitters:array<Emitter>;
fn pick(u:f32)->u32 {
  var lo=0u;var hi=arrayLength(&emitters)-1u;
  while lo<hi {let mid=lo+(hi-lo)/2u;if u<emitters[mid].cdf {hi=mid;}else{lo=mid+1u;}}
  return lo;
}
fn light(i:u32,p:vec3f,u:vec2f)->Lit {
  let e=emitters[i];
  if e.radius==0 {return Lit(e.pos,e.L,e.prob,p+e.pos*1e5);}
  let v=e.pos-p;let l=length(v);
  // Stable solid angle of a sphere. Inside the emitter every direction hits its exit.
  let sin2=min(1,e.radius*e.radius/dot(v,v));
  let cap=select(sin2/(1+sqrt(1-sin2)),2.0,l<=e.radius);
  let z=1-u.x*cap;let phi=2*PI*u.y;let w=sqrt(max(0,1-z*z));
  let axis=select(vec3f(0,0,1),v/max(l,1e-20),l>0);
  let wi=basis(axis)*vec3f(w*cos(phi),w*sin(phi),z);
  let ray=Ray(p,wi,1e-4,1e30,vec2f(0));let h=intersect(e.row,ray);
  if h.t<0 {return Lit(wi,vec3f(0),1,p);}
  // End the occlusion segment before the sampled emitter.
  return Lit(wi,e.L,e.prob/(2*PI*cap),p+wi*h.t);
}
