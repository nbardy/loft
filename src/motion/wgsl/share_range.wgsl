// Extremal feasible loads have at most three non-zero weights (sum, x and z).
// Include singleton and segment solutions for coincident or collinear anchors.
// An empty interval reports that the pressure point cannot be supported.
fn share_range(s:ptr<function,State>,mask:u32,q:vec2f,site:u32)->vec2f {
  var low=1.0; var high=0.0;
  for(var i=0u;i<8u;i++){
    if (mask & (1u<<i))==0u {continue;}
    let a=(*s).pin[i].at.xz;
    if distance(q,a)<1e-6 {
      let w=f32(i==site); low=min(low,w); high=max(high,w);
    }
    for(var j=i+1u;j<8u;j++){
      if (mask & (1u<<j))==0u {continue;}
      let ab=(*s).pin[j].at.xz-a;
      let aq=q-a; let length2=dot(ab,ab);
      if length2<1e-12 {continue;}
      let v=dot(aq,ab)/length2;
      if distance(aq,v*ab)<1e-6 && v>=0.0 && v<=1.0 {
        let w=f32(i==site)*(1.0-v)+f32(j==site)*v; low=min(low,w); high=max(high,w);
      }
      for(var k=j+1u;k<8u;k++){
        if (mask & (1u<<k))==0u {continue;}
        let ac=(*s).pin[k].at.xz-a;
        let area=wedge(ab,ac);
        if abs(area)<1e-7*length(ab)*length(ac) {continue;}
        if area==0.0 {continue;}
        let y=wedge(aq,ac)/area; let z=wedge(ab,aq)/area; let x=1.0-y-z;
        if min(x,min(y,z)) < -1e-6 {continue;}
        let w=clamp(f32(i==site)*x+f32(j==site)*y+f32(k==site)*z,0.0,1.0);
        low=min(low,w); high=max(high,w);
      }
    }
  }
  return vec2f(low,high);
}
