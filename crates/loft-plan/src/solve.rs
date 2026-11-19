use crate::spline::{BAND, Form, Vec3};
use std::array::from_fn;

pub(crate) const TOL: f64 = 1e-5;
const MAX_ITERATIONS: usize = 40;
const AXES: [Vec3; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

fn dot(a: Vec3, b: Vec3) -> f64 {
    a.into_iter().zip(b).map(|(x, y)| x * y).sum()
}

fn norm([x, y, z]: Vec3) -> f64 {
    x.hypot(y).hypot(z)
}

pub(crate) struct Cost {
    pub form: Form,
    pub target: Vec3,
    pub weight: f64,
}

pub(crate) enum Condition {
    Equal(Vec3, f64),
    Within(Vec3, f64),
    Clear {
        center: Vec3,
        half: Vec3,
        margin: f64,
    },
}

impl Condition {
    fn value(&self, x: Vec3) -> (f64, Vec3) {
        match *self {
            Self::Equal(axis, target) => (dot(axis, x) - target, axis),
            Self::Within(target, tolerance) => within(x, target, tolerance),
            Self::Clear {
                center,
                half,
                margin,
            } => clear(x, center, half, margin),
        }
    }

    fn equality(&self) -> bool {
        matches!(self, Self::Equal(..))
    }
}

fn within(x: Vec3, target: Vec3, tolerance: f64) -> (f64, Vec3) {
    let d = from_fn(|i| x[i] - target[i]);
    let length = norm(d);
    let direction = if length > 0.0 {
        d.map(|v| v / length)
    } else {
        [0.0; 3]
    };
    (length - tolerance, direction)
}

fn clear(x: Vec3, center: Vec3, half: Vec3, margin: f64) -> (f64, Vec3) {
    let d: Vec3 = from_fn(|i| x[i] - center[i]);
    let q: Vec3 = from_fn(|i| d[i].abs() - half[i]);
    let outside = q.map(|v| v.max(0.0));
    let length = norm(outside);
    if length > 0.0 {
        (
            margin - length,
            from_fn(|i| -d[i].signum() * outside[i] / length),
        )
    } else {
        // At an interior tie take the first supporting face in XYZ order.
        let mut face = 0;
        for i in 1..3 {
            if q[i] > q[face] {
                face = i;
            }
        }
        let mut direction = [0.0; 3];
        direction[face] = -d[face].signum();
        (margin - q[face], direction)
    }
}

pub(crate) struct HoldRow {
    pub form: Form,
    pub condition: Condition,
}

pub(crate) fn pin(rows: &mut Vec<HoldRow>, form: Form, target: Vec3, tolerance: f64) {
    if tolerance == 0.0 {
        rows.extend(AXES.into_iter().zip(target).map(|(axis, value)| HoldRow {
            form,
            condition: Condition::Equal(axis, value),
        }));
    } else {
        rows.push(HoldRow {
            form,
            condition: Condition::Within(target, tolerance),
        });
    }
}

#[derive(Clone)]
struct Band(Vec<[f64; BAND]>);

impl Band {
    fn rank(&mut self, form: Form, direction: Vec3, weight: f64) {
        for (i, a) in form.project(direction) {
            for (j, b) in form.project(direction).take_while(|&(j, _)| j <= i) {
                self.0[i][i - j] += weight * a * b;
            }
        }
    }

    fn newton(mut self, gradient: &[f64]) -> Option<Vec<f64>> {
        let peak = self.0.iter().map(|row| row[0]).fold(0.0, f64::max);
        let scale: Vec<_> = self
            .0
            .iter()
            .map(|row| row[0].max(peak * 1e-12).max(1e-30).sqrt())
            .collect();
        let mut step: Vec<_> = gradient.iter().zip(&scale).map(|(&g, &d)| -g / d).collect();
        for i in 0..step.len() {
            let first = (i + 1).saturating_sub(BAND);
            for j in first..=i {
                let mut v = self.0[i][i - j] / (scale[i] * scale[j]);
                for k in first..j {
                    v -= self.0[i][i - k] * self.0[j][j - k];
                }
                self.0[i][i - j] = if i == j {
                    // Damping only the Newton step leaves the objective and stationarity test intact.
                    v += 1e-10;
                    if !v.is_finite() || v <= 0.0 {
                        return None;
                    }
                    v.sqrt()
                } else {
                    v / self.0[j][0]
                };
            }
            for j in first..i {
                step[i] -= self.0[i][i - j] * step[j];
            }
            step[i] /= self.0[i][0];
        }
        for i in (0..step.len()).rev() {
            for j in i + 1..(i + BAND).min(step.len()) {
                step[i] -= self.0[j][j - i] * step[j];
            }
            step[i] /= self.0[i][0];
        }
        for (value, d) in step.iter_mut().zip(scale) {
            *value /= d;
        }
        Some(step)
    }
}

struct Multiplier<'a> {
    hold: &'a HoldRow,
    scale: f64,
    dual: f64,
}

