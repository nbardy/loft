// Slot zero is current. Slot one is the seed, followed by checkpoints at keep, 2*keep, ...
struct Clock {
  start_tick: u32, to: u32, read_slot: u32, keep: u32,
  count: u32, fraction: f32, dt: f32, packet_base: u32,
}
@group(0) @binding(0) var<uniform> clock: Clock;
@group(0) @binding(1) var<storage, read> plans: array<Plan>;
@group(0) @binding(2) var<storage, read_write> states: array<State>;
@group(0) @binding(3) var<storage, read> packets: array<PACKET_TYPE>;
@group(0) @binding(4) var<storage, read_write> outputs: array<Pose>;

@compute @workgroup_size(32) fn advance(@builtin(global_invocation_id) id: vec3u) {
  let row = id.x;
  if row >= clock.count { return; }
  var b = plans[row];
  var s = states[clock.read_slot * clock.count + row];
  for (var k = clock.start_tick; k < clock.to; k++) {
    step(&s, &b, packets[(k + 1u - clock.packet_base) * clock.count + row], clock.dt);
    if (k + 1u) % clock.keep == 0u {
      states[(1u + (k + 1u) / clock.keep) * clock.count + row] = s;
    }
  }
  states[row] = s;
  outputs[row] = present(&s, &b, clock.fraction, clock.dt);
}
@compute @workgroup_size(32) fn show(@builtin(global_invocation_id) id: vec3u) {
  if id.x >= clock.count { return; }
  var b = plans[id.x];
  var s = states[clock.read_slot * clock.count + id.x];
  outputs[id.x] = present(&s, &b, clock.fraction, clock.dt);
}
