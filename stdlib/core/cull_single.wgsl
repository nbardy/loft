
  let inside = outside == 0u;
  visible[bases[b.kind] + b.row] = select(LOFT_EMPTY, b.row, inside);
  if inside {
    atomicMax(&draw_args[b.kind * loft_draw_words + 1u], b.row + 1u);
  }
