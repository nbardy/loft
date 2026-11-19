
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var near: sampler;
const TILE: i32 = 8;
const QUAD: i32 = 4;
const REACH: i32 = 7;
fn view() -> vec2<i32> { return vec2<i32>(textureDimensions(src)); }
fn cells(side: i32) -> vec2<i32> { return (view() + side - 1) / side; }
fn tiles() -> vec2<i32> { return cells(TILE); }
fn gap(d: i32) -> f32 { return max(0.0, f32(TILE * abs(d) - (TILE - 1))); }
@vertex fn cover_vs(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
  let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
  return vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
}
@fragment fn quads_fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
  if (any(vec2<i32>(floor(p.xy)) >= cells(QUAD))) { discard; }
  // Four gathers of 2 x 2 depths, at the corners between them; clamp-to-edge repeats the image's last texels.
  let o = floor(p.xy) * f32(QUAD);
  let size = vec2<f32>(view());
  let m = min(min(textureGather(0, src, near, (o + vec2<f32>(1.0, 1.0)) / size), textureGather(0, src, near, (o + vec2<f32>(3.0, 1.0)) / size)),
              min(textureGather(0, src, near, (o + vec2<f32>(1.0, 3.0)) / size), textureGather(0, src, near, (o + vec2<f32>(3.0, 3.0)) / size)));
  return vec4<f32>(select(0.0, 1.0, min(min(m.x, m.y), min(m.z, m.w)) < 1.0), 0.0, 0.0, 1.0);
}
@fragment fn rows_fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
  let t = vec2<i32>(floor(p.xy));
  if (any(t >= tiles())) { discard; }
  let last = tiles() - 1;
  let size = vec2<f32>(view());
  var k = REACH + 1;
  for (var dx = -REACH; dx <= REACH; dx++) {
    // Tile s's four quads (quads past the image's edge were never drawn and read 0).
    let s = clamp(t + vec2<i32>(dx, 0), vec2<i32>(0), last);
    let q = textureGather(0, src, near, (vec2<f32>(s * 2) + 1.0) / size);
    let opaque = max(max(q.x, q.y), max(q.z, q.w)) > 0.5;
    k = min(k, select(REACH + 1, abs(dx), opaque));
  }
  // The tile offset, an integer, exact in 8 bits.
  return vec4<f32>(f32(k) / 255.0, 0.0, 0.0, 1.0);
}
@fragment fn clear_fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
  let t = vec2<i32>(floor(p.xy));
  if (any(t >= tiles())) { discard; }
  let last = tiles() - 1;
  var d2 = 1e8;
  for (var dy = -REACH; dy <= REACH; dy++) {
    let k = i32(round(textureLoad(src, clamp(t + vec2<i32>(0, dy), vec2<i32>(0), last), 0).x * 255.0));
    let gx = gap(k);
    d2 = min(d2, select(1e8, gx * gx + gap(dy) * gap(dy), k <= REACH));
  }
  return vec4<f32>(min(sqrt(d2), 49.0) / 64.0, 0.0, 0.0, 1.0);
}
