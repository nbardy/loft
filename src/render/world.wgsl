fn intersect(i: u32,r: Ray) -> Hit {
  let s=rows[i]; let ray=local(r,origin(i));
  let bounds=read_bound(s.refs.x,s.refs.y);
  let limits=span(bounds,ray);
  if limits.x>limits.y || limits.y<r.t0 || limits.x>=r.t1 {return miss;}
  return read_hit(s.refs.x,s.refs.y,ray);
}
fn trace_world(r: Ray,world: u32) -> Point {
  var ray=r; var x=Point(vec3f(0),vec3f(0),vec2f(0),-1,0u);
  for(var i=0u;i<arrayLength(&rows);i++){
    if ((rows[i].worlds & world)==0u) {continue;}
    let h=intersect(i,ray);
    if h.t>=0 {ray.t1=h.t;x=Point(r.o+h.t*r.d,h.n,h.uv,h.t,i);}
  }
  return x;
}
fn trace(r: Ray) -> Point {return trace_world(r,1u);}
fn visible_world(a: vec3f,b: vec3f,world: u32) -> vec3f {
  let delta=b-a; let dist=length(delta);
  let ray=Ray(a,delta/dist,1e-4,dist-1e-4,vec2f(0));
  for(var i=0u;i<arrayLength(&rows);i++){
    if ((rows[i].worlds & world)!=0u && intersect(i,ray).t>=0) {return vec3f(0);}
  }
  return vec3f(1);
}
fn visible(a: vec3f,b: vec3f) -> vec3f {return visible_world(a,b,2u);}
