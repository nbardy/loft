//! The JS implementations of LibFn (src/lib/noise.ts, src/grad.ts,
//! src/soft/medium.ts and src/geom/solve.ts). The WGSL-only helpers are
//! callable here only as part of the JS twin that uses them.
use super::math;
use std::f64::consts::{SQRT_2, TAU};

#[derive(Clone, Copy)]
pub(super) enum Twin {
    Hash2,
    Hash3,
    Hash22,
    Hash33,
    Noise2,
    Noise3,
    Cellular2,
    Cellular3,
    Noise2Grad,
    Noise3Grad,
    MediumLightDepth,
    SolveJet,
}

impl Twin {
    pub(super) fn by_name(name: &str) -> Option<Self> {
        Some(match name {
            "loft_hash2" => Self::Hash2,
            "loft_hash3" => Self::Hash3,
            "loft_hash22" => Self::Hash22,
            "loft_hash33" => Self::Hash33,
            "loft_noise2" => Self::Noise2,
            "loft_noise3" => Self::Noise3,
            "loft_cellular2" => Self::Cellular2,
            "loft_cellular3" => Self::Cellular3,
            "loft_noise2_grad" => Self::Noise2Grad,
            "loft_noise3_grad" => Self::Noise3Grad,
            "loft_medium_light_depth" => Self::MediumLightDepth,
            "loft_solve_jet" => Self::SolveJet,
            _ => return None,
        })
    }

    pub(super) fn run(self, args: &[[f64; 4]]) -> [f64; 4] {
        let p = args[0];
        let [x, y, z, _] = p;
        match self {
            Self::Hash2 => [hash2([x, y]), 0.0, 0.0, 0.0],
            Self::Hash3 => [hash3([x, y, z]), 0.0, 0.0, 0.0],
            Self::Hash22 => {
                let [a, b] = hash22([x, y]);
                [a, b, 0.0, 0.0]
            }
            Self::Hash33 => {
                let [a, b, c] = hash33([x, y, z]);
                [a, b, c, 0.0]
            }
            Self::Noise2 => [noise2([x, y]), 0.0, 0.0, 0.0],
            Self::Noise3 => [noise3([x, y, z]), 0.0, 0.0, 0.0],
            Self::Cellular2 => cellular2([x, y]),
            Self::Cellular3 => {
                let [d1, d2] = cellular3([x, y, z]);
                [d1, d2, 0.0, 0.0]
            }
            Self::Noise2Grad => {
                let [gx, gy] = noise2_grad([x, y]);
                [gx, gy, 0.0, 0.0]
            }
            Self::Noise3Grad => {
                let [gx, gy, gz] = noise3_grad([x, y, z]);
                [gx, gy, gz, 0.0]
            }
            Self::MediumLightDepth => [medium_light_depth(p, args[1], args[2][0], args[3][0]), 0.0, 0.0, 0.0],
            Self::SolveJet => [args[1][0], 0.0, 0.0, 0.0],
        }
    }
}

// The CPU twin wraps lattice coordinates through i32 (WGSL saturates out of
// range). Within +-2^31 the PCG hash has identical integer arithmetic.
fn pcg(v: u32) -> u32 {
    let s = v.wrapping_mul(747796405).wrapping_add(2891336453);
    let w = ((s >> ((s >> 28) + 4)) ^ s).wrapping_mul(277803737);
    (w >> 22) ^ w
}

/// The top 24 bits are exactly representable as an f32 in [0, 1).
fn unit(h: u32) -> f64 {
    f64::from(h >> 8) / 16777216.0
}

fn lat(x: f64) -> u32 {
    math::to_uint32(x.floor())
}

fn h2u(x: u32, y: u32) -> u32 {
    pcg(x.wrapping_add(pcg(y)))
}

fn h3u(x: u32, y: u32, z: u32) -> u32 {
    pcg(x.wrapping_add(pcg(y.wrapping_add(pcg(z)))))
}

fn grad2(h: u32) -> [f64; 2] {
    let a = unit(h) * TAU;
    [a.cos(), a.sin()]
}

// Uniform on the sphere. (1-z)(1+z) keeps the radius accurate near the poles.
fn grad3(h: u32) -> [f64; 3] {
    let z = 1.0 - 2.0 * unit(h);
    let a = unit(pcg(h)) * TAU;
    let r = ((1.0 - z) * (1.0 + z)).sqrt();
    [r * a.cos(), r * a.sin(), z]
}

fn hash2([x, y]: [f64; 2]) -> f64 {
    unit(h2u(lat(x), lat(y)))
}

fn hash3([x, y, z]: [f64; 3]) -> f64 {
    unit(h3u(lat(x), lat(y), lat(z)))
}

fn hash22([x, y]: [f64; 2]) -> [f64; 2] {
    let h = h2u(lat(x), lat(y));
    [unit(h), unit(pcg(h))]
}

fn hash33([x, y, z]: [f64; 3]) -> [f64; 3] {
    let h = h3u(lat(x), lat(y), lat(z));
    let h1 = pcg(h);
    [unit(h), unit(h1), unit(pcg(h1))]
}

