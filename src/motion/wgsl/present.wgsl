// Distal axis points from ankle to pad. Pick its rest-relative direction first, then solve the proximal
// two-link triangle. Limit the distal angle by the reachable cone when a chain approaches extension.
fn observed(f:Frame,l:f32)->vec3f {return f.t.xyz+rot(f.q,vec3f(0,l,0));}
fn present(s:ptr<function,State>,b:ptr<function,Plan>,fraction:f32,dt:f32)->Pose {
  var p:Pose;let v=(*s).motion.zw;let t=fraction*dt;
  let c=(*s).motion.xy+t*(v+0.5*t*(*s).drive.xy);
  let angle=(*s).tilt.xy+t*(*s).spin.xy;
  p.joints[0]=Frame(vec4f(c.x,(*b).laws.x,c.y,1),lean_q(angle));
  for(var i=0u;i<u32((*b).dims.w);i++){
    let chain=(*b).chain[i];let pin=(*s).pin[i];
    var pad=pin.at.xyz;
    if pin.at.w>=0.0 {pad=swing_at(pin,min(1.0,pin.at.w+t/pin.aim.w));}
    let root=root_at(b,i,c,angle);let lens=chain.lengths.xyz;
    let axis=normalize(pad-root);let dist=distance(pad,root);
    var pole=vec3f(0,1,0)-axis*axis.y;
    if dot(pole,pole)<1e-8 {pole=vec3f(1,0,0)-axis*axis.x;}
    pole=normalize(pole);
    let ankle_cos=clamp(max(0.55,(dist*dist+lens.z*lens.z-(lens.x+lens.y)*(lens.x+lens.y))/(2.0*dist*lens.z)),-1.0,1.0);
    let ankle=pad-lens.z*(axis*ankle_cos-pole*sqrt(1.0-ankle_cos*ankle_cos));
    let span=distance(root,ankle);let line=normalize(ankle-root);
    let along=clamp((lens.x*lens.x-lens.y*lens.y+span*span)/(2.0*span),-lens.x,lens.x);
    let knee=root+line*along+normalize(pole-line*dot(pole,line))*sqrt(max(0.0,lens.x*lens.x-along*along));
    let k=1u+3u*i;
    p.joints[k]=axis_frame(root,knee);p.joints[k+1]=axis_frame(knee,ankle);
    var distal=axis_frame(ankle,pad);
    p.joints[k+2]=distal;
    let site=observed(distal,lens.z);
    p.margins[i]=vec4f(0.0001-distance(site,pin.at.xyz),pin.at.w,pin.shape.z,site.y);
  }
  return p;
}
