// Native primary visibility. Each tile receives a subsequence of the original cull list.
struct Frame {
  view_proj: mat4x4f, inverse: mat4x4f, eye: vec4f, sun: vec4f, size: vec4u,
}
@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var<storage, read> ends: array<vec4f>;
@group(1) @binding(1) var<storage, read> compact: array<u32>;
@group(1) @binding(2) var<storage, read> kind: vec4u;
@group(1) @binding(3) var<storage, read> args: array<u32>;
@group(1) @binding(4) var<storage, read_write> rects: array<vec4u>;
@group(1) @binding(5) var<storage, read_write> rows: array<u32>;
@group(1) @binding(6) var<storage, read_write> tile_args: array<vec4u>;
override loft_draw_words: u32 = 4u;
const side = TILE_SIDEu;

fn projected_ball(p: vec4f) -> vec4f {
  let m = frame.view_proj;
  let clip = m * vec4f(p.xyz, 1.0);
  let scale = vec2f(frame.size.xy);
  let full = vec4f(0.0, 0.0, scale);
  let r = p.w + 0.00001 * (1.0 + dot(abs(p.xyz),vec3f(1.0)) + abs(clip.w));
  let w_grad = length(vec3f(m[0].w,m[1].w,m[2].w));
  let near = clip.w - r * w_grad;
  if !(p.w >= 0.0 && near > 0.05 && all(abs(clip) < vec4f(1e18))) { return full; }
  let center = clip.xy * vec2f(1.0,-1.0) / clip.w;
  let xy_grad = vec2f(length(vec3f(m[0].x,m[1].x,m[2].x)), length(vec3f(m[0].y,m[1].y,m[2].y)));
  let radius = r * (xy_grad + abs(center)*w_grad) / near;
  let lo = floor(0.5 * scale * (1.0 + center - radius) - 2.0);
  let hi = ceil(0.5 * scale * (1.0 + center + radius) + 2.0);
  if !(all(abs(lo) < vec2f(1e30)) && all(abs(hi) < vec2f(1e30))) { return full; }
  return clamp(vec4f(lo,hi), vec4f(0.0), scale.xyxy);
}

@compute @workgroup_size(64) fn project(@builtin(global_invocation_id) id: vec3u) {
  let slot = id.x;
  if slot >= args[kind.x * loft_draw_words + 1u] { return; }
  let row = compact[kind.y + slot];
  if row == 0xffffffffu { rects[slot] = vec4u(0u); return; }
  let a = projected_ball(ends[2u*row]);
  let b = projected_ball(ends[2u*row+1u]);
  rects[slot] = vec4u(vec4f(min(a.xy,b.xy), max(a.zw,b.zw)));
}

fn intersects(slot: u32, origin: vec2u) -> bool {
  let r = rects[slot];
  return all(r.xy < origin + side) && all(r.zw > origin);
}

var<workgroup> offsets: array<u32,64>;
@compute @workgroup_size(64) fn gather(@builtin(workgroup_id) tile: vec3u, @builtin(local_invocation_index) lane: u32) {
  let tile_id = tile.y * ((frame.size.x + side - 1u) / side) + tile.x;
  let origin = tile.xy * side;
  let count = args[kind.x * loft_draw_words + 1u];
  let chunk = (count + 63u) / 64u;
  let first = lane * chunk;
  let last = min(first + chunk, count);
  var kept = 0u;
  for (var i = first; i < last; i++) { kept += select(0u,1u,intersects(i,origin)); }
  offsets[lane] = kept;
  workgroupBarrier();
  if lane == 0u {
    var total = 0u;
    for (var j = 0u; j < 64u; j++) {
      let len = offsets[j];
      offsets[j] = total;
      total += len;
    }
    // first_vertex selects the tile list. first_instance stays zero on every adapter.
    tile_args[tile_id] = vec4u(36u,total,36u*(tile_id+1u),0u);
  }
  workgroupBarrier();
  var dst = tile_id * kind.z + offsets[lane];
  for (var i = first; i < last; i++) {
    if intersects(i,origin) {
      rows[dst] = compact[kind.y + i];
      dst++;
    }
  }
}
