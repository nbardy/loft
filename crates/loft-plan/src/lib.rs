//! Stage 0 horizon solving (06 §9). The curve and all optimization arithmetic are CPU f64.
mod solve;
mod spline;

use loft_abi::{VerdictReason, VerdictRecord, VerdictTag};
use solve::{Condition, Cost, HoldRow, pin};
use spline::{Layout, Vec3, prior};
use std::array::from_fn;

pub const KNOTS: usize = 24;
pub const COLLOCATION: usize = 4 * KNOTS;

pub enum Objective<'a> {
    Effort(f64),
    Jerk(f64),
    /// Cardinal cubic controls spanning the request's duration. Weights apply to position,
    /// velocity and acceleration, in that order.
    Near {
        controls: &'a [[f64; 3]],
        weights: [f64; 3],
    },
}

pub enum Hold {
    PosAt {
        time: f64,
        position: [f64; 3],
        tolerance: f64,
    },
    VelAt {
        time: f64,
        velocity: [f64; 3],
        tolerance: f64,
    },
    AccAt {
        time: f64,
        acceleration: [f64; 3],
    },
    ArriveAt {
        time: f64,
        position: [f64; 3],
        velocity: [f64; 3],
    },
    Clear {
        center: [f64; 3],
        half: [f64; 3],
        margin: f64,
    },
    /// Height and world velocity of the landing surface at the requested time.
    LandOn {
        time: f64,
        height: f64,
        surface_velocity: [f64; 3],
    },
    PitchIn {
        lo: f64,
        hi: f64,
    },
}

pub struct Plan<'a> {
    pub duration: f64,
    pub gravity: [f64; 3],
    /// Takeoff and touchdown times. The adjoining phases share position and velocity.
    pub flight: Option<[f64; 2]>,
    pub objectives: &'a [Objective<'a>],
    pub holds: &'a [Hold],
}

pub struct Curve {
    layout: Layout,
    controls: Vec<Vec3>,
    duration: f64,
    pitch: f64,
    /// Largest violation at an authored hold or clearance collocation point.
    pub residual: f64,
}

impl Curve {
    /// The jet in metres and seconds (position, velocity, acceleration, jerk).
    /// Position and velocity are continuous. At a phase switch the new phase
    /// supplies acceleration and jerk.
    pub fn at(&self, t: f64) -> Option<[[f64; 3]; 4]> {
        if !(0.0..=self.duration).contains(&t) {
            return None;
        }
        let jet = self.layout.at(t).map(|row| row.sample(&self.controls));
        jet.iter().copied().all(finite).then_some(jet)
    }

    pub fn pitch(&self) -> f64 {
        self.pitch
    }
}

fn failure() -> VerdictRecord {
    VerdictRecord {
        tag: VerdictTag::Unknown as u32,
        reason_code: VerdictReason::SolveFailed as u32,
        ..VerdictRecord::default()
    }
}

/// Solve the declared holds to the 1e-5 collocation tolerance.
pub fn solve(plan: &Plan<'_>) -> Result<Curve, VerdictRecord> {
    prepare_and_solve(plan).ok_or_else(failure)
}

fn finite(x: Vec3) -> bool {
    x.into_iter().all(f64::is_finite)
}

fn nonnegative(x: f64) -> bool {
    x.is_finite() && x >= 0.0
}

