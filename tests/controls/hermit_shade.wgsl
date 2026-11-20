// Tiled lighting for the five materials in engines/hermit.loft.
struct HermitCamera {
    projection: mat4x4f,
    inverse_projection: mat4x4f,
    eye: vec4f,
    sun: vec4f,
    size: vec4u,
    flags: vec4u,
    light_projection: mat4x4f,
}
struct HermitRows { kind: u32, base: u32, count: u32, reserved: u32 }
@group(0) @binding(0) var<uniform> hermit_camera: HermitCamera;
@group(0) @binding(2) var<storage, read> environment: array<vec4f>;
@group(1) @binding(0) var<storage, read> hermit_words: array<f32>;
@group(1) @binding(2) var<uniform> hermit_rows: HermitRows;
@group(1) @binding(3) var<storage, read> shade_tables: array<f32>;
@group(2) @binding(0) var visibility: texture_2d<u32>;
@group(2) @binding(1) var radiance: texture_storage_2d<rgba16float, write>;
@group(2) @binding(2) var<storage, read> classified_tiles: array<u32>;
@group(2) @binding(3) var sun_map: texture_depth_2d;
@group(2) @binding(4) var sun_compare: sampler_comparison;

const GROUND = 0u;
const SHELL = 1u;
const FACE = 2u;
const CLAW = 3u;
const LEG = 4u;
const TAU: f32 = 6.2831855;
const LUMA = vec3f(0.2126, 0.7152, 0.0722);

fn row_vector(i: u32) -> vec3f {
    return vec3f(hermit_words[i], hermit_words[i + 1u], hermit_words[i + 2u]);
}
fn view_ray(pixel: vec2f) -> vec3f {
    let xy = vec2f(pixel.x / f32(hermit_camera.size.x) * 2.0 - 1.0,
                   1.0 - pixel.y / f32(hermit_camera.size.y) * 2.0);
    let far = hermit_camera.inverse_projection * vec4f(xy, 1, 1);
    return normalize(far.xyz / far.w - hermit_camera.eye.xyz);
}
fn wave(a: f32) -> f32 { return 0.5 + 0.5 * sin(a); }
fn noise_corner(i: vec3i) -> f32 {
    let j = vec3u(i);
    var h = (j.x * 73856093u ^ j.y * 19349663u) ^ j.z * 83492791u;
    h = (h ^ (h >> 13u)) * 1274126177u;
    return f32((h ^ (h >> 16u)) & 0xffffffu) / 16777216.0;
}
fn noise_plane(i: vec3i, w: vec2f) -> f32 {
    let lower = mix(noise_corner(i), noise_corner(i + vec3i(1, 0, 0)), w.x);
    let upper = mix(noise_corner(i + vec3i(0, 1, 0)), noise_corner(i + vec3i(1, 1, 0)), w.x);
    return mix(lower, upper, w.y);
}
fn value_noise(p: vec3f) -> f32 {
    let base = floor(p);
    let d = p - base;
    let w = d * d * (vec3f(3) - 2.0 * d);
    let i = vec3i(base);
    return mix(noise_plane(i, w.xy), noise_plane(i + vec3i(0, 0, 1), w.xy), w.z);
}
fn fbm(p: vec3f) -> f32 {
    return 0.667 * value_noise(p) + 0.333 * value_noise(p * 2.03 + vec3f(1.7, 9.2, 3.1));
}

