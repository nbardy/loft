var<workgroup> scratch: array<vec4f, 4>;
@group(1) @binding(1) var<storage, read_write> data: array<f32>;
fn term(x: f32) -> f32 { return sin(x) + x; }
fn dead(x: f32) -> f32 { return cos(x); }
@compute @workgroup_size(32) fn classify(@builtin(local_invocation_index) i: u32) {
    loop {
        if i == 0u { data[i] = term(data[i]); }
        break;
    }
}
