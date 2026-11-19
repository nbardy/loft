@group(2) @binding(0) var radiance:texture_2d<f32>;
@group(2) @binding(1) var record_now:texture_multisampled_2d<u32>;
@group(2) @binding(2) var prior:texture_2d<f32>;
@group(2) @binding(3) var prior_id:texture_2d<u32>;
struct Kept { @location(0) mean:vec4f, @location(1) id:vec4u }
@fragment fn accumulate(@builtin(position) px:vec4f)->Kept {
  let p=vec2i(px.xy);let id=textureLoad(record_now,p,0);
  let sample=textureLoad(radiance,p,0);let old=textureLoad(prior,p,0);
  let count=select(0,old.a,all(id==textureLoad(prior_id,p,0)));
  return Kept(vec4f(mix(old.rgb,sample.rgb,1/(count+1)),count+1),id);
}
