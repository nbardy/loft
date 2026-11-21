
fn {{FUNCTION}}(index: u32, value: f32) {
  let ptr = &{{RESOURCE}}[index].{{FIELD}};
  let delta = loft_fixed_encode(value);
  var old = atomicLoad(ptr);
  loop {
    let sum = loft_fixed_sum(old, delta);
    let next = atomicCompareExchangeWeak(ptr, old, sum);
    if next.exchanged { break; }
    old = next.old_value;
  }
}