// With unit gradients the maxima are sqrt(2)/2 and sqrt(3)/2.
const SCALE2: f64 = SQRT_2;
const SCALE3: f64 = 2.0 / 1.732_050_807_568_877_2;

fn quintic(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn noise2(p: [f64; 2]) -> f64 {
    let [ix, iy] = p.map(lat);
    let [x, y] = p.map(|v| v - v.floor());
    let corner = |cx: u32, cy: u32| {
        let [gx, gy] = grad2(h2u(ix.wrapping_add(cx), iy.wrapping_add(cy)));
        gx * (x - f64::from(cx)) + gy * (y - f64::from(cy))
    };
    let (ux, uy) = (quintic(x), quintic(y));
    lerp(lerp(corner(0, 0), corner(1, 0), ux), lerp(corner(0, 1), corner(1, 1), ux), uy) * SCALE2
}

fn noise3(p: [f64; 3]) -> f64 {
    let [ix, iy, iz] = p.map(lat);
    let [x, y, z] = p.map(|v| v - v.floor());
    let corner = |cx: u32, cy: u32, cz: u32| {
        let [gx, gy, gz] = grad3(h3u(ix.wrapping_add(cx), iy.wrapping_add(cy), iz.wrapping_add(cz)));
        gx * (x - f64::from(cx)) + gy * (y - f64::from(cy)) + gz * (z - f64::from(cz))
    };
    let (ux, uy, uz) = (quintic(x), quintic(y), quintic(z));
    let z0 = lerp(lerp(corner(0, 0, 0), corner(1, 0, 0), ux), lerp(corner(0, 1, 0), corner(1, 1, 0), ux), uy);
    let z1 = lerp(lerp(corner(0, 0, 1), corner(1, 0, 1), ux), lerp(corner(0, 1, 1), corner(1, 1, 1), ux), uy);
    lerp(z0, z1, uz) * SCALE3
}

// The 3x3 (or 3x3x3) search can miss the true second-nearest point when
// jitter is extreme. Both backends search the same cells in the same order.
fn cellular2(p: [f64; 2]) -> [f64; 4] {
    let [ix, iy] = p.map(lat);
    let [x, y] = p.map(|v| v - v.floor());
    let (mut d1, mut d2, mut nx, mut ny) = (8.0_f64, 8.0_f64, 0.0, 0.0);
    for j in -1..=1 {
        for i in -1..=1 {
            let h = h2u(ix.wrapping_add_signed(i), iy.wrapping_add_signed(j));
            let (sx, sy) = (f64::from(i) + unit(h), f64::from(j) + unit(pcg(h)));
            let (rx, ry) = (sx - x, sy - y);
            let d = rx * rx + ry * ry;
            if d < d1 {
                d2 = d1;
                d1 = d;
                // floor(p) + site gives one colour per cell when hashed.
                nx = p[0].floor() + sx;
                ny = p[1].floor() + sy;
            } else if d < d2 {
                d2 = d;
            }
        }
    }
    [d1.sqrt(), d2.sqrt(), nx, ny]
}

fn cellular3(p: [f64; 3]) -> [f64; 2] {
    let [ix, iy, iz] = p.map(lat);
    let [x, y, z] = p.map(|v| v - v.floor());
    let (mut d1, mut d2) = (8.0_f64, 8.0_f64);
    for k in -1..=1 {
        for j in -1..=1 {
            for i in -1..=1 {
                let h = h3u(ix.wrapping_add_signed(i), iy.wrapping_add_signed(j), iz.wrapping_add_signed(k));
                let h1 = pcg(h);
                let rx = f64::from(i) + unit(h) - x;
                let ry = f64::from(j) + unit(h1) - y;
                let rz = f64::from(k) + unit(pcg(h1)) - z;
                let d = rx * rx + ry * ry + rz * rz;
                if d < d1 {
                    d2 = d1;
                    d1 = d;
                } else if d < d2 {
                    d2 = d;
                }
            }
        }
    }
    [d1.sqrt(), d2.sqrt()]
}

fn d_quintic(t: f64) -> f64 {
    30.0 * t * t * (t - 1.0) * (t - 1.0)
}

struct Vg<const N: usize> {
    v: f64,
    g: [f64; N],
}

// A quintic mix adds (b-a) du to the gradient along the mixing axis.
fn mix_vg<const N: usize>(a: Vg<N>, b: Vg<N>, t: f64, axis: usize) -> Vg<N> {
    let u = quintic(t);
    let du = d_quintic(t);
    let g = std::array::from_fn(|i| a.g[i] + u * (b.g[i] - a.g[i]) + if i == axis { du * (b.v - a.v) } else { 0.0 });
    Vg { v: a.v + u * (b.v - a.v), g }
}

// These twins hash floor(p) + corner, in f64 before the cast, as grad.ts does.
fn noise2_grad(p: [f64; 2]) -> [f64; 2] {
    let [ix, iy] = p.map(f64::floor);
    let [fx, fy] = [p[0] - ix, p[1] - iy];
    let corner = |cx: f64, cy: f64| {
        let a = hash2([ix + cx, iy + cy]) * TAU;
        let g = [a.cos(), a.sin()];
        Vg { v: g[0] * (fx - cx) + g[1] * (fy - cy), g }
    };
    let n = mix_vg(
        mix_vg(corner(0.0, 0.0), corner(1.0, 0.0), fx, 0),
        mix_vg(corner(0.0, 1.0), corner(1.0, 1.0), fx, 0),
        fy,
        1,
    );
    n.g.map(|x| x * SCALE2)
}

fn noise3_grad(p: [f64; 3]) -> [f64; 3] {
    let [ix, iy, iz] = p.map(f64::floor);
    let [fx, fy, fz] = [p[0] - ix, p[1] - iy, p[2] - iz];
    let corner = |cx: f64, cy: f64, cz: f64| {
        let h = hash33([ix + cx, iy + cy, iz + cz]);
        let z = 1.0 - 2.0 * h[0];
        let a = h[1] * TAU;
        let r = ((1.0 - z) * (1.0 + z)).sqrt();
        let g = [r * a.cos(), r * a.sin(), z];
        Vg { v: g[0] * (fx - cx) + g[1] * (fy - cy) + g[2] * (fz - cz), g }
    };
    let xs = |cy, cz| mix_vg(corner(0.0, cy, cz), corner(1.0, cy, cz), fx, 0);
    let n = mix_vg(mix_vg(xs(0.0, 0.0), xs(1.0, 0.0), fy, 1), mix_vg(xs(0.0, 1.0), xs(1.0, 1.0), fy, 1), fz, 2);
    n.g.map(|x| x * SCALE3)
}

const SQRT_PI_2: f64 = 1.772_453_850_905_515_9 / 2.0;

/// erfc(z) for z >= 0 (Numerical Recipes erfcc, fractional error < 1.2e-7).
fn erfc_pos(z: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.5 * z);
    t * (-z * z - 1.26551223
        + t * (1.00002368
            + t * (0.37409196
                + t * (0.09678418
                    + t * (-0.18628806
                        + t * (0.27886807
                            + t * (-1.13520398 + t * (1.48851587 + t * (-0.82215223 + t * 0.17087277)))))))))
        .exp()
}

