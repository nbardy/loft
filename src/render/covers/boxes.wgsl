struct Vertex { @builtin(position) px: vec4f, @location(0) @interpolate(flat) row: u32, @location(1) world:vec3f }
@vertex fn boxes(@builtin(vertex_index) k: u32,@builtin(instance_index) i: u32) -> Vertex {
  let corners=array<u32,36>(0,2,6,0,6,4,5,7,3,5,3,1,0,4,5,0,5,1,3,7,6,3,6,2,0,1,3,0,3,2,6,7,5,6,5,4);
  let c=corners[k]; let s=rows[i]; let b=read_bound(s.refs.x,s.refs.y);
  let p=origin(i) + select(b.lo,b.hi,(vec3u(c)&vec3u(1,2,4))!=vec3u(0));
  var clip=frame.vp*vec4f(p,1);
  // Only the proxy is depth-clamped. The fragment writes the actual surface depth.
  clip.z=clamp(clip.z,0,clip.w);
  return Vertex(clip,i,p);
}
struct Record { @location(0) id: vec4u, @builtin(frag_depth) depth: f32, @builtin(sample_mask) mask: u32 }
@fragment fn record(v: Vertex) -> Record {
  let s=rows[v.row]; if (s.worlds & 1u)==0u {discard;}
  let d=normalize(mix(frame.forward.xyz,v.world-frame.eye.xyz,frame.eye.w));
  let o=v.world-d*dot(v.world-frame.eye.xyz,d);
  let scale=2.0/(frame.viewport.y*length(vec3f(frame.vp[0].y,frame.vp[1].y,frame.vp[2].y)));
  let r=Ray(o,d,0,1e30,select(vec2f(scale,0),vec2f(0,scale*dot(frame.forward.xyz,d)),frame.eye.w!=0));
  let ray=local(r,origin(v.row));
  let h=read_hit(s.refs.x,s.refs.y,ray);
  var t=h.t; var mask=15u; var ink=0u;
  if t<0 {
    let g=read_gap(s.refs.x,s.refs.y,ray);
    t=g.x; if !on_ray(ray,t) {discard;}
    let width=max(1e-10,r.w.x+t*r.w.y);
    let edge=select(0.3,clamp(s.ink/width,1.5,4),s.ink>0);
    let count=u32(clamp(round(4*(1+edge-g.y/width)),0,4));
    if count==0u {discard;}
    let bits=(1u<<count)-1u;
    let rot=u32(fract(52.9829189*fract(dot(floor(v.px.xy),vec2f(0.06711056,0.00583715))))*4)&3u;
    mask=((bits<<rot)|(bits>>(4u-rot)))&15u;
    ink=select(0u,32768u,s.ink>0);
  }
  let p=frame.vp*vec4f(r.o+t*r.d,1); let z=p.z/p.w;
  if z<0 || z>1 {discard;}
  let id=v.row+1u;
  return Record(vec4u(id&65535u,(id>>16u)|ink,0,0),z,mask);
}
