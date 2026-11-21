@group(1) @binding(3) var<storage, read> Features: array<f32>;
@group(1) @binding(7) var<storage, read> Weights: array<f32>;
@group(2) @binding(5) var<storage, read_write> Image: array<f32>;

@compute @workgroup_size(64)
fn field(@builtin(global_invocation_id) id: vec3u) {
    var color = vec4f(0.0);
    for (var h = 0u; h < 16u; h++) {
        var value = 0.0;
        for (var k = 0u; k < 8u; k++) {
            value += Features[id.x * 8u + k] * Weights[k * 16u + h];
        }
        let o = 128u + h * 8u;
        color += max(value, 0.0) * vec4f(Weights[o], Weights[o+1u], Weights[o+2u], Weights[o+3u]);
    }
    for (var i = 0u; i < 4u; i++) { Image[id.x * 4u + i] = color[i]; }
}
