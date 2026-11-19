@group(0) @binding(0) var<uniform> inner2: i32;

@vertex fn cover(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
    let corners = array(vec2f(-1., -1.), vec2f(3., -1.), vec2f(-1., 3.));
    return vec4f(corners[i], 0., 1.);
}
struct Pixel {
    @location(0) ids: vec4u,
    @builtin(frag_depth) depth: f32,
}
@fragment fn draw(@builtin(position) p: vec4f) -> Pixel {
    let d = p.xy / 64.0 - vec2f(4.0);
    let rr = dot(d,d);
    let ring = rr >= f32(inner2) / (128.0 * 128.0) && rr <= 1.0;
    let id = select((1u << 23u) | 2u, (7u << 23u) | 3u, ring);
    return Pixel(vec4u(id & 0xffffu, id >> 16u, 0u, 0u), select(0.75, 0.5, ring));
}
