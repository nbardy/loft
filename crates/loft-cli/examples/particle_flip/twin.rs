use serde::Serialize;

pub const COUNT: usize = 65_536;
pub const CELLS: usize = 32 * 32 * 32;
pub const SCALE: f64 = 4096.;
const DT: f64 = 1. / 120.;
const VELOCITY_UNIT: f64 = 16.;
const LO: [f64; 3] = [1.25; 3];
const HI: [f64; 3] = [29.75, 30.0, 29.75];

#[derive(Clone, Copy)]
pub enum Stencil {
    Linear,
    Quadratic,
}

#[derive(Clone, Copy)]
pub enum Distribution {
    Spread,
    Clumped,
}

impl Distribution {
    pub fn name(self) -> &'static str {
        match self {
            Self::Spread => "spread",
            Self::Clumped => "clumped",
        }
    }

    fn span(self) -> u32 {
        match self {
            Self::Spread => 32,
            Self::Clumped => 4,
        }
    }
}

pub fn mix(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^ (x >> 16)
}

fn packed(q: [u32; 3], v: [i32; 3]) -> [u32; 4] {
    assert!(v.iter().all(|x| (-63..=63).contains(x)));
    [
        q[0],
        q[1],
        q[2],
        (v[0] + 63) as u32 | ((v[1] + 63) as u32) << 8 | ((v[2] + 63) as u32) << 16,
    ]
}

pub fn fixture(seed: u32, layout: Distribution) -> Vec<[u32; 4]> {
    (0..COUNT as u32)
        .map(|id| {
            // Seed 0 uses exactly the proposal's input function.
            let word = |lane| mix((6 * id + lane).wrapping_add(seed.wrapping_mul(0x9e37_79b9)));
            packed(
                [0, 1, 2].map(|lane| word(lane) % (16 * layout.span())),
                [3, 4, 5].map(|lane| (word(lane) % 127) as i32 - 63),
            )
        })
        .collect()
}

pub fn edge_fixture() -> Vec<[u32; 4]> {
    (0..COUNT)
        .map(|id| {
            let q = [0, 7, 8, 15, 16, 17, 503, 504, 511];
            let sign = if id % 2 == 0 { -1 } else { 1 };
            packed(
                [
                    q[(id / 2) % q.len()],
                    q[(id / 18) % q.len()],
                    q[(id / 162) % q.len()],
                ],
                [sign * 63, -sign * 63, sign],
            )
        })
        .collect()
}

pub fn cancellation_fixture() -> Vec<[u32; 4]> {
    (0..COUNT)
        .map(|id| {
            packed(
                [0; 3],
                if id < COUNT / 2 {
                    [63, -63, 63]
                } else {
                    [-63, 63, -63]
                },
            )
        })
        .collect()
}

fn taps(q: [u32; 3], stencil: Stencil, mut emit: impl FnMut(usize, i64, i64)) {
    let (width, base, weights, den) = match stencil {
        Stencil::Linear => (
            2,
            q.map(|q| q as i64 / 16),
            q.map(|q| {
                let f = i64::from(q % 16);
                [16 - f, f, 0]
            }),
            4096,
        ),
        Stencil::Quadratic => (
            3,
            q.map(|q| (i64::from(q) + 8) / 16 - 1),
            q.map(|q| {
                let d = i64::from((q + 8) % 16);
                [(16 - d).pow(2), 256 + 32 * d - 2 * d * d, d.pow(2)]
            }),
            134_217_728,
        ),
    };
    for z in 0..width {
        for y in 0..width {
            for x in 0..width {
                let [a, b, c] = [x, y, z];
                let [cx, cy, cz] = [a as i64, b as i64, c as i64];
                let cell = (base[0] + cx).rem_euclid(32)
                    + 32 * ((base[1] + cy).rem_euclid(32) + 32 * (base[2] + cz).rem_euclid(32));
                emit(
                    cell as usize,
                    weights[0][a] * weights[1][b] * weights[2][c],
                    den,
                );
            }
        }
    }
}

fn velocity(bits: u32) -> [i64; 3] {
    [0, 8, 16].map(|shift| i64::from((bits >> shift) & 127) - 63)
}

pub fn integer_grid(rows: &[[u32; 4]], stencil: Stencil) -> Vec<[i64; 4]> {
    assert!(rows.len() <= COUNT);
    let mut grid = vec![[0_i64; 4]; CELLS];
    for &[x, y, z, bits] in rows {
        let v = velocity(bits);
        taps([x, y, z], stencil, |cell, w, den| {
            grid[cell][0] += (w * 4096 + den / 2) / den;
            for (a, v) in v.into_iter().enumerate() {
                let num = w * 4096 * v;
                let den = den * 63;
                grid[cell][a + 1] += num.signum() * ((num.abs() + den / 2) / den);
            }
        });
    }
    grid
}

