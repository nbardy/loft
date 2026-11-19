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
  (*pin).origin.w=landing.x;
  (*pin).shape=vec4f(lift,landing.y,0,urgency);
}
fn advance_pin(pin:ptr<function,Pin>,home:vec2f,v:vec2f,dt:f32){
  let start=(*pin).at.w;let u=min(1.0,start+dt/(*pin).aim.w);
  let landing=home+v*(0.25+(1.0-u)*(*pin).aim.w);
  let aim=mix((*pin).aim.xz,landing,1.0-smoothstep(0.5,0.85,u));
  // A retarget changes the endpoint within this span. Preserve its start for fractional presentation.
  (*pin).origin.w=(*pin).aim.x;(*pin).shape.y=(*pin).aim.z;
  (*pin).aim=vec4f(aim.x,0,aim.y,(*pin).aim.w);
  if u>=1.0 {(*pin).at=vec4f((*pin).aim.xyz,-1);(*pin).origin.w=0.0;(*pin).shape.y=start;}
  else {(*pin).at=vec4f(swing_at(*pin,u),u);}
}

// xyz is the observed target in the support frame. A negative w denotes a hold.
fn present_pin(pin:Pin,f:f32,dt:f32)->vec4f {
  if pin.at.w<0.0 {
    if pin.origin.w>0.0 || pin.shape.x==0.0 {return pin.at;}
    // The last interval can be shorter than one tick. Retargeting has already stopped.
    let u=min(1.0,pin.shape.y+f*dt/pin.aim.w);
    return vec4f(swing_at(pin,u),select(u,-1.0,u>=1.0));
  }
  let u=mix(max(0.0,pin.at.w-dt/pin.aim.w),pin.at.w,f);
  var span=pin;
  let aim=mix(vec2f(pin.origin.w,pin.shape.y),pin.aim.xz,f);
  span.aim=vec4f(aim.x,pin.aim.y,aim.y,pin.aim.w);
  return vec4f(swing_at(span,u),u);
}
