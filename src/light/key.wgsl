// Counter-based PCG permutation, indexed by pixel, sample, bounce and dimension.
fn permute(x:u32)->u32 {
  let state=x*747796405u+2891336453u;
  let word=((state>>((state>>28u)+4u))^state)*277803737u;
  return (word>>22u)^word;
}
fn random(px:vec2u,s:u32,b:u32,d:u32)->f32 {
  return f32(permute(px.x ^ permute(px.y ^ permute(s ^ permute(b ^ permute(d)))))>>8u)*0.000000059604644775390625;
}
fn basis(n:vec3f)->mat3x3f {
  let a=select(vec3f(0,0,1),vec3f(0,1,0),abs(n.z)>0.9);
  let u=normalize(cross(a,n));
  return mat3x3f(u,cross(n,u),n);
}
fn cosine(u:vec2f)->vec3f {
  let a=2*PI*u.y;return vec3f(sqrt(u.x)*vec2f(cos(a),sin(a)),sqrt(1-u.x));
}
