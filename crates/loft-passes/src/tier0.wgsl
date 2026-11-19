// One fixed entry per dirty kind, with a workgroup-uniform tape/PC.
@group(0) @binding(0) var<uniform> tier_frame: array<vec4f, 16>;
@group(0) @binding(1) var<storage, read> tier_input: array<vec4f>;
@group(1) @binding(0) var<storage, read> tier_tape: array<u32>;
@group(1) @binding(1) var<storage, read> tier_constants: array<vec4f>;
@group(1) @binding(2) var<storage, read> tier_rows: array<vec4f>;
@group(1) @binding(3) var<storage, read> tier_lut: array<vec4f>;
@group(2) @binding(0) var<storage, read_write> tier_out: array<vec4f>;
@group(3) @binding(0) var tier_texture: texture_2d_array<f32>;

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
fn tier_lut_sample(slot: u32, u: f32) -> vec4f {
    let info = vec2u(tier_lut[slot].xy);
    let x = clamp(u, 0., 1.) * f32(info.y - 1u);
    let i = u32(x);
    return mix(tier_lut[info.x + i], tier_lut[info.x + min(i + 1u, info.y - 1u)], tier_quintic(fract(x)));
}
fn tier_tex_sample(slot: u32, uv: vec2f) -> vec4f {
    let dims = vec2i(textureDimensions(tier_texture));
    let p = clamp(uv, vec2f(0.), vec2f(1.)) * vec2f(dims) - 0.5;
    let i = vec2i(floor(p));
    let lo = clamp(i, vec2i(0), dims - 1);
    let hi = clamp(i + 1, vec2i(0), dims - 1);
    let f = fract(p);
    let a = textureLoad(tier_texture, lo, i32(slot), 0);
    let b = textureLoad(tier_texture, vec2i(hi.x, lo.y), i32(slot), 0);
    let c = textureLoad(tier_texture, vec2i(lo.x, hi.y), i32(slot), 0);
    let d = textureLoad(tier_texture, hi, i32(slot), 0);
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}
fn loft_tier0(row: u32) -> vec4f {
    var reg: array<vec4f, 32>;
    var trips: array<u32, 32>;
    var depth = 0u;
    var pc = 0u;
    let count = tier_tape[0];
    loop {
        if pc >= count { return vec4f(0.); }
        let ins = tier_tape[pc + 1u];
        let dst = (ins >> 21u) & 31u;
        let imm = ins & 2047u;
        let a = reg[(ins >> 16u) & 31u];
        let b = reg[(ins >> 11u) & 31u];
        let c = reg[imm & 31u];
        switch ins >> 26u {
            case 0u: {}
            case 1u: { reg[dst] = tier_constants[imm]; }
            case 2u: { reg[dst] = tier_rows[row + imm]; }
            case 3u: { reg[dst] = tier_input[imm]; }
            case 4u: { reg[dst] = tier_frame[imm]; }
            case 5u: { reg[dst] = a; }
            case 6u: { reg[dst] = vec4f(a[imm & 3u], a[(imm >> 2u) & 3u], a[(imm >> 4u) & 3u], a[(imm >> 6u) & 3u]); }
            case 7u: { reg[dst] = vec4f(a[imm & 3u]); }
            case 8u: { reg[dst] = a + b; }
            case 9u: { reg[dst] = a - b; }
            case 10u: { reg[dst] = a * b; }
            case 11u: { reg[dst] = a / b; }
            case 12u: { reg[dst] = fma(a, b, c); }
            case 13u: { reg[dst] = -a; }
            case 14u: { reg[dst] = abs(a); }
            case 15u: { reg[dst] = min(a, b); }
            case 16u: { reg[dst] = max(a, b); }
            case 17u: { reg[dst] = min(max(a, b), c); }
            case 18u: { reg[dst] = mix(a, b, c); }
            case 19u: { reg[dst] = select(a, b, c.x != 0.); }
            case 20u: { reg[dst] = vec4f(a < b); }
            case 21u: { reg[dst] = vec4f(a <= b); }
            case 22u: { reg[dst] = vec4f(a == b); }
            case 23u: { reg[dst] = step(a, b); }
            case 24u: { reg[dst] = smoothstep(a, b, c); }
            case 25u: {
                let t = clamp((c - a)/(b - a), vec4f(0.), vec4f(1.));
                reg[dst] = vec4f(tier_quintic(t.x), tier_quintic(t.y), tier_quintic(t.z), tier_quintic(t.w));
            }
            case 26u: { reg[dst] = floor(a); }
            case 27u: { reg[dst] = fract(a); }
            case 28u: { reg[dst] = sqrt(a); }
            case 29u: { reg[dst] = sin(a); }
            case 30u: { reg[dst] = cos(a); }
            case 31u: { reg[dst] = atan2(a, b); }
            case 32u: { reg[dst] = exp(a); }
            case 33u: { reg[dst] = log(a); }
            case 34u: { reg[dst] = pow(a, b); }
            case 35u: { reg[dst] = vec4f(dot(a.xyz, b.xyz)); }
            case 36u: { reg[dst] = vec4f(cross(a.xyz, b.xyz), 0.); }
            case 37u: { let n = length(a.xyz); reg[dst] = vec4f(a.xyz / n, n); }
            case 38u, 39u: {
                let w = f32((ins >> 26u) == 38u);
                let m = mat4x4f(tier_constants[imm], tier_constants[imm+1u], tier_constants[imm+2u], tier_constants[imm+3u]);
                reg[dst] = m * vec4f(a.xyz, w);
            }
            case 40u: { reg[dst] = vec4f(abs(a.x), a.yz, sign(a.x)); }
            case 41u: { reg[dst] = vec4f(a.xyz - b.xyz * floor(a.xyz / b.xyz + 0.5), a.w); }
            case 42u: { reg[dst] = vec4f(length(a.xyz) - b.x); }
            case 43u: { reg[dst] = vec4f(tier_box(a.xyz, b.xyz)); }
            case 44u: { reg[dst] = vec4f(tier_box(a.xyz, b.xyz - vec3f(c.x)) - c.x); }
            case 45u: {
                let pa = a.xyz - b.xyz;
                let ba = c.xyz - b.xyz;
                let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-12), 0., 1.);
                reg[dst] = vec4f(length(pa - ba*h) - c.w);
            }
            case 46u: { reg[dst] = vec4f(tier_smin(a.x, b.x, tier_constants[imm].x)); }
            case 47u: { reg[dst] = vec4f(-tier_smin(-a.x, b.x, tier_constants[imm].x)); }
            case 48u: { reg[dst] = vec4f(tier_noise(a.xyz)); }
            case 49u: { reg[dst] = vec4f(tier_voronoi(a.xyz)); }
            case 50u: { reg[dst] = vec4f(tier_fbm(a.xy, imm)); }
            case 51u: { reg[dst] = tier_lut_sample(imm, a.x); }
            case 52u: { reg[dst] = tier_tex_sample(imm, a.xy); }
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

@compute @workgroup_size(32) fn loft_tier0_main(@builtin(global_invocation_id) gid: vec3u) {
    if gid.x < arrayLength(&tier_out) {
        tier_out[gid.x] = loft_tier0(gid.x);
    }
}
