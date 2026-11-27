//! Integration of the joint medium and shared directional GGX albedo data.
pub use crate::paths::cpu::{Coefficients, Vol, inscatter_joint};
use std::fmt::Write as _;

const RES: usize = 32;
const PITCH: usize = RES + 2;
const ENERGY: [u16; (RES + 1) * PITCH] = include!("bsdf/energy.in");

/// Directional and cosine-averaged single-scattering albedo for unit Fresnel.
pub fn ggx_energy(roughness: f32, mu: f32) -> [f32; 2] {
    let r = roughness.clamp(0., 1.) * RES as f32;
    let v = mu.clamp(0., 1.).sqrt() * RES as f32;
    let (i, j) = ((r as usize).min(RES - 1), (v as usize).min(RES - 1));
    let (a, b) = (r - i as f32, v - j as f32);
    let e = |row, col| f32::from(ENERGY[row * PITCH + col]) / 65535.;
    let lerp = |x, y, t| (1. - t) * x + t * y;
    let directional = |row| lerp(e(row, j), e(row, j + 1), b);
    [
        lerp(directional(i), directional(i + 1), a),
        lerp(e(i, RES + 1), e(i + 1, RES + 1), a),
    ]
}

pub fn energy_words() -> impl Iterator<Item = f32> {
    ENERGY.into_iter().map(f32::from)
}

pub fn energy_wgsl(table: Option<&str>) -> String {
    let mut wgsl = String::new();
    if let Some(table) = table {
        writeln!(wgsl, "fn loft_ggx_word(i: u32) -> f32 {{ return {table}[arrayLength(&{table}) - {}u + i]; }}", ENERGY.len()).unwrap();
    } else {
        wgsl = format!("const loft_ggx_albedo = array<u32, {}>(\n", ENERGY.len());
        for row in ENERGY.chunks_exact(PITCH) {
            for value in row {
                write!(wgsl, "{value}u,").unwrap();
            }
            wgsl.push('\n');
        }
        wgsl.push_str(");\nfn loft_ggx_word(i: u32) -> f32 { return f32(loft_ggx_albedo[i]); }\n");
    }
    wgsl.push_str(
        "fn loft_ggx_energy(roughness: f32, mu: f32) -> vec2<f32> {\n\
            let r = clamp(roughness, 0.0, 1.0) * 32.0;\n\
            let v = sqrt(clamp(mu, 0.0, 1.0)) * 32.0;\n\
            let i = min(u32(r), 31u);\n\
            let j = min(u32(v), 31u);\n\
            let base = i * 34u;\n\
            let b = v - f32(j);\n\
            let e0 = mix(loft_ggx_word(base+j), loft_ggx_word(base+j+1u), b);\n\
            let e1 = mix(loft_ggx_word(base+34u+j), loft_ggx_word(base+34u+j+1u), b);\n\
            return mix(vec2(e0, loft_ggx_word(base+33u)), vec2(e1, loft_ggx_word(base+67u)), r - f32(i)) / 65535.0;\n\
        }\n",
    );
    wgsl
}

/// Bind the world lowering's summed sampler as
/// `fn loft_sample_medium(o: vec3<f32>, d: vec3<f32>, t: f32, step: u32, rng: vec4<u32>) -> LoftCoefficients`.
/// The caller supplies a normalized ray, clipped interval, and positive step count.
pub const INSCATTER_WGSL: &str = concat!(include_str!("bsdf/volume_step.wgsl"), include_str!("bsdf/inscatter.wgsl"));
pub const VOLUME_STEP_WGSL: &str = include_str!("bsdf/volume_step.wgsl");