fn distance_box(p: vec3f, half_size: vec3f) -> f32 {
    let d = abs(p) - half_size;
    return length(max(d, vec3f(0))) + min(max(d.x, max(d.y, d.z)), 0.0);
}
fn rounded_box(p: vec3f, half_size: vec3f, r: f32) -> f32 {
    return distance_box(p, half_size - vec3f(r)) - r;
}
fn capsule_distance(p: vec3f, a: vec3f, b: vec3f, r: f32) -> f32 {
    let ap = p - a;
    let ab = b - a;
    let u = clamp(dot(ap, ab) / max(dot(ab, ab), 1e-12), 0.0, 1.0);
    return length(ap - ab * u) - r;
}
fn smooth_union(a: f32, b: f32, k: f32) -> f32 {
    let h = max(k - abs(a - b), 0.0) / k;
    return min(a, b) - h * h * k * 0.25;
}
fn face_distance(p: vec3f, aggro: f32) -> f32 {
    let q = vec3f(abs(p.x), p.yz) + vec3f(-0.15, -0.72, -0.42);
    let shell = rounded_box(p - vec3f(0, 0.58, 0.12), vec3f(0.40, 0.18, 0.34), 0.15);
    let lip = capsule_distance(p, vec3f(-0.22, 0.52, 0.44), vec3f(0.22, 0.52, 0.44), 0.09);
    let eye = vec3f(0.03, 0.30, 0.07);
    let stalk = capsule_distance(q, vec3f(0), eye, 0.045);
    let globe = length(q - eye) - 0.085;
    let b = q + (-vec3f(0, 0.40, 0.02));
    let s = sin(-0.35 * aggro);
    let c = cos(-0.35 * aggro);
    let brow = vec3f(c * b.x + s * b.y, c * b.y - s * b.x, b.z);
    let plate = rounded_box(brow, vec3f(0.11, 0.025, 0.07), 0.02);
    let feeler = capsule_distance(q, vec3f(-0.05, 0.02, 0.06), vec3f(0.32, 0.55, 0.95), 0.016);
    let eyes = smooth_union(smooth_union(stalk, globe, 0.03), plate, 0.02);
    return min(smooth_union(smooth_union(shell, lip, 0.08), eyes, 0.05), feeler);
}
fn claw_distance(p: vec3f, scale: f32, jaw_angle: f32) -> f32 {
    let q = p / scale;
    let arm = capsule_distance(q, vec3f(0, 0, -0.75), vec3f(0, 0.02, -0.05), 0.13);
    let palm = rounded_box(q - vec3f(0, 0, 0.32), vec3f(0.30, 0.24, 0.42), 0.16);
    let cut = distance_box(q - vec3f(0, -0.34, 0.80), vec3f(0.42, 0.16, 0.30));
    let fixed = capsule_distance(q, vec3f(0, -0.10, 0.60), vec3f(0, -0.06, 1.45), 0.11);
    let a = q + (-vec3f(0, 0.10, 0.60));
    let s = sin(-jaw_angle);
    let c = cos(-jaw_angle);
    let jaw = vec3f(a.x, c * a.y + s * a.z, c * a.z - s * a.y);
    let t = clamp((jaw.z - 0.1) / (0.9f - 0.1f), 0.0, 1.0);
    let teeth = 0.025 * sin(14.0 * jaw.z) * (t * t * t * ((6.0 * t - 15.0) * t + 10.0));
    let finger = capsule_distance(jaw, vec3f(0), vec3f(0, -0.05, 0.95), 0.1) - teeth;
    return scale * smooth_union(smooth_union(smooth_union(arm, -smooth_union(-palm, cut, 0.05), 0.08), fixed, 0.06), finger, 0.05);
}
fn hermit_field(kind: u32, row: u32, p: vec3f) -> f32 {
    if kind == GROUND {
        let r = hermit_words[row];
        return distance_box(p - vec3f(0, -0.05, 0), vec3f(r, 0.05, r));
    }
    if kind == FACE { return face_distance(p, hermit_words[row + 12u]); }
    if kind == CLAW { return claw_distance(p, hermit_words[row + 13u], hermit_words[row + 12u]); }
    let a = row_vector(row + 12u);
    let b = row_vector(row + 15u);
    let c = row_vector(row + 18u);
    let d = row_vector(row + 21u);
    return min(min(capsule_distance(p, a, b, 0.075), capsule_distance(p, b, c, 0.065)),
        capsule_distance(p, c, d, 0.045));
}

