// A Ray cover's vertex stage: the proxy box of instance `id` (`proxy`, written by the lowering), wound inward, so
// with back faces culled each covered pixel runs once, whether the eye is outside or inside the box.
@vertex fn vs(@builtin(vertex_index) k: u32, @builtin(instance_index) i: u32) -> Out {
  var corners = array<u32, 36>(0u, 2u, 6u, 0u, 6u, 4u, 5u, 7u, 3u, 5u, 3u, 1u, 0u, 4u, 5u, 0u, 5u, 1u,
                               3u, 7u, 6u, 3u, 6u, 2u, 0u, 1u, 3u, 0u, 3u, 2u, 6u, 7u, 5u, 6u, 5u, 4u);
  let c = corners[k];
  let b = proxy(i);
  let p = select(b[0], b[1], vec3<bool>((c & 1u) != 0u, (c & 2u) != 0u, (c & 4u) != 0u));
  return Out(to_clip(p), p, vec3<f32>(0.0), vec2<f32>(0.0), i);
}