pub fn integer_words(grid: &[[i64; 4]]) -> Vec<u32> {
    grid.iter()
        .flatten()
        .map(|&x| i32::try_from(x).expect("bounded i32") as u32)
        .collect()
}

pub fn words(bytes: &[u8]) -> impl Iterator<Item = u32> + '_ {
    bytes
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
}

pub fn bytes(words: impl IntoIterator<Item = u32>) -> Vec<u8> {
    words.into_iter().flat_map(u32::to_le_bytes).collect()
}

pub fn float_grid(rows: &[[u32; 4]]) -> (Vec<[f64; 4]>, [f64; 4]) {
    let mut grid = vec![[0.; 4]; CELLS];
    let mut abs = [0.; 4];
    for &[x, y, z, bits] in rows {
        let [vx, vy, vz] = velocity(bits).map(|v| v as f64 / 63.);
        let channels = [1., vx, vy, vz];
        for a in 0..4 {
            abs[a] += channels[a].abs();
        }
        taps([x, y, z], Stencil::Linear, |cell, w, den| {
            for (word, v) in grid[cell].iter_mut().zip(channels) {
                *word += (w as f64 / den as f64) * v;
            }
        });
    }
    (grid, abs)
}

#[derive(Serialize)]
pub struct FloatError {
    pub worst_cell: f64,
    pub worst_cell_word: usize,
    pub rms: f64,
    pub global: [f64; 4],
    pub nonfinite: usize,
}

impl FloatError {
    pub fn passes(&self) -> bool {
        self.nonfinite == 0 && self.worst_cell <= 1e-4 && self.global.iter().all(|e| *e <= 1e-5)
    }
}

pub fn float_error(raw: &[u8], truth: &[[f64; 4]], abs: [f64; 4]) -> FloatError {
    assert_eq!(raw.len(), CELLS * 16);
    let mut error = FloatError {
        worst_cell: 0.,
        worst_cell_word: 0,
        rms: 0.,
        global: [0.; 4],
        nonfinite: 0,
    };
    for (i, (actual, expected)) in words(raw)
        .map(f32::from_bits)
        .zip(truth.iter().flatten())
        .enumerate()
    {
        if !actual.is_finite() {
            error.nonfinite += 1;
            continue;
        }
        let delta = f64::from(actual) - expected;
        if delta.abs() > error.worst_cell {
            error.worst_cell = delta.abs();
            error.worst_cell_word = i;
        }
        error.rms += delta * delta;
        error.global[i % 4] += delta;
    }
    error.rms = (error.rms / (CELLS * 4) as f64).sqrt();
    error.global = std::array::from_fn(|a| (error.global[a] / abs[a].max(f64::MIN_POSITIVE)).abs());
    error
}

#[derive(Clone, Copy)]
pub struct State {
    pos: [f64; 3],
    vel: [f64; 3],
}

pub fn splash() -> Vec<State> {
    let sample = |id, axis| f64::from(mix(id * 7 + axis) >> 8) / 16_777_216.;
    (0..COUNT as u32)
        .map(|id| {
            let [u, v, w] = [0, 1, 2].map(|axis| sample(id, axis));
            let state = if id < COUNT as u32 * 3 / 4 {
                // A falling sheet strikes a shallow pool.
                State {
                    pos: [8. + 8. * u, 10. + 9. * v, 8. + 15. * w],
                    vel: [6. + 2. * v, -4. - 2. * u, 3. * (0.5 - w)],
                }
            } else {
                State {
                    pos: [6. + 20. * u, 1.35 + 1.3 * v, 5. + 22. * w],
                    vel: [0.; 3],
                }
            };
            State {
                pos: state.pos.map(|x| f64::from(x as f32)),
                vel: state.vel.map(|x| f64::from(x as f32)),
            }
        })
        .collect()
}

pub fn state_bytes(states: &[State]) -> Vec<u8> {
    bytes(states.iter().flat_map(|s| {
        [
            s.pos[0], s.pos[1], s.pos[2], 0., s.vel[0], s.vel[1], s.vel[2], 0.,
        ]
        .map(|x| (x as f32).to_bits())
    }))
}

