// Store the last wrapped bearing, its continuous lift and the yaw the support allowed.
struct Gaze { bearing:f32, unwrapped:f32, yaw:f32, neck:f32 }
fn follow_gaze(last:Gaze,bearing:f32,dt:f32,range:f32,tc:f32,supported:bool)->Gaze {
  let delta=bearing-last.bearing;
  let unwrapped=last.unwrapped+atan2(sin(delta),cos(delta));
  let strain=last.unwrapped-last.yaw;
  let turn=sign(strain)*max(0.0,min(abs(strain),range)-0.6*range)/tc;
  let yaw=last.yaw+select(0.0,turn*dt,supported);
  return Gaze(bearing,unwrapped,yaw,clamp(unwrapped-yaw,-range,range));
}
fn gaze_error(g:Gaze)->f32 {return g.unwrapped-g.yaw-g.neck;}
