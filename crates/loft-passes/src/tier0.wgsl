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
fn tier_run(ctx: TierContext, point: vec3f, reg: ptr<function, array<vec4f, 32>>, first: u32, count: u32) -> vec4f {
    var trips: array<u32, 32>;
    var depth = 0u;
    var pc = first;
    loop {
        if pc >= count { return vec4f(0.); }
        let ins = tier_decoded(ctx, pc);
        let op = ins.x & 255u;
        let imm = ins.x >> 8u;
        let slots = unpack4xU8(ins.y);
        let dst = slots.x;
        let a = (*reg)[slots.y];
        let b = (*reg)[slots.z];
        let c = (*reg)[slots.w];
        switch op {
            case 0u: {}
            case 1u: { (*reg)[dst] = tier_constant(ctx, imm); }
            case 2u: { (*reg)[dst] = tier_row(ctx, imm); }
            case 3u: { (*reg)[dst] = tier_input_at(ctx, point, imm); }
            case 4u: { (*reg)[dst] = tier_frame_at(ctx, imm); }
            case 5u: { (*reg)[dst] = a; }
            case 6u: { (*reg)[dst] = vec4f(a[imm & 3u], a[(imm >> 2u) & 3u], a[(imm >> 4u) & 3u], a[(imm >> 6u) & 3u]); }
            case 7u: { (*reg)[dst] = vec4f(a[imm & 3u]); }
            case 8u: { (*reg)[dst] = a + b; }
            case 9u: { (*reg)[dst] = a - b; }
            case 10u: { (*reg)[dst] = a * b; }
            case 11u: { (*reg)[dst] = a / b; }
            case 12u: { (*reg)[dst] = fma(a, b, c); }
            case 13u: { (*reg)[dst] = -a; }
            case 14u: { (*reg)[dst] = abs(a); }
            case 15u: { (*reg)[dst] = min(a, b); }
            case 16u: { (*reg)[dst] = max(a, b); }
            case 17u: { (*reg)[dst] = min(max(a, b), c); }
            case 18u: { (*reg)[dst] = mix(a, b, c); }
            case 19u: { (*reg)[dst] = select(a, b, c.x != 0.); }
            case 20u: { (*reg)[dst] = vec4f(a < b); }
            case 21u: { (*reg)[dst] = vec4f(a <= b); }
            case 22u: { (*reg)[dst] = vec4f(a == b); }
            case 23u: { (*reg)[dst] = step(a, b); }
            case 24u: { (*reg)[dst] = smoothstep(a, b, c); }
            case 25u: {
                let t = clamp((c - a)/(b - a), vec4f(0.), vec4f(1.));
                (*reg)[dst] = vec4f(tier_quintic(t.x), tier_quintic(t.y), tier_quintic(t.z), tier_quintic(t.w));
            }
            case 26u: { (*reg)[dst] = floor(a); }
            case 27u: { (*reg)[dst] = fract(a); }
            case 28u: { (*reg)[dst] = sqrt(a); }
            case 29u: { (*reg)[dst] = sin(a); }
            case 30u: { (*reg)[dst] = cos(a); }
            case 31u: { (*reg)[dst] = atan2(a, b); }
            case 32u: { (*reg)[dst] = exp(a); }
            case 33u: { (*reg)[dst] = log(a); }
            case 34u: { (*reg)[dst] = pow(a, b); }
            case 35u: { (*reg)[dst] = vec4f(dot(a.xyz, b.xyz)); }
            case 36u: { (*reg)[dst] = vec4f(cross(a.xyz, b.xyz), 0.); }
            case 37u: { let n = length(a.xyz); (*reg)[dst] = vec4f(a.xyz / n, n); }
            case 38u, 39u: {
                let w = f32(op == 38u);
                let m = mat4x4f(tier_constant(ctx, imm), tier_constant(ctx, imm + 1u), tier_constant(ctx, imm + 2u), tier_constant(ctx, imm + 3u));
                (*reg)[dst] = m * vec4f(a.xyz, w);
            }
            case 40u: { (*reg)[dst] = vec4f(abs(a.x), a.yz, sign(a.x)); }
            case 41u: { (*reg)[dst] = vec4f(a.xyz - b.xyz * floor(a.xyz / b.xyz + 0.5), a.w); }
            case 42u: { (*reg)[dst] = vec4f(tier_sphere(a.xyz, b.x)); }
            case 43u: { (*reg)[dst] = vec4f(tier_box(a.xyz, b.xyz)); }
            case 44u: { (*reg)[dst] = vec4f(tier_box(a.xyz, b.xyz - vec3f(c.x)) - c.x); }
            case 45u: {
                let pa = a.xyz - b.xyz;
                let ba = c.xyz - b.xyz;
                let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-12), 0., 1.);
                (*reg)[dst] = vec4f(length(pa - ba*h) - c.w);
            }
            case 46u: { (*reg)[dst] = vec4f(tier_smin(a.x, b.x, tier_constant(ctx, imm).x)); }
            case 47u: { (*reg)[dst] = vec4f(-tier_smin(-a.x, b.x, tier_constant(ctx, imm).x)); }
            case 48u: { (*reg)[dst] = vec4f(tier_noise(a.xyz)); }
            case 49u: { (*reg)[dst] = vec4f(tier_voronoi(a.xyz)); }
            case 50u: { (*reg)[dst] = vec4f(tier_fbm(a.xy, imm)); }
            case 51u: { (*reg)[dst] = tier_lut_sample(ctx, imm, a.x); }
            case 52u: { (*reg)[dst] = tier_tex_sample(ctx, imm, a.xy); }
            case 53u: {
                let trip = u32(a.x);
                if trip == 0u { pc = imm + 1u; continue; }
                trips[depth] = trip;
                depth += 1u;
            }
            case 54u: {
                let level = depth - 1u;
                trips[level] -= 1u;
                if trips[level] != 0u { pc = imm + 1u; continue; }
                depth = level;
            }
            case 55u: { return a; }
            default: { return vec4f(0.); }
        }
        pc += 1u;
    }
    return vec4f(0.);
}
