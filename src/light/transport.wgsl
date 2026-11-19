fn transport(first:Point,dir:vec3f,px:vec2u,s:u32,depth:u32)->vec3f {
  var x=first;var wo=dir;var L=vec3f(0);var weight=vec3f(1);var delta=true;
  for(var b=0u;b<=depth;b++) {
    if x.t<0 { L+=weight*sky(-wo);break; }
    let m=material(x);let n=select(-x.n,x.n,dot(x.n,wo)>=0);
    let basisN=basis(n);let localWo=transpose(basisN)*wo;
    if delta || rows[x.thing].pad.x==0u {L+=weight*emit(m,localWo);}
    if b==depth {break;}
    let l=light(pick(random(px,s,b,0)),x.p,vec2f(random(px,s,b,1),random(px,s,b,2)));
    if any(l.L>vec3f(0)) && dot(l.wi,n)>0 {
      L+=weight*eval(m,localWo,transpose(basisN)*l.wi)*l.L*visible(x.p,l.p)/l.pdf;
    }
    let sc=sample(m,localWo,vec2f(random(px,s,b,3),random(px,s,b,4)));
    weight*=sc.weight;if all(weight==vec3f(0)) {break;}
    let wi=basisN*sc.wi;
    delta=sc.delta;wo=-wi;x=trace(Ray(x.p,wi,1e-4,1e30,vec2f(0)));
  }
  return L;
}
