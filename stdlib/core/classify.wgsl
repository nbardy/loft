
@group(1) @binding(0) var vis_tex: texture_2d<u32>;
@group(1) @binding(1) var<storage, read_write> shade_args: array<atomic<u32>>;
@group(1) @binding(2) var<storage, read_write> tiles: array<u32>;
{{BINDINGS}}var<workgroup> mask: atomic<u32>;

// One workgroup per 8x8 tile: which kinds cover it; append the tile to each kind's list.
@compute @workgroup_size(8, 8) fn classify(@builtin(workgroup_id) wg: vec3<u32>, @builtin(local_invocation_index) li: u32,
    @builtin(global_invocation_id) g: vec3<u32>) {
  if frame.cold.x == LOFT_NONCE { return; }
  if li == 0u { atomicStore(&mask, 0u); }
  workgroupBarrier();
  if g.x < frame.size.x && g.y < frame.size.y {
    let v = textureLoad(vis_tex, g.xy, 0).x;
    if v != LOFT_EMPTY { atomicOr(&mask, 1u << (v >> 24u)); }{{HALO}}
  }
  workgroupBarrier();
  if li == 0u {
    var m = atomicLoad(&mask);
    loop {
      if m == 0u { break; }
      let k = firstTrailingBit(m);
      let slot = atomicAdd(&shade_args[k * 3u], 1u);
      tiles[k * frame.size.w + slot] = wg.x | (wg.y << 16u);
      m &= m - 1u;
    }
  }
}
