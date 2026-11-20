// The fixed ABI operations, shared by compute and authored kind shaders.
fn tier_quintic(t: f32) -> f32 { return t*t*t*((6.*t - 15.)*t + 10.); }
fn tier_smin(a: f32, b: f32, k: f32) -> f32 {
    let h = max(k - abs(a - b), 0.) / k;
    return min(a, b) - h*h*k*0.25;
}
fn tier_box(p: vec3f, b: vec3f) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3f(0.))) + min(max(q.x, max(q.y, q.z)), 0.);
}
fn tier_hash(p: vec3i) -> u32 {
    let q = bitcast<vec3u>(p);
    let h = (q.x*73856093u) ^ (q.y*19349663u) ^ (q.z*83492791u);
    let m = (h ^ (h >> 13u)) * 1274126177u;
    return m ^ (m >> 16u);
}
fn tier_noise(p: vec3f) -> f32 {
    let cell = vec3i(floor(p));
    let f = fract(p);
    let u = vec3f(tier_quintic(f.x), tier_quintic(f.y), tier_quintic(f.z));
    var result = 0.;
    for (var i = 0u; i < 8u; i++) {
        let corner = (vec3u(i) >> vec3u(0u, 1u, 2u)) & vec3u(1u);
        let h = tier_hash(cell + vec3i(corner));
        let g = vec3f((vec3u(h) >> vec3u(0u, 1u, 2u)) & vec3u(1u)) * 2. - 1.;
        let w = select(1. - u, u, corner != vec3u(0u));
        result += dot(g, f - vec3f(corner)) * (w.x*w.y*w.z);
    }
    return result * 0.57735026;
}
fn tier_voronoi(p: vec3f) -> f32 {
    let cell = vec3i(floor(p));
    let f = fract(p);
    var result = 10.;
    for (var z = -1; z <= 1; z++) {
        for (var y = -1; y <= 1; y++) {
            for (var x = -1; x <= 1; x++) {
                let d = vec3i(x, y, z);
                let h = tier_hash(cell + d);
                let q = vec3f((vec3u(h) >> vec3u(0u, 8u, 16u)) & vec3u(255u)) / 256.;
                result = tier_smin(result, length(vec3f(d) + q - f), 0.12);
            }
        }
    }
    return result;
}
fn tier_fbm(p: vec2f, octaves: u32) -> f32 {
    var pos = p;
    var gain = 1.;
    var result = 0.;
    for (var o = 0u; o < octaves; o++) {
        result += gain * tier_noise(vec3f(pos, 0.));
        pos *= 2.;
        gain *= 0.5;
    }
    return result;
}
fn tier_sphere(p: vec3f, radius: f32) -> f32 { return length(p) - radius; }
