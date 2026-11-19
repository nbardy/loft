//! Integration of the joint medium and shared directional GGX albedo data.
use std::{fmt::Write as _, num::NonZeroU32};

const RES: usize = 32;
const PITCH: usize = RES + 2;
const ENERGY: [u16; (RES + 1) * PITCH] = include!("bsdf/energy.in");

/// Directional and cosine-averaged single-scattering albedo for unit Fresnel.
pub(crate) fn ggx_energy(roughness: f32, mu: f32) -> [f32; 2] {
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

pub(crate) fn energy_words() -> impl Iterator<Item = f32> {
    ENERGY.into_iter().map(f32::from)
}

pub(crate) fn energy_wgsl(table: Option<&str>) -> String {
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

/// The summed world-medium coefficients at a ray sample. Source is radiance
/// per metre (emission plus illuminated scattering, including the phase).
#[derive(Clone, Copy, Debug, Default)]
pub struct Coefficients {
    pub extinction: [f32; 3],
    pub source: [f32; 3],
}

impl std::ops::Add for Coefficients {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            extinction: std::array::from_fn(|i| self.extinction[i] + other.extinction[i]),
            source: std::array::from_fn(|i| self.source[i] + other.source[i]),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Vol {
    pub l: [f32; 3],
    pub t: [f32; 3],
}

/// Midpoint quadrature over an already clipped ray interval in metres.
/// Integrate each constant-coefficient segment exactly. A ray's incident
/// radiance behind the medium is composed as `vol.l + vol.t * background`.
pub fn inscatter_joint(
    begin: f32,
    end: f32,
    steps: NonZeroU32,
    mut sample: impl FnMut(f32) -> Coefficients,
) -> Vol {
    let mut vol = Vol {
        l: [0.; 3],
        t: [1.; 3],
    };
    if end <= begin {
        return vol;
    }
    let ds = (end - begin) / steps.get() as f32;
    for k in 0..steps.get() {
        let medium = sample(begin + (k as f32 + 0.5) * ds);
        for (i, sigma) in medium.extinction.into_iter().enumerate() {
            let tau = sigma * ds;
            let tr = (-tau).exp();
            // Keep the vacuum limit and avoid subtracting nearly equal f32 values.
            let weight = if tau < 0.001 {
                ds * (1. - tau * (0.5 - tau / 6.))
            } else {
                (1. - tr) / sigma
            };
            vol.l[i] += vol.t[i] * medium.source[i] * weight;
            vol.t[i] *= tr;
        }
    }
    vol
}

/// Bind the world lowering's summed sampler as
/// `fn loft_sample_medium(o: vec3<f32>, d: vec3<f32>, t: f32, step: u32, rng: vec4<u32>) -> LoftCoefficients`.
/// The caller supplies a normalized ray, clipped interval, and positive step count.
pub const INSCATTER_WGSL: &str = concat!(include_str!("bsdf/volume_step.wgsl"), include_str!("bsdf/inscatter.wgsl"));
pub(crate) const VOLUME_STEP_WGSL: &str = include_str!("bsdf/volume_step.wgsl");
