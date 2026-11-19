use std::array::from_fn;

pub(crate) type Vec3 = [f64; 3];
pub(crate) type Point = [Vec3; 4];

// Three launch controls plus at most three consecutive landing controls.
const SUPPORT: usize = 6;
pub(crate) const BAND: usize = 3 * SUPPORT;

#[derive(Clone, Copy, Default)]
pub(crate) struct Form {
    pub first: usize,
    pub weights: [f64; SUPPORT],
    pub bias: Vec3,
}

impl Form {
    fn control(first: usize) -> Self {
        let mut form = Self {
            first,
            ..Self::default()
        };
        form.weights[0] = 1.0;
        form
    }

    fn combine(terms: impl IntoIterator<Item = (Self, f64)> + Clone) -> Self {
        let first = terms
            .clone()
            .into_iter()
            .flat_map(|(form, _)| form.terms().next().map(|(index, _)| index))
            .min()
            .unwrap_or(0);
        let mut result = Self {
            first,
            ..Self::default()
        };
        for (form, scale) in terms {
            for (index, coefficient) in form.terms() {
                result.weights[index - first] += coefficient * scale;
            }
            for (bias, source) in result.bias.iter_mut().zip(form.bias) {
                *bias += scale * source;
            }
        }
        result
    }

    fn terms(self) -> impl Iterator<Item = (usize, f64)> {
        self.weights
            .into_iter()
            .enumerate()
            .filter(|&(_, weight)| weight != 0.0)
            .map(move |(i, weight)| (self.first + i, weight))
    }

    pub(crate) fn sample(self, controls: &[Vec3]) -> Vec3 {
        let mut result = self.bias;
        for (i, weight) in self.terms() {
            for (axis, value) in result.iter_mut().zip(controls[i]) {
                *axis += weight * value;
            }
        }
        result
    }

    pub(crate) fn project(self, direction: Vec3) -> impl Iterator<Item = (usize, f64)> {
        self.terms().flat_map(move |(i, weight)| {
            direction
                .into_iter()
                .enumerate()
                .filter(move |&(_, d)| d != 0.0)
                .map(move |(axis, d)| (3 * i + axis, weight * d))
        })
    }
}

/// Cardinal cubic basis and its first three time derivatives on one span.
/// The first span uses controls 0..4, including the two endpoint neighbors.
fn basis(t: f64, start: f64, end: f64, spans: usize) -> (usize, [[f64; 4]; 4]) {
    let h = (end - start) / spans as f64;
    let s = ((t - start) / h).clamp(0.0, spans as f64);
    let i = (s as usize).min(spans - 1);
    let u = s - i as f64;
    let v = 1.0 - u;
    let weights = [
        [
            v.powi(3),
            4.0 - 6.0 * u * u + 3.0 * u.powi(3),
            4.0 - 6.0 * v * v + 3.0 * v.powi(3),
            u.powi(3),
        ],
        [
            -3.0 * v * v,
            -12.0 * u + 9.0 * u * u,
            12.0 * v - 9.0 * v * v,
            3.0 * u * u,
        ],
        [6.0 * v, -12.0 + 18.0 * u, -12.0 + 18.0 * v, 6.0 * u],
        [-6.0, 18.0, -18.0, 6.0],
    ];
    (
        i,
        from_fn(|d| weights[d].map(|w| w / (6.0 * h.powi(d as i32)))),
    )
}

pub(crate) fn prior(controls: &[Vec3], t: f64, duration: f64) -> Point {
    let (i, weights) = basis(t, 0.0, duration, controls.len() - 3);
    weights.map(|row| from_fn(|axis| (0..4).map(|j| row[j] * controls[i + j][axis]).sum()))
}

pub(crate) struct Spline {
    start: f64,
    end: f64,
    controls: Vec<Form>,
}

impl Spline {
    fn new(
        start: f64,
        end: f64,
        spans: usize,
        seeds: &mut Vec<f64>,
        join: Option<[Form; 2]>,
    ) -> Self {
        let h = (end - start) / spans as f64;
        let mut controls = Vec::with_capacity(spans + 3);
        if let Some([p, v]) = join {
            // At u=0, p=(c0+4c1+c2)/6 and v=(c2-c0)/(2h). Solve these
            // identities for c0 and c1. c2 remains the free landing acceleration.
            let c2 = Form::control(seeds.len());
            controls.push(Form::combine([(c2, 1.0), (v, -2.0 * h)]));
            controls.push(Form::combine([(p, 1.5), (c2, -0.5), (v, 0.5 * h)]));
        }
        while controls.len() < spans + 3 {
            controls.push(Form::control(seeds.len()));
            seeds.push(start + (controls.len() as f64 - 2.0) * h);
        }
        Self {
            start,
            end,
            controls,
        }
    }

