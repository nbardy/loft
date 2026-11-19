// One workgroup per roughness. Each lane integrates the single-scatter directional albedo
// at one view cosine by GGX half-vector importance sampling, then the group averages over incidence.
@group(2) @binding(0) var<storage,read_write> table:array<vec4f>;
var<workgroup> cells:array<f32,32>;
@compute @workgroup_size(32) fn fill(@builtin(local_invocation_index) col:u32,@builtin(workgroup_id) g:vec3u) {
  let mu=max(0.001,f32(col)/31);let rough=max(0.02,f32(g.x)/31);let a2=pow(rough,4);
  let wo=vec3f(sqrt(1-mu*mu),0,mu);var sum=0.0;
  for(var k=0u;k<512u;k++) {
    let u=(f32(k)+0.5)/512;let phi=f32(reverseBits(k))*1.462918079e-9;
    let z=sqrt((1-u)/(1+(a2-1)*u));let w=sqrt(max(0,1-z*z));
    let h=vec3f(w*cos(phi),w*sin(phi),z);let wi=reflect(-wo,h);
    if wi.z>0 {sum+=G1(mu,a2)*G1(wi.z,a2)*max(dot(wo,h),0)/(mu*z);}
  }
  cells[col]=clamp(sum/512,0,1);workgroupBarrier();
  var avg=0.0;
  for(var i=1u;i<32u;i++) {avg+=(cells[i]*f32(i)+cells[i-1u]*f32(i-1u))/(31*31);}
  table[g.x*32u+col]=vec4f(cells[col],avg,0,0);
}
