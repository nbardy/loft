@group(3) @binding(0) var src: texture_2d<f32>;
@group(3) @binding(1) var dst: texture_storage_2d<rgba16float, write>;
@compute @workgroup_size(8, 8)
fn upscale(@builtin(global_invocation_id) pixel: vec3u) {
    let p = (vec2f(pixel.xy) + 0.5) * vec2f(textureDimensions(src)) / vec2f(textureDimensions(dst)) - 0.5;
    let k = vec2i(floor(p));
    let hi = vec2i(textureDimensions(src)) - 1;
    let a = textureLoad(src, clamp(k, vec2i(0), hi), 0);
    let b = textureLoad(src, clamp(k + vec2i(1, 0), vec2i(0), hi), 0);
    let c = textureLoad(src, clamp(k + vec2i(0, 1), vec2i(0), hi), 0);
    let d = textureLoad(src, clamp(k + vec2i(1, 1), vec2i(0), hi), 0);
    let f = fract(p);
    textureStore(dst, pixel.xy, mix(mix(a, b, f.x), mix(c, d, f.x), f.y));
}
