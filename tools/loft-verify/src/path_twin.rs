//! f32 twin of the path loop and direct reservoir. Geometry, media and staged
//! material sampling are supplied by the same World boundary as the shader.

use std::num::NonZeroU32;

pub type Rgb = [f32; 3];

#[derive(Clone, Copy)]
pub struct Ray {
    pub o: Rgb,
    pub d: Rgb,
}

#[derive(Clone, Copy)]
pub struct Hit {
    pub p: Rgb,
    pub n: Rgb,
}

pub trait World {
    fn trace(&self, ray: Ray) -> Option<Hit>;
    fn sky(&self, d: Rgb) -> Rgb;
    fn eval(&self, hit: Hit, wo: Rgb, wi: Rgb) -> Rgb;
    fn direct(&self, hit: Hit, wo: Rgb, seed: u32, bounce: u32) -> Rgb;
}

fn add(a: Rgb, b: Rgb) -> Rgb { std::array::from_fn(|i| a[i] + b[i]) }
fn mul(a: Rgb, s: f32) -> Rgb { a.map(|x| x * s) }
fn dot(a: Rgb, b: Rgb) -> f32 { a[0]*b[0] + a[1]*b[1] + a[2]*b[2] }
fn cross(a: Rgb, b: Rgb) -> Rgb {
    [a[1]*b[2] - a[2]*b[1], a[2]*b[0] - a[0]*b[2], a[0]*b[1] - a[1]*b[0]]
}

pub fn hash(key: u32) -> u32 {
    let h = (key ^ (key >> 16)).wrapping_mul(0x7feb352d);
    let h = (h ^ (h >> 15)).wrapping_mul(0x846ca68b);
    h ^ (h >> 16)
}

pub fn random(key: u32, i: u32) -> f32 { (hash(key ^ hash(i)) >> 8) as f32 / 16_777_216.0 }

fn cosine(n: Rgb, key: u32, bounce: u32) -> Rgb {
    let u = random(key, bounce * 16);
    let phi = std::f32::consts::TAU * random(key, bounce * 16 + 1);
    let axis = if n[1].abs() > 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let x = crate::unit(cross(axis, n));
    let y = cross(n, x);
    crate::unit(add(add(mul(x, u.sqrt()*phi.cos()), mul(y, u.sqrt()*phi.sin())), mul(n, (1.0-u).sqrt())))
}

/// Bounces counts continuations after the initial hit. The return is linear
/// radiance, including the final hit's emission and direct lighting.
pub fn sample(world: &impl World, mut ray: Ray, seed: u32, bounces: u32) -> Rgb {
    let mut throughput = [1.0; 3];
    let mut radiance = [0.0; 3];
    for bounce in 0..=bounces {
        let Some(hit) = world.trace(ray) else {
            let sky = world.sky(ray.d);
            for i in 0..3 { radiance[i] += throughput[i] * sky[i]; }
            break;
        };
        let wo = mul(ray.d, -1.0);
        let light = add(world.eval(hit, wo, [0.0; 3]), world.direct(hit, wo, seed, bounce));
        for i in 0..3 { radiance[i] += throughput[i] * light[i]; }
        if bounce == bounces { break; }
        ray.d = cosine(hit.n, seed, bounce);
        let pdf = dot(hit.n, ray.d).max(0.0) * std::f32::consts::FRAC_1_PI;
        if pdf <= 0.0 { break; }
        let f_cos = world.eval(hit, wo, ray.d);
        for i in 0..3 { throughput[i] *= f_cos[i] / pdf; }
        if throughput.iter().all(|x| *x <= 0.0) { break; }
        ray.o = add(hit.p, mul(hit.n, 0.0002));
    }
    radiance
}

#[derive(Default)]
pub struct Reservoir {
    weights: f32,
    target: f32,
    pub count: u32,
    pub light: u32,
}

impl Reservoir {
    pub fn add(&mut self, light: u32, target: f32, weight: f32, count: u32, xi: f32) {
        self.weights += weight;
        self.count += count;
        if weight > 0.0 && xi * self.weights < weight {
            self.light = light;
            self.target = target;
        }
    }

    pub fn reuse(&mut self, source: &Self, target: f32, xi: f32) {
        let weight = if source.target > 0.0 { source.weights * target / source.target } else { 0.0 };
        self.add(source.light, target, weight, source.count, xi);
    }

    pub fn scale(&self) -> f32 {
        if self.target == 0.0 { 0.0 } else { self.weights / (self.target * self.count as f32) }
    }
}

#[derive(Clone, Copy)]
pub struct OpticalLayer {
    pub boundary: u32,
    pub ior: f32,
    pub absorb: f32,
}

pub struct MediumStack {
    at: usize,
    entries: [OpticalLayer; 4],
}

impl Default for MediumStack {
    fn default() -> Self {
        Self { at: 0, entries: [OpticalLayer { boundary: u32::MAX, ior: 1.0, absorb: 0.0 }; 4] }
    }
}

impl MediumStack {
    pub fn segment(&self, length: f32) -> f32 { (-self.entries[self.at].absorb * length).exp() }

    /// Compute the tentative transition. Only transmission accepts the new stack.
    pub fn interface(&mut self, layer: OpticalLayer, entering: bool, incoming_cos: f32, xi: f32) -> Option<(f32, bool)> {
        let next = if entering {
            let next = self.at + 1;
            if next == self.entries.len() { return None; }
            next
        } else {
            if self.at == 0 || self.entries[self.at].boundary != layer.boundary { return None; }
            self.at - 1
        };
        let eta = self.entries[self.at].ior / if entering { layer.ior } else { self.entries[next].ior };
        let ci = incoming_cos.clamp(0.0, 1.0);
        let sin2 = eta * eta * (1.0 - ci * ci);
        if sin2 >= 1.0 { return Some((eta, false)); }
        let ct = (1.0 - sin2).sqrt();
        let rs = (eta*ci - ct)/(eta*ci + ct);
        let rp = (ci - eta*ct)/(ci + eta*ct);
        let through = xi >= 0.5*(rs*rs + rp*rp);
        if through {
            if entering { self.entries[next] = layer; }
            self.at = next;
        }
        Some((eta, through))
    }
}

pub fn photon_fixed(power: f32) -> u32 { (power.clamp(0.0, 1.0) * 16384.0).round() as u32 }

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
