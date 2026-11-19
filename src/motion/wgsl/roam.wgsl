fn step(s:ptr<function,State>,b:ptr<function,Plan>,go:vec4f,dt:f32){
  let n=u32((*b).dims.w);let height=(*b).laws.x;let tc=sqrt(height/9.81);
  let c=(*s).motion.xy;let v=(*s).motion.zw;
  var down=0u;var strain:array<f32,8>;
  for(var j=0u;j<n;j++){
    if (*s).pin[j].at.w<0.0 {down|=1u<<j;}
    // Compare displacement in the rest frame first. Adding a small displacement to a large home
    // otherwise breaks the symmetric tie according to the ulp of the home coordinate.
    strain[j]=length(((*s).pin[j].at.xz-(*b).chain[j].home.xz)-(c+v*tc));
  }
  var h=hull(s,down);
  var a=(go.xy-v)/(0.5*tc);
  a*=min(1.0,(*b).laws.y*9.81/max(length(a),1e-10));
  let cop=contact(&h,c-a*(height/9.81)).closest;
  a=select((c-cop)*(9.81/height),vec2f(0),down==0u);
  let weights=shares(s,down,cop);
  let urgency=clamp((distance(c+v*tc,centroid(s,down))-0.5)*0.5,0.0,1.0);
  var waiting=down;var support=down;var blocks=0u;
  for(var rank=0u;rank<n;rank++){
    var best=(*b).laws.z;var i=8u;
    for(var j=0u;j<n;j++){
      if (waiting & (1u<<j))!=0u && strain[j]>best {best=strain[j];i=j;}
    }
    if i==8u {break;}
    let bit=1u<<i;waiting &= ~bit;
    if (*s).pin[i].origin.w<0.12 || (u32((*b).chain[i].lengths.w) & ~support)!=0u || weights[i]>1.1/f32(countOneBits(down)) {continue;}
    var left=hull(s,support & ~bit);
    if contact(&left,cop).margin<(*b).laws.w {blocks++;continue;}
    lift_pin(&(*s).pin[i],(*b).chain[i].home.xz+c,v,urgency);
    support &= ~bit;
  }
  for(var i=0u;i<n;i++){
    if (down & (1u<<i))==0u {advance_pin(&(*s).pin[i],(*b).chain[i].home.xz+c,v,dt);}
    else if (support & (1u<<i))!=0u {(*s).pin[i].origin.w+=dt;}
  }
  let omega=12.5663706;let angle=(*s).tilt.xy;
  let spin=(*s).spin.xy+(omega*omega*(atan(a/9.81)-angle)-1.4*omega*(*s).spin.xy)*dt;
  (*s).before=(*s).motion;(*s).old_tilt=(*s).tilt;
  (*s).motion=vec4f(c+dt*(v+0.5*dt*a),v+dt*a);
  (*s).drive=vec4f(a,cop);(*s).tilt=vec4f(angle+spin*dt,0,0);(*s).spin=vec4f(spin,0,0);
  var final_mask=0u;var leash=0u;var slack=1e10;
  for(var i=0u;i<n;i++){if (*s).pin[i].at.w<0.0 {
    final_mask|=1u<<i;
    let root=root_at(b,i,(*s).motion.xy,(*s).tilt.xy);
    let delta=(*s).pin[i].at.xyz-root;let cap=0.97*dot((*b).chain[i].lengths.xyz,vec3f(1));
    slack=min(slack,cap-length(delta));
    if length(delta)>cap && length(delta.xz)>1e-6 {
      let offset=normalize(delta.xz)*(length(delta.xz)-sqrt(max(0.0,cap*cap-delta.y*delta.y)));
      (*s).motion=vec4f((*s).motion.xy+offset,(*s).motion.zw);leash++;
    }
  }}
  var final_h=hull(s,final_mask);
  let w=shares(s,final_mask,cop);var least=1.0;
  for(var i=0u;i<n;i++){(*s).pin[i].shape.z=w[i];if (final_mask & (1u<<i))!=0u {least=min(least,w[i]);}}
  (*s).support=vec4f(least,contact(&final_h,cop).margin,f32(countOneBits(final_mask)),f32(blocks));
  (*s).reach=vec4f(slack,f32(leash),0,0);(*s).stamps.x+=1;
}