struct FieldSurface { normal: vec3f, occlusion: f32 }
fn sample_field(kind: u32, row: u32, p: vec3f, t: f32) -> FieldSurface {
    const tetra = array(vec3f(1, -1, -1), vec3f(-1, -1, 1), vec3f(-1, 1, -1), vec3f(1, 1, 1));
    let epsilon = 5e-4 * max(1.0, 0.25 * t);
    var gradient = vec3f(0);
    var normal = vec3f(0, 1, 0);
    var occlusion = 0.0;
    var weight = 1.0;
    // Evaluate the large field at one site for both the four normal taps and five AO taps.
    for (var i = 0u; i < 9u; i++) {
        var q = p + tetra[min(i, 3u)] * epsilon;
        var h = 0.0;
        if i >= 4u {
            if i == 4u { normal = normalize(gradient); }
            h = 0.02 + 0.06 * f32(i - 4u);
            q = p + normal * h;
        }
        let d = hermit_field(kind, row, q);
        if i < 4u {
            gradient += tetra[i] * d;
        } else {
            occlusion += (h - d) * weight;
            weight *= 0.8;
        }
    }
    return FieldSurface(normal, clamp(1.0 - 2.0 * occlusion, 0.0, 1.0));
}

struct ShellBasis { origin: vec3f, normal: vec3f, binormal: vec3f }
fn spline_component(i: i32, offset: u32) -> vec3f {
    let word = u32(clamp(i, 0, 255)) * 12u + offset;
    return vec3f(shade_tables[word], shade_tables[word + 1u], shade_tables[word + 2u]);
}
fn blend_component(i: i32, o: u32, w: vec4f) -> vec3f {
    return spline_component(i - 1, o) * w.x + spline_component(i, o) * w.y
        + spline_component(i + 1, o) * w.z + spline_component(i + 2, o) * w.w;
}
fn blend_basis(i: i32, w: vec4f) -> ShellBasis {
    return ShellBasis(blend_component(i, 0u, w), blend_component(i, 6u, w), blend_component(i, 9u, w));
}
fn rib(u: f32, angle: f32) -> f32 {
    let c = 0.5 + 0.5 * cos(28.0 * u);
    return 0.06 * c * c * c * c * (0.6 + 0.4 * cos(5.0 * angle));
}
fn shell_tangent(b: ShellBasis, db: ShellBasis, u: f32, angle: f32, du: f32, dv: f32) -> vec3f {
    let a = TAU * dv;
    let e = exp(2.6 * (u - 1.0));
    let r = 0.26 * e;
    let dr = 0.26 * (e * (2.6 * du));
    let k = rib(u, angle);
    let c = 0.5 + 0.5 * cos(28.0 * u);
    let dc = 0.5 * (-sin(28.0 * u) * (28.0 * du));
    let c2 = (0.06 * c) * c;
    let dc2 = (0.06 * dc) * c + (0.06 * c) * dc;
    let dc4 = (dc2 * c + c2 * dc) * c + (c2 * c) * dc;
    let dk = dc4 * (0.6 + 0.4 * cos(5.0 * angle))
        + (c2 * c * c) * (0.4 * (-sin(5.0 * angle) * (5.0 * a)));
    let x = r * (1.0 + k);
    let y = r * (0.9 + k);
    let dx = (dr * (1.0 + k) + r * dk) * cos(angle) + x * (-sin(angle) * a);
    let dy = (dr * (0.9 + k) + r * dk) * sin(angle) + y * (cos(angle) * a);
    return db.origin + (db.normal * (x * cos(angle)) + b.normal * dx)
        + (db.binormal * (y * sin(angle)) + b.binormal * dy);
}
struct ShellSurface { point: vec3f, normal: vec3f }
fn shell_surface(uv: vec2f) -> ShellSurface {
    let s = uv.x * 255.0;
    let i = clamp(floor(s), 0.0, 255.0);
    let f = s - i;
    let g = 1.0 - f;
    let b = blend_basis(i32(i), vec4f(g * g * g / 6.0, (3.0 * f * f * f - 6.0 * f * f + 4.0) / 6.0,
        (-3.0 * f * f * f + 3.0 * f * f + 3.0 * f + 1.0) / 6.0, f * f * f / 6.0));
    let dw = vec4f(-g * g / 2.0, (3.0 * f * f - 4.0 * f) / 2.0, (-3.0 * f * f + 2.0 * f + 1.0) / 2.0, f * f / 2.0);
    let a = TAU * uv.y;
    let du = shell_tangent(b, blend_basis(i32(i), dw * 255.0), uv.x, a, 1.0, 0.0);
    let dv = shell_tangent(b, blend_basis(i32(i), dw * 0.0), uv.x, a, 0.0, 1.0);
    let r = 0.26 * exp(2.6 * (uv.x - 1.0));
    let k = rib(uv.x, a);
    let p = b.origin + b.normal * (r * (1.0 + k) * cos(a)) + b.binormal * (r * (0.9 + k) * sin(a));
    let normal = cross(du, dv);
    return ShellSurface(p, normal * inverseSqrt(max(dot(normal, normal), 1e-30)));
}