fn prepare_and_solve(plan: &Plan<'_>) -> Option<Curve> {
    let Plan {
        duration,
        gravity,
        flight,
        objectives,
        holds,
    } = *plan;
    let valid_time = |t| (0.0..=duration).contains(&t);
    if !duration.is_finite() || duration <= 0.0 || !finite(gravity) || objectives.is_empty() {
        return None;
    }
    if let Some([from, to]) = flight {
        if !valid_time(from) || !valid_time(to) || from >= to {
            return None;
        }
    }

    let (layout, times) = Layout::new(duration, flight, gravity);
    let dt = duration / COLLOCATION as f64;
    let collocation: Vec<_> = (0..COLLOCATION)
        .map(|i| {
            let t = (i as f64 + 0.5) * dt;
            (t, layout.at(t))
        })
        .collect();
    let mut costs = Vec::new();
    let mut seed_prior = None;
    for objective in objectives {
        match *objective {
            Objective::Effort(weight) => {
                if !nonnegative(weight) {
                    return None;
                }
                costs.extend(collocation.iter().map(|(_, jet)| Cost {
                    form: jet[2],
                    target: gravity,
                    weight: dt * weight,
                }));
            }
            Objective::Jerk(weight) => {
                if !nonnegative(weight) {
                    return None;
                }
                costs.extend(layout.jerk().map(|(form, duration)| Cost {
                    form,
                    target: [0.0; 3],
                    weight: duration * weight,
                }));
            }
            Objective::Near { controls, weights } => {
                if controls.len() < 4
                    || !controls.iter().copied().all(finite)
                    || !weights.into_iter().all(nonnegative)
                {
                    return None;
                }
                if weights.into_iter().any(|w| w > 0.0) {
                    seed_prior.get_or_insert(controls);
                }
                for (t, jet) in &collocation {
                    let target = prior(controls, *t, duration);
                    costs.extend((0..3).map(|d| Cost {
                        form: jet[d],
                        target: target[d],
                        weight: dt * weights[d],
                    }));
                }
            }
        }
    }
    costs.retain(|row| row.weight > 0.0);

    let mut rows = Vec::new();
    let mut pitch_lo = f64::NEG_INFINITY;
    let mut pitch_hi = f64::INFINITY;
    let mut start = [[0.0; 3]; 3];
    let mut arrivals = Vec::new();
    let mut accelerations = Vec::new();
    for hold in holds {
        match *hold {
            Hold::PosAt {
                time,
                position,
                tolerance,
            } => {
                if !valid_time(time) || !finite(position) || !nonnegative(tolerance) {
                    return None;
                }
                pin(&mut rows, layout.at(time)[0], position, tolerance);
                if time == 0.0 {
                    start[0] = position;
                }
            }
            Hold::VelAt {
                time,
                velocity,
                tolerance,
            } => {
                if !valid_time(time) || !finite(velocity) || !nonnegative(tolerance) {
                    return None;
                }
                pin(&mut rows, layout.at(time)[1], velocity, tolerance);
                if time == 0.0 {
                    start[1] = velocity;
                }
            }
            Hold::AccAt { time, acceleration } => {
                if !valid_time(time) || !finite(acceleration) {
                    return None;
                }
                pin(&mut rows, layout.at(time)[2], acceleration, 0.0);
                accelerations.push((time, acceleration));
                if time == 0.0 {
                    start[2] = acceleration;
                }
            }
            Hold::ArriveAt {
                time,
                position,
                velocity,
            } => {
                if !valid_time(time) || !finite(position) || !finite(velocity) {
                    return None;
                }
                let [p, v, _, _] = layout.at(time);
                pin(&mut rows, p, position, 0.0);
                pin(&mut rows, v, velocity, 0.0);
                if time == 0.0 {
                    start[0] = position;
                    start[1] = velocity;
                } else {
                    arrivals.push((time, [position, velocity, [0.0; 3]]));
                }
            }
            Hold::Clear {
                center,
                half,
                margin,
            } => {
                if !finite(center) || !half.into_iter().all(nonnegative) || !nonnegative(margin) {
                    return None;
                }
                rows.extend(collocation.iter().map(|(_, jet)| HoldRow {
                    form: jet[0],
                    condition: Condition::Clear {
                        center,
                        half,
                        margin,
                    },
                }));
            }
            Hold::LandOn {
                time,
                height,
                surface_velocity,
            } => {
                if !valid_time(time) || !height.is_finite() || !finite(surface_velocity) {
                    return None;
                }
                let [p, v, _, _] = layout.at(time);
                rows.push(HoldRow {
                    form: p,
                    condition: Condition::Equal([0.0, 1.0, 0.0], height),
                });
                pin(&mut rows, v, surface_velocity, 0.0);
            }
            Hold::PitchIn { lo, hi } => {
                if !lo.is_finite() || !hi.is_finite() || lo > hi {
                    return None;
                }
                pitch_lo = pitch_lo.max(lo);
                pitch_hi = pitch_hi.min(hi);
            }
        }
    }
    if pitch_lo > pitch_hi {
        return None;
    }

    arrivals.sort_by(|a, b| a.0.total_cmp(&b.0));
    arrivals.dedup_by(|a, b| a.0 == b.0);
    for (time, target) in &mut arrivals {
        if let Some((_, acceleration)) = accelerations.iter().find(|(at, _)| at == time) {
            target[2] = *acceleration;
        }
    }
    let seed = layout.seed(&times, |t| match seed_prior {
        Some(c) => prior(c, t, duration)[0],
        None => anticipate(t, start, &arrivals),
    });
    let (controls, residual) = solve::optimize(seed, &costs, &rows)?;
    let curve = Curve {
        layout,
        controls,
        duration,
        pitch: 0.0f64.clamp(pitch_lo, pitch_hi),
        residual,
    };
    // Catch non-finite derivative scales even when a request assigns them zero weight.
    collocation
        .iter()
        .all(|(t, _)| curve.at(*t).is_some())
        .then_some(curve)
}

fn anticipate(t: f64, mut start: [Vec3; 3], arrivals: &[(f64, [Vec3; 3])]) -> Vec3 {
    let mut from = 0.0;
    for &(to, end) in arrivals {
        if t <= to {
            let h = to - from;
            let s = ((t - from) / h).max(0.0);
            // Lift from the endpoint jets into a quintic seed. Only the spline
            // coefficients enter the objective, so no extremum becomes a hold.
            return from_fn(|axis| {
                let p = start[0][axis];
                let v = h * start[1][axis];
                let a = 0.5 * h * h * start[2][axis];
                let d = end[0][axis] - (p + v + a);
                let w = h * end[1][axis] - (v + 2.0 * a);
                let b = h * h * end[2][axis] - 2.0 * a;
                let c3 = 10.0 * d - 4.0 * w + 0.5 * b;
                let c4 = -15.0 * d + 7.0 * w - b;
                let c5 = 6.0 * d - 3.0 * w + 0.5 * b;
                p + s * (v + s * (a + s * (c3 + s * (c4 + s * c5))))
            });
        }
        start = end;
        from = to;
    }
    from_fn(|axis| start[0][axis] + (t - from) * start[1][axis])
}
