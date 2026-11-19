fn arch(u:f32,urgency:f32)->f32 {return 16.0*u*u*(1.0-u)*(1.0-u);}
fn swing_at(pin:Pin,u:f32)->vec3f{
  let m=u*u*u*(10.0+u*(-15.0+6.0*u));
  return mix(pin.origin.xyz,pin.aim.xyz,m)+vec3f(0,pin.shape.x*arch(u,pin.shape.w),0);
}
fn lift_pin(pin:ptr<function,Pin>,home:vec2f,v:vec2f,urgency:f32){
  let aim=home+v*0.25;let stride=distance(aim,(*pin).at.xz);
  let duration=clamp(0.30+0.03*stride-0.30*urgency,0.12,0.40);
  let lift=clamp(0.70+0.25*stride-1.20*urgency,0.25,1.60);
  (*pin).origin=vec4f((*pin).at.xyz,0);
  (*pin).at.w=0;
  let landing=aim+v*duration;
  (*pin).aim=vec4f(landing.x,0,landing.y,duration);
  (*pin).shape=vec4f(lift,(*pin).shape.y+1.0,0,urgency);
}
fn advance_pin(pin:ptr<function,Pin>,home:vec2f,v:vec2f,dt:f32){
  let u=min(1.0,(*pin).at.w+dt/(*pin).aim.w);
  let landing=home+v*(0.25+(1.0-u)*(*pin).aim.w);
  let aim=mix((*pin).aim.xz,landing,1.0-smoothstep(0.5,0.85,u));
  (*pin).aim=vec4f(aim.x,0,aim.y,(*pin).aim.w);
  if u>=1.0 {(*pin).at=vec4f((*pin).aim.xyz,-1);(*pin).origin.w=0.0;}
  else {(*pin).at=vec4f(swing_at(*pin,u),u);}
}