struct Material {
    albedo: vec3f, roughness: f32,
    coat_weight: f32, coat_roughness: f32,
    film_weight: f32, scatter_color: vec3f, scatter_radius: f32, emission: vec3f,
}
fn ground_material(p: vec3f) -> Material {
    let ripple = wave(5.0 * p.x + 4.0 * fbm(p * 0.4));
    let color = mix(vec3f(0.36, 0.31, 0.23), vec3f(0.52, 0.45, 0.34), 0.75 * fbm(p * 6.0) + 0.25 * ripple);
    return Material(color, 0.9, 0, 0, 0, vec3f(0), 0, vec3f(0));
}
fn shell_material(p: vec3f, uv: vec2f, tint: vec3f) -> Material {
    let a = TAU * uv.y;
    let band = wave(12.0 * uv.x + 3.0 * sin(a));
    let mottle = fbm(p * 7.0);
    let base = mix(vec3f(0.80, 0.62, 0.42), vec3f(0.42, 0.20, 0.09), band * 0.7 + 0.3 * mottle) * tint;
    let grime = saturate(1.0 - rib(uv.x, a) * 20.0) * smoothstep(0.35, 0.75, mottle);
    let lip = smoothstep(0.93, 1.0, uv.x);
    return Material(mix(base, vec3f(0.12, 0.16, 0.10), grime * 0.6), 0.45 + 0.3 * grime,
        0.5 * (1.0 - grime), 0.12, 0.08 + 0.6 * lip, vec3f(0), 0, vec3f(0));
}
fn face_material(p: vec3f, rotation: mat3x3f, position: vec3f, row: u32) -> Material {
    let mirror = vec3f(select(-1.0, 1.0, p.x >= 0.0), 1, 1);
    let offset = vec3f(-0.15, -0.72, -0.42);
    let q = vec3f(abs(p.x), p.yz) + offset;
    let eye = vec3f(0.03, 0.30, 0.07);
    let eye_world = rotation * ((eye - offset) * mirror) + position;
    let look = transpose(rotation) * normalize(row_vector(row + 13u) - eye_world) * mirror;
    let eye_mask = 1.0 - smoothstep(0.088, 0.095, distance(q, eye));
    let pupil = smoothstep(0.80, 0.90, dot(normalize(q - eye), look)) * eye_mask;
    let skin = mix(vec3f(0.55, 0.12, 0.05), vec3f(0.85, 0.42, 0.18), smoothstep(0.45, 0.8, p.y))
        * row_vector(row + 16u) * (0.75 + 0.5 * fbm(p * 11.0));
    let iris = mix(vec3f(0.85, 0.62, 0.15), vec3f(0.01, 0.01, 0.012), pupil);
    return Material(mix(skin, iris, eye_mask), mix(0.5, 0.2, eye_mask),
        0.35 + 0.65 * eye_mask, 0.05, 0, vec3f(0.9, 0.35, 0.15), 0.12 * (1.0 - eye_mask), vec3f(0));
}
fn claw_material(p: vec3f, row: u32) -> Material {
    let q = p / hermit_words[row + 13u];
    let pit = fbm(q * 9.0);
    let tip = smoothstep(1.0, 1.45, q.z);
    let base = mix(vec3f(0.62, 0.11, 0.05), vec3f(0.08, 0.03, 0.02), tip)
        * row_vector(row + 15u) * (0.8 + 0.4 * pit);
    let vein = smoothstep(0.70, 0.78, fbm(q * 4.0)) * hermit_words[row + 14u] * (1.0 - tip);
    return Material(base, 0.22 + 0.25 * pit, 0.85, 0.06, 0, vec3f(0.9, 0.25, 0.08), 0.1,
        vec3f(1, 0.32, 0.04) * vein * 0.8);
}
fn leg_material(p: vec3f, row: u32) -> Material {
    let band = smoothstep(0.35, 0.65, wave(9.0 * distance(p, row_vector(row + 12u))));
    let base = mix(vec3f(0.62, 0.14, 0.05), vec3f(0.88, 0.55, 0.30), band * 0.5) * row_vector(row + 24u);
    return Material(base, 0.35, 0.7, 0.07, 0, vec3f(0.9, 0.3, 0.1), 0.08, vec3f(0));
}

