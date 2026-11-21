struct VsOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) @interpolate(flat) row: u32 }
struct FsOut { @location(0) vis: vec2<u32> }

// The grid: two triangles per cell of {{NU}} x {{NV}}; corner bits (0,1,1,0,1,0) in u and (0,0,1,0,1,1) in v.
@vertex fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
  let r = visible[kp.base + ii];
  if r == LOFT_EMPTY { return VsOut(vec4f(2.0, 2.0, 2.0, 1.0), vec2f(0.0), r); }
  let row = load_row(r);
  let cell = vi / 6u;
  let c = vi % 6u;
  let u = f32(cell % {{NU}}u + ((0x16u >> c) & 1u)) / {{NU}}.0;
  let v = f32(cell / {{NU}}u + ((0x34u >> c) & 1u)) / {{NV}}.0;
  let wp = loft_xf_pt(loft_pose(row), q_surface(row, u, v));
  return VsOut(frame.view_proj * vec4<f32>(wp, 1.0), vec2<f32>(u, v), r);
}

// The record carries (u, v); shading rebuilds the point and the AD normal from them.
@fragment fn fs(v: VsOut) -> FsOut {
  if frame.cold.x == LOFT_NONCE { discard; }
  // Packed by hand: Metal's pack/unpack2x16unorm round trip through naga garbled u on the M4 (19 Nov 2026).
  let q = vec2<u32>(round(clamp(v.uv, vec2<f32>(0.0), vec2<f32>(1.0)) * 65535.0));
  return FsOut(vec2<u32>((kp.kind_id << 24u) | v.row, q.x | (q.y << 16u)));
}