impl Multiplier<'_> {
    fn augmented(&self, x: &[Vec3], rho: f64) -> (f64, Vec3, bool) {
        let (value, direction) = self.hold.condition.value(self.hold.form.sample(x));
        let shift = self.dual + rho * self.scale * value;
        let active = self.hold.condition.equality() || shift > 0.0;
        (if active { shift } else { 0.0 }, direction, active)
    }

    fn update(&mut self, x: &[Vec3], rho: f64) -> f64 {
        self.dual = self.augmented(x, rho).0;
        let (value, _) = self.hold.condition.value(self.hold.form.sample(x));
        if !value.is_finite() {
            return f64::INFINITY;
        }
        if self.hold.condition.equality() || self.dual > 0.0 {
            value.abs()
        } else {
            value.max(0.0)
        }
    }
}

fn merit(x: &[Vec3], costs: &[Cost], holds: &[Multiplier<'_>], rho: f64) -> f64 {
    let objective: f64 = costs
        .iter()
        .map(|row| {
            let sample = row.form.sample(x);
            let residual = from_fn(|axis| sample[axis] - row.target[axis]);
            0.5 * row.weight * dot(residual, residual)
        })
        .sum();
    objective
        + holds
            .iter()
            .map(|hold| 0.5 * hold.augmented(x, rho).0.powi(2) / rho)
            .sum::<f64>()
}

fn minimize(
    x: &mut Vec<Vec3>,
    costs: &[Cost],
    holds: &[Multiplier<'_>],
    base: &Band,
    rho: f64,
) -> Option<()> {
    for _ in 0..32 {
        let mut hessian = base.clone();
        let mut gradient = vec![0.0; x.len() * 3];
        for row in costs {
            let sample = row.form.sample(x);
            let residual = from_fn(|axis| sample[axis] - row.target[axis]);
            for (i, value) in row.form.project(residual) {
                gradient[i] += row.weight * value;
            }
        }
        for hold in holds {
            let (dual, direction, active) = hold.augmented(x, rho);
            if active {
                hessian.rank(hold.hold.form, direction, rho * hold.scale.powi(2));
                for (i, value) in hold.hold.form.project(direction) {
                    gradient[i] += hold.scale * dual * value;
                }
            }
        }

        let cost = merit(x, costs, holds, rho);
        if !cost.is_finite() || !gradient.iter().all(|g| g.is_finite()) {
            return None;
        }
        // Clearance uses the supporting plane of the signed box distance in the
        // Newton model. The line search evaluates the actual signed distance.
        let threshold = 1e-8 * (1.0 + cost.sqrt());
        if gradient
            .iter()
            .zip(&hessian.0)
            .all(|(g, row)| g.abs() <= threshold * row[0].max(1e-30).sqrt())
        {
            return Some(());
        }
        let step = hessian.newton(&gradient)?;
        let slope: f64 = gradient.iter().zip(&step).map(|(g, s)| g * s).sum();
        if !slope.is_finite() || slope >= 0.0 {
            return None;
        }

        let mut accepted = None;
        for i in 0..24 {
            let alpha = 0.5f64.powi(i);
            let next: Vec<Vec3> = x
                .iter()
                .zip(step.chunks_exact(3))
                .map(|(q, dq)| from_fn(|axis| q[axis] + alpha * dq[axis]))
                .collect();
            if merit(&next, costs, holds, rho) <= cost + 1e-4 * alpha * slope {
                accepted = Some(next);
                break;
            }
        }
        *x = accepted?;
    }
    None
}

pub(crate) fn optimize(
    mut x: Vec<Vec3>,
    costs: &[Cost],
    rows: &[HoldRow],
) -> Option<(Vec<Vec3>, f64)> {
    let mut base = Band(vec![[0.0; BAND]; x.len() * 3]);
    for row in costs {
        for axis in AXES {
            base.rank(row.form, axis, row.weight);
        }
    }
    let peak = base.0.iter().map(|row| row[0]).fold(0.0, f64::max);
    let mut holds: Vec<_> = rows
        .iter()
        .map(|hold| {
            // Equilibrate units before updating duals (metres, m/s and m/s²).
            // The stopping test below still reads the original physical units.
            let metric: f64 = hold
                .form
                .project(AXES[0])
                .map(|(i, a)| a * a / base.0[i][0].max(peak * 1e-8).max(1e-12))
                .sum();
            Multiplier {
                hold,
                scale: metric.max(1e-30).sqrt().recip().min(1e8),
                dual: 0.0,
            }
        })
        .collect();

    let mut rho = 16.0;
    let mut previous = f64::INFINITY;
    for _ in 0..MAX_ITERATIONS {
        minimize(&mut x, costs, &holds, &base, rho)?;
        let mut worst = 0.0f64;
        for hold in &mut holds {
            worst = worst.max(hold.update(&x, rho));
        }
        if worst <= TOL {
            return Some((x, worst));
        }
        if worst > 0.5 * previous {
            rho *= 4.0;
        }
        previous = worst;
    }
    None
}