fn energy_word(i: u32) -> f32 {
    return shade_tables[arrayLength(&shade_tables) - 1122u + i];
}
fn directional_energy(roughness: f32, cosine: f32) -> vec2f {
    let r = clamp(roughness, 0.0, 1.0) * 32.0;
    let angle = sqrt(clamp(cosine, 0.0, 1.0)) * 32.0;
    let i = min(u32(r), 31u);
    let j = min(u32(angle), 31u);
    let base = i * 34u;
    let b = angle - f32(j);
    let low = mix(energy_word(base + j), energy_word(base + j + 1u), b);
    let high = mix(energy_word(base + 34u + j), energy_word(base + 34u + j + 1u), b);
    return mix(vec2f(low, energy_word(base + 33u)), vec2f(high, energy_word(base + 67u)), r - f32(i)) / 65535.0;
}
fn schlick(f0: vec3f, cosine: f32) -> vec3f {
    let x = 1.0 - clamp(cosine, 0.0, 1.0);
    let xx = x * x;
    return f0 + (vec3f(1) - f0) * (xx * xx * x);
}
fn smith_ggx(nh: f32, nv: f32, nl: f32, roughness: f32) -> f32 {
    let a = max(roughness * roughness, 0.002);
    let a2 = a * a;
    let q = (1.0 - nh * nh) + nh * nh * a2;
    let gv = nl * sqrt(a2 + (1.0 - a2) * nv * nv);
    let gl = nv * sqrt(a2 + (1.0 - a2) * nl * nl);
    return a2 / (6.2831855 * q * q * max(gv + gl, 1e-8));
}
fn reflect_ggx(nh: f32, nv: f32, nl: f32, rough: f32, fh: vec3f, average: vec3f, outgoing: vec2f) -> vec3f {
    let ei = directional_energy(rough, nl).x;
    let ea = outgoing.y;
    let lost = 1.0 - ea;
    let tint = average * average * ea / max(vec3f(1) - average * lost, vec3f(1e-6));
    let multi = tint * ((1.0 - outgoing.x) * (1.0 - ei) * 0.31830987 / max(lost, 1e-6));
    return (fh * smith_ggx(nh, nv, nl, rough) + multi) * nl;
}
fn interference(ior: f32, thickness: f32, mu: f32) -> vec3f {
    let ct = sqrt(max(1.0 - (1.0 - mu * mu) / (ior * ior), 0.0));
    let f = (ior - 1.0) / (ior + 1.0);
    let r0 = f * f;
    let phase = 12.566371 * ior * thickness * ct;
    let c = vec3f(cos(phase / 650.0), cos(phase / 532.0), cos(phase / 450.0));
    return 2.0 * r0 * (vec3f(1) - c) / (vec3f(1.0 + r0 * r0) - 2.0 * r0 * c);
}
fn shadow(p: vec3f, n: vec3f) -> f32 {
    if hermit_camera.flags.z == 0u { return 1.0; }
    let q = hermit_camera.light_projection * vec4f(p + n * (1.5 * hermit_camera.sun.w), 1);
    let uv = vec2f(q.x * 0.5 + 0.5, 0.5 - q.y * 0.5);
    if any(uv < vec2f(0)) || any(uv > vec2f(1)) || q.z > 1.0 || q.z < 0.0 { return 1.0; }
    let texel = 1.0 / f32(textureDimensions(sun_map).x);
    let depth = q.z - 0.0005;
    var sum = 0.0;
    // Keep the x-major nine-tap accumulation order and expose independent lookups.
    sum += textureSampleCompareLevel(sun_map, sun_compare, uv + vec2f(-1, -1) * texel, depth);
    sum += textureSampleCompareLevel(sun_map, sun_compare, uv + vec2f(-1,  0) * texel, depth);
    sum += textureSampleCompareLevel(sun_map, sun_compare, uv + vec2f(-1,  1) * texel, depth);
    sum += textureSampleCompareLevel(sun_map, sun_compare, uv + vec2f( 0, -1) * texel, depth);
    sum += textureSampleCompareLevel(sun_map, sun_compare, uv + vec2f( 0,  0) * texel, depth);
    sum += textureSampleCompareLevel(sun_map, sun_compare, uv + vec2f( 0,  1) * texel, depth);
    sum += textureSampleCompareLevel(sun_map, sun_compare, uv + vec2f( 1, -1) * texel, depth);
    sum += textureSampleCompareLevel(sun_map, sun_compare, uv + vec2f( 1,  0) * texel, depth);
    sum += textureSampleCompareLevel(sun_map, sun_compare, uv + vec2f( 1,  1) * texel, depth);
    return sum / 9.0;
}
const SKY_DIRECTIONS = array(
    vec3f(0.3996526002883911, 0.9166666865348816, 0.0),
    vec3f(-0.48772361874580383, 0.75, 0.44679486751556396),
    vec3f(0.07100989669561386, 0.5833333730697632, -0.8091228604316711),
    vec3f(0.5531073808670044, 0.4166666865348816, 0.7214298248291016),
    vec3f(-0.9534448385238647, 0.25, -0.16865061223506927),
    vec3f(0.8408204317092896, 0.08333331346511841, -0.5348613262176514),
    vec3f(-0.25870034098625183, -0.08333337306976318, 0.9623563289642334),
    vec3f(-0.4462720453739166, -0.25, -0.8592678904533386),
    vec3f(0.8538990616798401, -0.4166666269302368, 0.3118418753147125),
    vec3f(-0.7507836222648621, -0.5833333730697632, 0.3099130690097809),
    vec3f(0.2803475260734558, -0.75, -0.5990869998931885),
    vec3f(0.11960970610380173, -0.9166666269302368, 0.38133421540260315)
);
fn sky_radiance(l: vec3f) -> vec3f {
    let up = clamp(l.y, -1.0, 1.0);
    let sky = mix(environment[2].xyz, environment[1].xyz, sqrt(max(up, 0.0)));
    let ground = mix(vec3f(0.24, 0.21, 0.16), vec3f(0.12, 0.10, 0.08), sqrt(max(-up, 0.0)));
    return select(ground, sky, up >= 0.0 || environment[1].w != 0.0);
}