    fn at(&self, t: f64) -> [Form; 4] {
        let (first, weights) = basis(t, self.start, self.end, self.controls.len() - 3);
        weights.map(|row| Form::combine((0..4).map(|i| (self.controls[first + i], row[i]))))
    }

    fn jerk(&self) -> impl Iterator<Item = (Form, f64)> + '_ {
        // This is the integral of the collocation integrand on the span:
        // jerk is constant even if a uniform time grid straddles a knot.
        let spans = self.controls.len() - 3;
        let h = (self.end - self.start) / spans as f64;
        (0..spans).map(move |i| (self.at(self.start + (i as f64 + 0.5) * h)[3], h))
    }
}

struct Flight {
    start: f64,
    end: f64,
    launch: [Form; 2],
    gravity: Vec3,
}

impl Flight {
    fn at(&self, t: f64) -> [Form; 4] {
        let dt = t - self.start;
        let [p, v] = self.launch;
        let mut pos = Form::combine([(p, 1.0), (v, dt)]);
        let mut vel = v;
        for (axis, g) in self.gravity.into_iter().enumerate() {
            pos.bias[axis] += 0.5 * g * dt * dt;
            vel.bias[axis] += g * dt;
        }
        [
            pos,
            vel,
            Form {
                bias: self.gravity,
                ..Form::default()
            },
            Form::default(),
        ]
    }
}

pub(crate) struct Layout {
    before: Option<Spline>,
    flight: Option<Flight>,
    after: Option<Spline>,
}

impl Layout {
    pub(crate) fn new(duration: f64, flight: Option<[f64; 2]>, gravity: Vec3) -> (Self, Vec<f64>) {
        let mut seeds = Vec::with_capacity(crate::KNOTS);
        let Some([start, end]) = flight else {
            let spans = crate::KNOTS - 3;
            let before = Some(Spline::new(0.0, duration, spans, &mut seeds, None));
            return (
                Self {
                    before,
                    flight: None,
                    after: None,
                },
                seeds,
            );
        };
        let spans = crate::KNOTS - 6;
        let remaining = duration - end;
        let before_spans = if remaining == 0.0 {
            crate::KNOTS - 3
        } else {
            ((spans as f64 * (start / (start + remaining))).round() as usize).clamp(1, spans - 1)
        };
        let before = (start > 0.0).then(|| Spline::new(0.0, start, before_spans, &mut seeds, None));
        let launch = match &before {
            Some(spline) => {
                let [p, v, _, _] = spline.at(start);
                [p, v]
            }
            None => {
                // With no preceding spline the launch position and velocity are free.
                // The two duplicate seed times give a zero velocity seed.
                seeds.extend([0.0, 0.0]);
                [Form::control(0), Form::control(1)]
            }
        };
        let flight = Flight {
            start,
            end,
            launch,
            gravity,
        };
        let after_spans = if before.is_some() {
            spans - before_spans.min(spans)
        } else {
            crate::KNOTS - 3
        };
        let after = (remaining > 0.0).then(|| {
            let [p, v, _, _] = flight.at(end);
            Spline::new(end, duration, after_spans, &mut seeds, Some([p, v]))
        });
        (
            Self {
                before,
                flight: Some(flight),
                after,
            },
            seeds,
        )
    }

    pub(crate) fn at(&self, t: f64) -> [Form; 4] {
        // If acceleration jumps, the new phase owns the switching time.
        if let Some(flight) = &self.flight {
            if t >= flight.start && (t < flight.end || self.after.is_none()) {
                return flight.at(t);
            }
            if let Some(after) = &self.after {
                if t >= flight.end {
                    return after.at(t);
                }
            }
        }
        self.before
            .as_ref()
            .expect("a spline covers the time before flight")
            .at(t)
    }

    pub(crate) fn jerk(&self) -> impl Iterator<Item = (Form, f64)> + '_ {
        self.before.iter().chain(&self.after).flat_map(Spline::jerk)
    }

    pub(crate) fn seed(&self, times: &[f64], anticipation: impl Fn(f64) -> Vec3) -> Vec<Vec3> {
        let mut controls: Vec<_> = times.iter().copied().map(anticipation).collect();
        if self.before.is_none() {
            controls[1] = [0.0; 3];
        }
        controls
    }
}
