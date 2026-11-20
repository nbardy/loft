fn G1(c:f32,a2:f32)->f32 {return 2*c/(c+sqrt(a2+(1-a2)*c*c));}
fn ggx(rough:f32,wo:vec3f,wi:vec3f)->f32 {
  if wo.z<=0 || wi.z<=0 {return 0;}
  let h=normalize(wo+wi);let a2=pow(clamp(rough,0.02,1),4);
  let q=(a2-1)*h.z*h.z+1;
  return a2*G1(wo.z,a2)*G1(wi.z,a2)/(3.141592653589793*q*q*4*wo.z);
}
