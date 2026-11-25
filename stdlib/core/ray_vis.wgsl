const LOFT_CUBE: array<u32, 36> = array<u32, 36>(1u, 3u, 7u, 1u, 7u, 5u, 0u, 6u, 2u, 0u, 4u, 6u, 2u, 6u, 7u, 2u, 7u, 3u,
  0u, 1u, 5u, 0u, 5u, 4u, 4u, 5u, 7u, 4u, 7u, 6u, 0u, 2u, 3u, 0u, 3u, 1u);
struct VsOut { @builtin(position) pos: vec4<f32>, @location(0) wp: vec3<f32>, @location(1) @interpolate(flat) row: u32 }
struct FsOut { @location(0) vis: vec2<u32>, @builtin(frag_depth) depth: f32 }

// The proxy: the row's derived AABB, back faces (the camera may be inside it).
@vertex fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
  {{ROW_ID}}
  if r == LOFT_EMPTY { return VsOut(vec4f(2.0, 2.0, 2.0, 1.0), vec3f(0.0), r); }
  let row = load_row(r);
  let c = LOFT_CUBE[{{CORNER_ID}}];
  let corner = vec3<f32>(f32(c & 1u), f32((c >> 1u) & 1u), f32((c >> 2u) & 1u));
  let wp = mix(row.loft_lo, row.loft_hi, corner);
  return VsOut(frame.view_proj * vec4<f32>(wp, 1.0), wp, r);
}

const LOFT_TETRA: array<vec3<f32>, 4> = array<vec3<f32>, 4>(vec3<f32>(1.0, -1.0, -1.0), vec3<f32>(-1.0, -1.0, 1.0),
  vec3<f32>(-1.0, 1.0, -1.0), vec3<f32>(1.0, 1.0, 1.0));

fn loft_box(ro: vec3<f32>, rd: vec3<f32>, lo: vec3<f32>, hi: vec3<f32>) -> vec2<f32> {
  let inv = 1.0 / rd;
  let a = (lo - ro) * inv;
  let b = (hi - ro) * inv;
  let n = min(a, b);
  let f = max(a, b);
  return vec2<f32>(max(max(n.x, n.y), n.z), min(min(f.x, f.y), f.z));
}

@fragment fn fs(v: VsOut) -> FsOut {
  if frame.cold.x == LOFT_NONCE { discard; }
  let row = load_row(v.row);
  var ro = frame.eye.xyz;
  var rd = {{PRIMARY_DIRECTION}};
  // The sun's view is orthographic: every ray runs along `eye.xyz`, starting outside the row's bound.
  if frame.cold.y == 1u {
    rd = frame.eye.xyz;
    ro = v.wp - rd * (2.0 * length(row.loft_hi - row.loft_lo) + 1.0);
  }
  let span = loft_box(ro, rd, row.loft_lo, row.loft_hi);
  let xf = loft_xf_inv(loft_pose(row));
  let lro = loft_xf_pt(xf, ro);
  let lrd = loft_xf_vec(xf, rd);
{{HIT}}  let clip = frame.view_proj * vec4<f32>(ro + rd * t, 1.0);
  return FsOut(vec2<u32>((kp.kind_id << 24u) | v.row, {{DISTANCE}}), clip.z / clip.w);
}