struct Lighting {
    nv: f32, base_fv: vec3f, base_energy: vec2f,
    wrap: f32, coat_energy: vec2f, film_energy: vec2f, film_fv: vec3f, film_average: vec3f,
}

fn sample_light(kind: u32, m: Material, n: vec3f, v: vec3f, l: vec3f, view: Lighting) -> vec3f {
    let f0 = vec3f(0.04);
    let nv = view.nv;
    let base_fv = view.base_fv;
    let base_energy = view.base_energy;
    let wrap = view.wrap;
    let coat_energy = view.coat_energy;
    let film_energy = view.film_energy;
    let film_fv = view.film_fv;
    let film_average = view.film_average;
    let li = dot(l, l);
    let ndotl = dot(n, l);
    if ndotl <= -wrap && li > 0.0 { return vec3f(0); }
    let nl = max(ndotl, 0.0);
    var spec = vec3f(0);
    if nl > 0.0 && nv > 0.0 {
        let h = normalize(v + l);
        let fh = schlick(f0, dot(v, h));
        spec = reflect_ggx(clamp(dot(n, h), 0.0, 1.0), nv, nl, m.roughness, fh,
            (vec3f(1) + 20.0 * f0) / 21.0, base_energy);
    }
    var c = m.albedo * (vec3f(1) - base_fv) * (0.31830987 * nl) + spec;
    if kind == SHELL {
        var film = vec3f(0);
        if ndotl > 0.0 && nv > 0.0 {
            let h = normalize(v + l);
            let fh = interference(1.42, 380.0, clamp(dot(v, h), 0.0, 1.0));
            film = reflect_ggx(clamp(dot(n, h), 0.0, 1.0), nv, ndotl, 0.3, fh, film_average, film_energy);
        }
        c = c * (1.0 - m.film_weight * film_energy.x * dot(film_fv, LUMA)) + m.film_weight * film;
    }
    if kind == FACE || kind == CLAW || kind == LEG {
        let wrapped = max(ndotl + wrap, 0.0) / ((1.0 + wrap) * (1.0 + wrap));
        c += m.scatter_color * (0.31830988 * wrapped * min(li, 1.0) * wrap);
    }
    if kind != GROUND {
        var coat = vec3f(0);
        if ndotl > 0.0 && nv > 0.0 {
            let h = normalize(v + l);
            let fh = schlick(f0, dot(v, h));
            coat = reflect_ggx(clamp(dot(n, h), 0.0, 1.0), nv, ndotl, m.coat_roughness, fh,
                vec3f(1.8 / 21.0), coat_energy);
        }
        c = c * (1.0 - m.coat_weight * coat_energy.x * base_fv.x) + m.coat_weight * coat;
    }
    var emission = vec3f(0);
    if kind == CLAW { emission = m.emission * (1.0 - min(li, 1.0)); }
    return c + emission;
}

