// Fixed fma evaluation after symmetric range reduction. At |x| <= pi/2,
// the first omitted sin term is bounded by (pi/2)^15 / 15! < 7e-10.
var<private> loft_tick_time: f32;
fn det_sin(a: f32) -> f32 {
  let tau = 6.283185307179586;
  let pi = 3.141592653589793;
  let r = fma(-floor(a / tau + 0.5), tau, a);
  let x = sign(r) * min(abs(r), pi - abs(r));
  let z = x*x;
  var p = fma(z, 1.0/6227020800.0, -1.0/39916800.0);
  p = fma(z,p,1.0/362880.0);
  p = fma(z,p,-1.0/5040.0);
  p = fma(z,p,1.0/120.0);
  p = fma(z,p,-1.0/6.0);
  return x*fma(z,p,1.0);
}
fn det_cos(a: f32) -> f32 { return det_sin(a+1.5707963267948966); }
fn det_atan2(y: f32,x: f32) -> f32 {
  let hi=max(abs(x),abs(y));
  if hi==0.0 {return 0.0;}
  let a=min(abs(x),abs(y))/hi;
  let shift=a>0.414213562373095;
  let r=select(a,(a-1.0)/(a+1.0),shift);
  let z=r*r;
  var p=fma(z,1.0/13.0,-1.0/11.0);
  p=fma(z,p,1.0/9.0);
  p=fma(z,p,-1.0/7.0);
  p=fma(z,p,1.0/5.0);
  p=fma(z,p,-1.0/3.0);
  var v=fma(r,fma(z,p,1.0),select(0.0,0.7853981633974483,shift));
  if abs(y)>abs(x) {v=1.5707963267948966-v;}
  if x<0.0 {v=3.141592653589793-v;}
  return select(v,-v,y<0.0);
}
