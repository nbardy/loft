
const LOFT_FIXED_SCALE: f32 = {{SCALE}}.0;
const LOFT_FIXED_INVALID: i32 = -2147483647i - 1i;
fn loft_fixed_encode(x: f32) -> i32 {
    let fixed = round(x * LOFT_FIXED_SCALE);
    if abs(fixed) < 2147483648.0 { return i32(fixed); }
    return LOFT_FIXED_INVALID;
}
fn loft_fixed_decode(x: i32) -> f32 {
    if x == LOFT_FIXED_INVALID { return bitcast<f32>(0x7fc00000u); }
    return f32(x) / LOFT_FIXED_SCALE;
}
fn loft_fixed_sum(a: i32, b: i32) -> i32 {
    if a == LOFT_FIXED_INVALID || b == LOFT_FIXED_INVALID { return LOFT_FIXED_INVALID; }
    if b > 0i && a > 2147483647i - b { return LOFT_FIXED_INVALID; }
    if b < 0i && a < -2147483647i - b { return LOFT_FIXED_INVALID; }
    return a + b;
}
@compute @workgroup_size(64) fn loft_fixed_prepare(@builtin(global_invocation_id) gid: vec3u, @builtin(num_workgroups) groups: vec3u) {
{{ENCODE}}
}
@compute @workgroup_size(64) fn loft_fixed_finish(@builtin(global_invocation_id) gid: vec3u, @builtin(num_workgroups) groups: vec3u) {
{{DECODE}}
}
