struct MediumStack {
  top: u32, boundary: array<u32, 4>, ior: array<f32, 4>, absorb: array<f32, 4>,
}
struct MediumBend { ray: PathRay, stack: MediumStack, eta: f32, through: bool, valid: bool }
struct CausticBin { flux: atomic<u32> }
@group(2) @binding(4) var<storage, read_write> loft_local_bins: array<CausticBin>;

fn medium_air() -> MediumStack {
  return MediumStack(0u, array<u32, 4>(0xffffffffu,0u,0u,0u), array<f32,4>(1.0,0.0,0.0,0.0), array<f32,4>(0.0,0.0,0.0,0.0));
}
fn medium_interface(stack: MediumStack, h: PathHit, rd: vec3<f32>, optical: Optics, nm: f32, xi: f32) -> MediumBend {
  var next = stack;
  if h.entering {
    if stack.top == 3u { return MediumBend(PathRay(h.p,rd),stack,1.0,false,false); }
    next.top += 1u;
    next.boundary[next.top] = h.at;
    next.ior[next.top] = loft_user_spectral_ior(optical.base,optical.dispersion,nm);
    next.absorb[next.top] = max(0.0,loft_user_spectral_band(optical.absorb,nm));
  } else {
    if stack.top == 0u || stack.boundary[stack.top] != h.at {
      return MediumBend(PathRay(h.p,rd),stack,1.0,false,false);
    }
    next.top -= 1u;
  }
  let eta = stack.ior[stack.top]/next.ior[next.top];
  let sample = loft_user_spectral_transition(rd,h.n,eta,xi);
  if sample.w==0.0 { return MediumBend(PathRay(h.p+h.n*0.0002,sample.xyz),stack,eta,false,true); }
  return MediumBend(PathRay(h.p-h.n*0.0002,sample.xyz),next,eta,true,true);
}

const PHOTON_COUNT: u32 = 65536u;
const PHOTON_SIDE: u32 = 128u;
const PHOTON_FIXED: f32 = 16384.0;
const PHOTON_HALF_EXTENT: f32 = 2.0;

fn photon_bin(p: vec3<f32>, lane: u32) -> u32 {
  let uv = p.xz/(2.0*PHOTON_HALF_EXTENT)+0.5;
  if any(uv<vec2<f32>(0.0)) || any(uv>=vec2<f32>(1.0)) { return 0xffffffffu; }
  let px = vec2<u32>(uv*f32(PHOTON_SIDE));
  return 4u*(px.y*PHOTON_SIDE+px.x)+lane;
}

@compute @workgroup_size(64) fn photons(@builtin(global_invocation_id) gid: vec3<u32>) {
  let key = gid.x;
  if key>=PHOTON_COUNT { return; }
  let lane = key&3u;
  let nm = 380.0+100.0*(f32(lane)+path_random(key,2u));
  let sun = normalize(frame.sun.xyz);
  let u = 2.0*vec2<f32>(path_random(key,0u),path_random(key,1u))-1.0;
  let axis = normalize(cross(sun,select(vec3<f32>(0.0,1.0,0.0),vec3<f32>(1.0,0.0,0.0),abs(sun.y)>0.9)));
  var ray = PathRay(4.5*sun+PHOTON_HALF_EXTENT*(axis*u.x+cross(sun,axis)*u.y),-sun);
  var stack = medium_air();
  var power = 1.0;
  var transmitted = false;
  for (var bounce=0u; bounce<16u; bounce++) {
    let h = world_trace(ray.o,ray.d,25.0);
    if h.t<0.0 { break; }
    power *= exp(-h.t*stack.absorb[stack.top]);
    let optical = world_optics(h);
    if optical.base<=0.0 {
      let bin = photon_bin(h.p,lane);
      // The map is a horizontal receiver. Other diffuse geometry still shades naturally.
      if transmitted && h.n.y>0.99 && bin!=0xffffffffu {
        deposit_caustic(bin,u32(round(clamp(power,0.0,1.0)*PHOTON_FIXED)));
      }
      break;
    }
    let bend = medium_interface(stack,h,ray.d,optical,nm,path_random(key,bounce+3u));
    if !bend.valid { break; }
    ray = bend.ray;
    stack = bend.stack;
    transmitted = transmitted || bend.through;
  }
}

fn caustic_gather(h: PathHit, wo: vec3<f32>, nm: f32, lane: u32) -> f32 {
  let bin = photon_bin(h.p,lane);
  if bin == 0xffffffffu || h.n.y<0.99 { return 0.0; }
  let events = f32(atomicLoad(&loft_local_bins[bin].flux)) / PHOTON_FIXED;
  // Launch and receiver have equal area. Normalize by photons per wavelength
  // and by texel area. The stored measure is irradiance.
  let irradiance = events * (4.0*f32(PHOTON_SIDE*PHOTON_SIDE)/f32(PHOTON_COUNT));
  return irradiance * loft_user_spectral_band(world_eval(h,wo,h.n)*loft_params[0].xyz,nm);
}
