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

// atan2 reconstructs acos without a native transcendental.
fn det_acos(x: f32) -> f32 {
  return det_atan2(sqrt((1.0 - x) * (1.0 + x)), x);
}
// Reduce exp to |r| <= ln(2)/2, then use a fixed degree-seven polynomial.
fn det_exp(x: f32) -> f32 {
  if x < -103.97208 { return 0.0; }
  if x > 88.72283 { return bitcast<f32>(0x7f800000u); }
  let k = clamp(i32(floor(fma(x, 1.4426950408889634, 0.5))), -126, 127);
  let r = fma(f32(-k), 0.6931471805599453, x);
  if r < -0.346574 {
    return bitcast<f32>(1u << 23u) * det_exp_subnormal(r);
  }
  return bitcast<f32>(u32(k + 127) << 23u) * det_exp_poly(r);
}
fn det_exp_poly(r: f32) -> f32 {
  var p = fma(r, 1.0/5040.0, 1.0/720.0);
  p = fma(r, p, 1.0/120.0);
  p = fma(r, p, 1.0/24.0);
  p = fma(r, p, 1.0/6.0);
  p = fma(r, p, 0.5);
  return fma(r*r, p, 1.0+r);
}
fn det_exp_subnormal(x: f32) -> f32 {
  let k = i32(floor(fma(x, 1.4426950408889634, 0.5)));
  return det_exp_poly(fma(f32(-k), 0.6931471805599453, x)) * bitcast<f32>(u32(k + 127) << 23u);
}
// Decompose x = 2^k*m. For m in [1, 2), z=(m-1)/(m+1) <= 1/3.
fn det_log(x: f32) -> f32 {
  if x == 0.0 { return bitcast<f32>(0xff800000u); }
  if x < 0.0 { return bitcast<f32>(0x7fc00000u); }
  if x == bitcast<f32>(0x7f800000u) { return x; }
  let small = x < bitcast<f32>(1u << 23u);
  let bits = bitcast<u32>(select(x, x * 8388608.0, small));
  let k = i32(bits >> 23u) - select(127, 150, small);
  let m = bitcast<f32>((bits & 0x7fffffu) | 0x3f800000u);
  let z = (m - 1.0) / (m + 1.0);
  let w = z*z;
  var p = fma(w, 1.0/13.0, 1.0/11.0);
  p = fma(w, p, 1.0/9.0);
  p = fma(w, p, 1.0/7.0);
  p = fma(w, p, 1.0/5.0);
  p = fma(w, p, 1.0/3.0);
  return fma(f32(k), 0.6931471805599453, (2.0*z) * fma(w, p, 1.0));
}