fn shade_lighting(kind: u32, m: Material, p: vec3f, n: vec3f, v: vec3f, ao: f32) -> vec3f {
    let sun = normalize(hermit_camera.sun.xyz);
    let direct = shadow(p, n);
    let nv = max(dot(n, v), 0.0);
    let f0 = vec3f(0.04);
    let base_fv = schlick(f0, nv);
    let base_energy = directional_energy(m.roughness, nv);
    let wrap = clamp(m.scatter_radius * 4.0, 0.0, 1.0);
    let coat_energy = directional_energy(m.coat_roughness, nv);
    let film_energy = directional_energy(0.3, nv);
    let film_fv = interference(1.42, 380.0, nv);
    let film_average = (5.0 * interference(1.42, 380.0, 0.335711)
        + 8.0 * interference(1.42, 380.0, 0.707107)
        + 5.0 * interference(1.42, 380.0, 0.941965)) / 18.0;
    let view = Lighting(nv, base_fv, base_energy, wrap, coat_energy, film_energy, film_fv, film_average);
    var total = vec3f(0);
    for (var sample = 0u; sample < 14u; sample++) {
        var l = vec3f(0);
        var e = vec3f(1);
        if sample == 1u {
            l = sun;
            e = environment[0].xyz * direct;
        }
        if sample >= 2u {
            l = SKY_DIRECTIONS[sample - 2u];
            e = sky_radiance(l) * (0.85 * 1.0471976 * ao);
        }
        total += sample_light(kind, m, n, v, l, view) * e;
    }
    return total;
}