pub fn step(states: &mut [State]) {
    // Independent f64 twin of the linear, floating FLIP arm.
    let packed_rows: Vec<_> = states
        .iter()
        .map(|s| {
            packed(
                s.pos.map(|x| (x * 16. + 0.5).floor() as u32),
                s.vel
                    .map(|x| ((x / VELOCITY_UNIT).clamp(-1., 1.) * 63. + 63.5).floor() as i32 - 63),
            )
        })
        .collect();
    let (grid, _) = float_grid(&packed_rows);
    let nodes: Vec<_> = grid
        .into_iter()
        .enumerate()
        .map(|(i, [mass, x, y, z])| {
            let old = if mass > 0. {
                [x, y, z].map(|v| v * VELOCITY_UNIT / mass)
            } else {
                [0.; 3]
            };
            let c = [i % 32, (i / 32) % 32, i / 1024];
            let next = std::array::from_fn(|a| {
                if mass == 0. {
                    return 0.;
                }
                let v = old[a] - if a == 1 { 12. * DT } else { 0. };
                let c = c[a] as f64;
                if c <= LO[a] + 0.75 {
                    v.max(0.)
                } else if c >= HI[a] - 0.75 {
                    v.min(0.)
                } else {
                    v
                }
            });
            (old, next)
        })
        .collect();
    for (s, &[x, y, z, _]) in states.iter_mut().zip(&packed_rows) {
        let mut pic = [0.; 3];
        let mut delta = [0.; 3];
        taps([x, y, z], Stencil::Linear, |cell, w, den| {
            let (old, next): ([f64; 3], [f64; 3]) = nodes[cell];
            let weight = w as f64 / den as f64;
            for a in 0..3 {
                pic[a] += weight * next[a];
                delta[a] += weight * (next[a] - old[a]);
            }
        });
        for a in 0..3 {
            let v =
                (0.92 * (s.vel[a] + delta[a]) + 0.08 * pic[a]).clamp(-VELOCITY_UNIT, VELOCITY_UNIT);
            let p = s.pos[a] + v * DT;
            s.pos[a] = p.clamp(LO[a], HI[a]);
            s.vel[a] = if s.pos[a] == p { v } else { -0.15 * v };
        }
    }
}

#[derive(Serialize)]
pub struct Drift {
    pub velocity_rms: f64,
    pub velocity_max: f64,
    pub gpu_mean_velocity: [f64; 3],
    pub cpu_mean_velocity: [f64; 3],
    pub gpu_kinetic_energy: f64,
    pub cpu_kinetic_energy: f64,
}

pub fn drift(gpu: &[u8], truth: &[State]) -> Drift {
    assert_eq!(gpu.len(), truth.len() * 32);
    let n = truth.len() as f64;
    let mut out = Drift {
        velocity_rms: 0.,
        velocity_max: 0.,
        gpu_mean_velocity: [0.; 3],
        cpu_mean_velocity: [0.; 3],
        gpu_kinetic_energy: 0.,
        cpu_kinetic_energy: 0.,
    };
    for (row, state) in gpu.chunks_exact(32).zip(truth) {
        for (a, x) in words(&row[16..28])
            .map(|w| f64::from(f32::from_bits(w)))
            .enumerate()
        {
            assert!(x.is_finite());
            let y = state.vel[a];
            out.velocity_rms += (x - y).powi(2);
            out.velocity_max = out.velocity_max.max((x - y).abs());
            out.gpu_mean_velocity[a] += x / n;
            out.cpu_mean_velocity[a] += y / n;
            out.gpu_kinetic_energy += 0.5 * x * x;
            out.cpu_kinetic_energy += 0.5 * y * y;
        }
    }
    out.velocity_rms = (out.velocity_rms / (3. * n)).sqrt();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maximum_occupancy_cancels_signed_words() {
        let grid = integer_grid(&cancellation_fixture(), Stencil::Linear);
        assert_eq!(grid[0], [1 << 28, 0, 0, 0]);
        assert!(grid[1..].iter().all(|g| *g == [0; 4]));
    }

    #[test]
    fn periodic_stencils_visit_distinct_nodes_and_partition_unity() {
        for stencil in [Stencil::Linear, Stencil::Quadratic] {
            for q in [0, 7, 8, 15, 16, 17, 503, 504, 511] {
                let mut nodes = std::collections::BTreeSet::new();
                let mut sum = 0;
                taps([q, q, q], stencil, |i, w, den| {
                    assert!(i < CELLS && nodes.insert(i));
                    sum += w;
                    assert!(sum <= den);
                });
                match stencil {
                    Stencil::Linear => assert_eq!((nodes.len(), sum), (8, 4096)),
                    Stencil::Quadratic => assert_eq!((nodes.len(), sum), (27, 134_217_728)),
                }
            }
        }
    }
}
