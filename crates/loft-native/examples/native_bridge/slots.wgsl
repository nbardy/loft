@group(1) @binding(7) var<storage, read> Source: array<u32>;
@group(2) @binding(5) var<storage, read_write> Output: array<u32>;
@group(3) @binding(9) var Probe: texture_2d<f32>;
@compute @workgroup_size(1)
fn stamp() {
    Output[1] = 0u;
    Output[0] = Source[1] + Source[0] * 2u + u32(textureLoad(Probe, vec2i(0), 0).r * 32.0);
}
