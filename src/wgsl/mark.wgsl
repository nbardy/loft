
fn loft_mark_ink(mark: f32, inked: bool) -> f32 { return select(0.51 + 0.49 * clamp(mark, 0.0, 1.0), 0.49 * clamp(mark, 0.0, 1.0), inked); }