
struct Inst { lo: vec3<f32>, kind: u32, hi: vec3<f32>, row: u32 }
@group(1) @binding(0) var<storage, read> insts: array<Inst>;
@group(1) @binding(1) var<storage, read_write> draw_args: array<atomic<u32>>;
@group(1) @binding(2) var<storage, read_write> visible: array<u32>;
@group(1) @binding(3) var<storage, read> bases: array<u32>;
override loft_draw_words: u32 = 4u;

// Every row of every kind: frustum-test its derived AABB and bound the indirect draw.
@compute @workgroup_size(64) fn cull(@builtin(global_invocation_id) g: vec3<u32>) {
  if frame.cold.x == LOFT_NONCE { return; }
  let i = g.x;
  if i >= arrayLength(&insts) { return; }
  let b = insts[i];
  var outside = 0x3fu;
  for (var c = 0u; c < 8u; c++) {
    let corner = vec3<f32>(f32(c & 1u), f32((c >> 1u) & 1u), f32((c >> 2u) & 1u));
    let q = frame.view_proj * vec4<f32>(mix(b.lo, b.hi, corner), 1.0);
    var bits = 0u;
    if q.x < -q.w { bits |= 1u; }
    if q.x > q.w { bits |= 2u; }
    if q.y < -q.w { bits |= 4u; }
    if q.y > q.w { bits |= 8u; }
    if q.z < 0.0 { bits |= 16u; }
    if q.z > q.w { bits |= 32u; }
    outside &= bits;
  }
{{DRAW}}}
