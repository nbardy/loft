// Position, velocity and acceleration of the least-jerk quintic. A span begins at
// rest and ends at the requested displacement and speed, with zero endpoint acceleration.
fn plan_at(t:f32,T:f32,D:f32,end_speed:f32)->vec3f {
  let u=clamp(t/T,0.0,1.0); let V=end_speed*T;
  let a=10.0*D-4.0*V; let b=7.0*V-15.0*D; let c=6.0*D-3.0*V;
  return vec3f(u*u*u*(a+u*(b+u*c)),
               u*u*(3.0*a+u*(4.0*b+5.0*u*c))/T,
               u*(6.0*a+u*(12.0*b+20.0*u*c))/(T*T));
}
// strength is maximum *net* upward acceleration in units of g. The caller owns
// the landing and its next plan. The parabola remains defined down to any deck.
fn hop_stance(speed:f32,g:f32,strength:f32)->f32 {
  let u=(7.0-sqrt(19.0))/15.0;
  let v=(7.0+sqrt(19.0))/15.0;
  let pull=-(u*(-24.0+u*(84.0-60.0*u)));
  let push=v*(-24.0+v*(84.0-60.0*v));
  return max(pull,push/strength)*speed/g;
}
fn hop_at(t:f32,speed:f32,g:f32,strength:f32)->vec3f {
  let T=hop_stance(speed,g,strength);
  if t<T {return plan_at(t,T,0.0,speed);}
  let air=t-T;
  return vec3f(air*(speed-0.5*g*air),speed-g*air,-g);
}
