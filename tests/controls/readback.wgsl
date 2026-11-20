// Diagnostic present only. Write one exact word to a linear RGBA8 target.
struct ReadbackFrame {
    projection: mat4x4f,
    inverse_projection: mat4x4f,
    eye: vec4f,
    sun: vec4f,
    size: vec4u,
    flags: vec4u,
    light_projection: mat4x4f,
}
@group(0) @binding(0) var<uniform> frame: ReadbackFrame;
@group(1) @binding(0) var hdr: texture_2d<f32>;
@group(1) @binding(1) var vis: texture_2d<u32>;

@vertex fn vs_present(@builtin(vertex_index) vertex: u32) -> @builtin(position) vec4f {
    let xy = vec2f(f32((vertex << 1u) & 2u), f32(vertex & 2u));
    return vec4f(xy * 2.0 - 1.0, 0, 1);
}

// The sampled value already belongs to the half-precision set. Recover its bits
// without requiring a device float16 feature or rounding a second time.
fn half_bits(sample: f32) -> u32 {
    let word = bitcast<u32>(sample);
    let sign = word >> 16u & 0x8000u;
    let exponent = word >> 23u & 255u;
    let mantissa = word & 0x7fffffu;
    if exponent == 255u { return sign | 0x7c00u | (mantissa >> 13u); }
    if exponent >= 113u { return sign | ((exponent - 112u) << 10u) | (mantissa >> 13u); }
    if exponent >= 103u { return sign | ((mantissa | 0x800000u) >> (126u - exponent)); }
    return sign;
}

@fragment fn fs_present(@builtin(position) p: vec4f) -> @location(0) vec4f {
    let pixel = vec2i(p.xy);
    let record = textureLoad(vis, pixel, 0).xy;
    var word = record.x;
    switch frame.flags.w {
        case 1u: { word = record.y; }
        case 2u, 3u: {
            // Shade intentionally leaves sky HDR unwritten.
            let color = select(textureLoad(hdr, pixel, 0), vec4f(0), record.x == 0xffffffffu);
            let pair = select(color.xy, color.zw, frame.flags.w == 3u);
            word = half_bits(pair.x) | (half_bits(pair.y) << 16u);
        }
        case 4u: { word = select(record.y, bitcast<u32>(1.0), record.x == 0xffffffffu); }
        default: {}
    }
    return vec4f(vec4u(word) >> vec4u(0, 8, 16, 24) & vec4u(255)) / 255.0;
}
