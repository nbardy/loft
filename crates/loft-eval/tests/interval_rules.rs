//! The interval and Lipschitz rules added for the Auncel findings (F1 inverse trigonometry, F11 squares), sampled.
//! For random argument intervals, including ones that touch the singular points, every sampled value must lie in
//! the derived range and the worst slope the argument's L allows there must not exceed the derived L. A rule that
//! under-bounds either lets the sphere tracer step through a surface. A refusal (L = infinity) is always sound;
//! the counts show the rules still derive finite bounds where they exist.

use loft_eval::eval::{Scalar, Un};
use loft_eval::lip::Iv;

struct Rng(u64);

impl Rng {
    fn unit(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn within(&mut self, lo: f64, hi: f64) -> f64 {
        lo + self.unit() * (hi - lo)
    }
    /// An interval centred in [lo, hi] with a width from 1e-3 to 3, log-uniform.
    fn interval(&mut self, lo: f64, hi: f64) -> (f64, f64) {
        let c = self.within(lo, hi);
        let w = 10f64.powf(self.within(-3.0, 0.5));
        (c - 0.5 * w, c + 0.5 * w)
    }
}

const TRIALS: usize = 400;
const SAMPLES: usize = 25;
const SLACK: f64 = 1e-9;

struct Tally {
    finite: usize,
    refused: usize,
    samples: usize,
    worst: f64,
}

/// `f`, `slope` = |f'|, sampled over intervals of `centres`; returns how tight and how often finite.
fn unary(name: &str, rule: impl Fn(Iv) -> Iv, f: fn(f64) -> f64, slope: fn(f64) -> f64, centres: (f64, f64)) -> Tally {
    let mut rng = Rng(0x5eed ^ name.len() as u64);
    let mut t = Tally { finite: 0, refused: 0, samples: 0, worst: 0.0 };
    for _ in 0..TRIALS {
        let (lo, hi) = rng.interval(centres.0, centres.1);
        let l = rng.within(0.0, 3.0);
        let out = rule(Iv { lo, hi, l });
        if !out.l.is_finite() {
            t.refused += 1;
            continue;
        }
        t.finite += 1;
        for _ in 0..SAMPLES {
            let x = rng.within(lo, hi);
            let (v, s) = (f(x), slope(x) * l);
            assert!(v >= out.lo - SLACK * (1.0 + v.abs()) && v <= out.hi + SLACK * (1.0 + v.abs()),
                "{name}: f({x}) = {v} outside [{}, {}] for [{lo}, {hi}]", out.lo, out.hi);
            assert!(s <= out.l * (1.0 + SLACK) + SLACK, "{name}: slope {s} at {x} above L {} for [{lo}, {hi}] l {l}", out.l);
            if out.l > 0.0 {
                t.worst = t.worst.max(s / out.l);
            }
            t.samples += 1;
        }
    }
    eprintln!("{name:6} finite {:3}  refused {:3}  samples {:5}  worst slope / L {:.4}", t.finite, t.refused, t.samples, t.worst);
    t
}

#[test]
fn inverse_trig_log_and_powers_bound_value_and_slope() {
    let un = |f: Un| move |x: Iv| x.un(f);
    let cases = [
        unary("atan", un(Un::Atan), f64::atan, |x| 1.0 / (1.0 + x * x), (-10.0, 10.0)),
        unary("asin", un(Un::Asin), f64::asin, |x| 1.0 / (1.0 - x * x).sqrt(), (-1.2, 1.2)),
        unary("acos", un(Un::Acos), f64::acos, |x| 1.0 / (1.0 - x * x).sqrt(), (-1.2, 1.2)),
        unary("tan", un(Un::Tan), f64::tan, |x| 1.0 + x.tan() * x.tan(), (-4.0, 4.0)),
        unary("log", un(Un::Log), f64::ln, |x| 1.0 / x, (-0.5, 5.0)),
        unary("sqr", |x: Iv| x.sqr(), |x| x * x, |x| 2.0 * x.abs(), (-3.0, 3.0)),
        unary("pow2", |x: Iv| x.pow(&Iv::c(2.0)), |x| x * x, |x| 2.0 * x.abs(), (-3.0, 3.0)),
        unary("pow3", |x: Iv| x.pow(&Iv::c(3.0)), |x| x * x * x, |x| 3.0 * x * x, (-3.0, 3.0)),
        unary("pow4", |x: Iv| x.pow(&Iv::c(4.0)), |x| x.powi(4), |x| 4.0 * x.abs().powi(3), (-2.0, 2.0)),
    ];
    for t in cases {
        // Not vacuous: most intervals have a finite bound, and the bound is reached to within a few times.
        assert!(t.finite > TRIALS / 4 && t.worst > 0.2, "finite {} worst {}", t.finite, t.worst);
    }
    // The squares never go negative, which is what kept `1 + 8 * min(x * x, 0.35)` from being a safe divisor.
    assert!(Iv { lo: -1.0, hi: 1.0, l: 1.0 }.sqr().lo == 0.0);
    assert!(Iv { lo: -1.0, hi: 1.0, l: 1.0 }.pow(&Iv::c(2.0)).lo == 0.0);
}

#[test]
fn atan2_is_bounded_away_from_the_origin_and_the_cut_and_refused_across_them() {
    let mut rng = Rng(0xa7a2);
    let (mut finite, mut refused, mut samples, mut worst) = (0, 0, 0, 0.0f64);
    for _ in 0..TRIALS * 2 {
        let (y0, y1) = rng.interval(-3.0, 3.0);
        let (x0, x1) = rng.interval(-3.0, 3.0);
        let (ly, lx) = (rng.within(0.0, 2.0), rng.within(0.0, 2.0));
        let out = Iv { lo: y0, hi: y1, l: ly }.atan2(&Iv { lo: x0, hi: x1, l: lx });
        let crosses = (x0 <= 0.0 && x1 >= 0.0 && y0 <= 0.0 && y1 >= 0.0) || (x0 < 0.0 && y0 <= 0.0 && y1 >= 0.0);
        assert_eq!(crosses, !out.l.is_finite(), "y [{y0}, {y1}] x [{x0}, {x1}]");
        if crosses {
            refused += 1;
            continue;
        }
        finite += 1;
        for _ in 0..SAMPLES {
            let (y, x) = (rng.within(y0, y1), rng.within(x0, x1));
            let v = y.atan2(x);
            assert!(v >= out.lo - SLACK && v <= out.hi + SLACK, "atan2({y}, {x}) = {v} outside [{}, {}]", out.lo, out.hi);
            // x and y may move along one direction: the slope is at most (|y| lx + |x| ly) / r².
            let s = (y.abs() * lx + x.abs() * ly) / (x * x + y * y);
            assert!(s <= out.l * (1.0 + SLACK) + SLACK, "atan2 slope {s} at ({y}, {x}) above L {}", out.l);
            if out.l > 0.0 {
                worst = worst.max(s / out.l);
            }
            samples += 1;
        }
    }
    eprintln!("atan2  finite {finite:3}  refused {refused:3}  samples {samples:5}  worst slope / L {worst:.4}");
    assert!(finite > TRIALS / 2 && worst > 0.5);
    // A per-frame angle (neither argument moves with the point) has no slope even across the cut.
    let heading = Iv { lo: -1.0, hi: 1.0, l: 0.0 }.atan2(&Iv { lo: -1.0, hi: 1.0, l: 0.0 });
    assert_eq!((heading.l, heading.lo, heading.hi), (0.0, -std::f64::consts::PI, std::f64::consts::PI));
}