fn shade_tile(kind: u32, stride: u32, group: vec3u, lane: vec3u) {
    if hermit_camera.flags.x == 0xffffffffu { return; }
    let tile = classified_tiles[hermit_rows.kind * hermit_camera.size.w + group.x];
    let pixel = vec2u(tile & 0xffffu, tile >> 16u) * 8u + lane.xy;
    if any(pixel >= hermit_camera.size.xy) { return; }
    let record = textureLoad(visibility, pixel, 0).xy;
    if record.x == 0xffffffffu || record.x >> 24u != hermit_rows.kind { return; }

    let row = (record.x & 0xffffffu) * stride;
    let ray = view_ray(vec2f(pixel) + 0.5);
    var rotation = mat3x3f(1, 0, 0, 0, 1, 0, 0, 0, 1);
    var translation = vec3f(0);
    if kind != GROUND {
        rotation = mat3x3f(row_vector(row), row_vector(row + 3u), row_vector(row + 6u));
        translation = row_vector(row + 9u);
    }
    var p: vec3f;
    var n: vec3f;
    var ao = 1.0;
    var material: Material;
    if kind == SHELL {
        let uv = vec2f(f32(record.y & 0xffffu), f32(record.y >> 16u)) / 65535.0;
        let surface = shell_surface(uv);
        p = rotation * surface.point + translation;
        n = normalize(rotation * surface.normal);
        if dot(n, ray) > 0.0 { n = -n; }
        material = shell_material(surface.point, uv, row_vector(row + 12u));
    } else {
        let t = bitcast<f32>(record.y);
        p = hermit_camera.eye.xyz + ray * t;
        let inverse = transpose(rotation);
        let local = inverse * p + -(inverse * translation);
        let surface = sample_field(kind, row, local, t);
        n = normalize(rotation * surface.normal);
        ao = surface.occlusion;
        if kind == GROUND { material = ground_material(local); }
        if kind == FACE { material = face_material(local, rotation, translation, row); }
        if kind == CLAW { material = claw_material(local, row); }
        if kind == LEG { material = leg_material(local, row); }
    }
    textureStore(radiance, pixel, vec4f(shade_lighting(kind, material, p, n, -ray, ao), 1));
}

@compute @workgroup_size(8, 8) fn ground_shade(@builtin(workgroup_id) group: vec3u, @builtin(local_invocation_id) lane: vec3u) {
    shade_tile(GROUND, 8u, group, lane);
}
@compute @workgroup_size(8, 8) fn shell_shade(@builtin(workgroup_id) group: vec3u, @builtin(local_invocation_id) lane: vec3u) {
    shade_tile(SHELL, 22u, group, lane);
}
@compute @workgroup_size(8, 8) fn face_shade(@builtin(workgroup_id) group: vec3u, @builtin(local_invocation_id) lane: vec3u) {
    shade_tile(FACE, 26u, group, lane);
}
@compute @workgroup_size(8, 8) fn claw_shade(@builtin(workgroup_id) group: vec3u, @builtin(local_invocation_id) lane: vec3u) {
    shade_tile(CLAW, 25u, group, lane);
}
@compute @workgroup_size(8, 8) fn leg_shade(@builtin(workgroup_id) group: vec3u, @builtin(local_invocation_id) lane: vec3u) {
    shade_tile(LEG, 34u, group, lane);
}
