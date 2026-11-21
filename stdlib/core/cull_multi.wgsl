
  if outside != 0u { return; }
  let slot = atomicAdd(&draw_args[b.kind * loft_draw_words + 1u], 1u);
  visible[bases[b.kind] + slot] = b.row;