/// Integral of exp(-x²) over a <= x <= b, accurate in the tails.
fn gseg(a: f64, b: f64) -> f64 {
    let d = if a >= 0.0 {
        erfc_pos(a) - erfc_pos(b)
    } else if b <= 0.0 {
        erfc_pos(-b) - erfc_pos(-a)
    } else {
        2.0 - erfc_pos(-a) - erfc_pos(b)
    };
    SQRT_PI_2 * d
}

fn gtail(a: f64) -> f64 {
    SQRT_PI_2 * if a >= 0.0 { erfc_pos(a) } else { 2.0 - erfc_pos(-a) }
}

/// The optical depth alone. Simpson's rule avoids subtracting nearly equal
/// integrals for short or near-parallel rays.
fn gauss_depth([ox, oy, oz, ow]: [f64; 4], [dx, dy, dz, dw]: [f64; 4], t0: f64, t1: f64) -> f64 {
    let len = math::max(t1 - t0, 0.0);
    let alpha = dx * dx + dy * dy + dz * dz + dw * dw;
    let al = math::max(alpha, 1e-20);
    let tm = -(ox * dx + oy * dy + oz * dz + ow * dw) / al;
    let s = al.sqrt();
    let q = |t| {
        let (x, y, z, w) = (ox + dx * t, oy + dy * t, oz + dz * t, ow + dw * t);
        x * x + y * y + z * z + w * w
    };
    if s * len < 0.05 {
        return (len / 6.0) * ((-q(t0)).exp() + 4.0 * (-q(t0 + 0.5 * len)).exp() + (-q(t0 + len)).exp());
    }
    ((-q(tm)).exp() / s) * gseg(s * (t0 - tm), s * (t0 + len - tm))
}

/// Integral of exp(-|O + D t|²) from zero to infinity.
fn gauss_tail([ox, oy, oz, _]: [f64; 4], [dx, dy, dz, _]: [f64; 4]) -> f64 {
    let alpha = math::max(dx * dx + dy * dy + dz * dz, 1e-12);
    let tm = -(ox * dx + oy * dy + oz * dz) / alpha;
    let s = alpha.sqrt();
    let qmin = math::pow(ox + dx * tm, 2.0) + math::pow(oy + dy * tm, 2.0) + math::pow(oz + dz * tm, 2.0);
    ((-qmin).exp() / s) * gtail(-s * tm)
}

fn medium_light_depth(o: [f64; 4], d: [f64; 4], t: f64, density: f64) -> f64 {
    if density <= 0.0 {
        return 0.0;
    }
    // A finite stand-in for the ray toward the sun.
    density * if t >= 1e4 { gauss_tail(o, d) } else { gauss_depth(o, d, 0.0, t) }
}
