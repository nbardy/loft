//! A second check on cells the real-valued Lipschitz proof wants to discard.
//! Keep an outward interval at every f32 operation, including at the inputs of
//! a cancellation. A spatial pad added only to the final box cannot cover that.
use crate::eval::{Scalar, Un};
use crate::lip::Iv;
use loft_parse::ast::BinOp;

#[derive(Clone, Copy, Debug)]
pub(super) struct Floats(pub Iv);

impl Floats {
    fn full() -> Self {
        Self(Iv {
            lo: f64::NEG_INFINITY,
            hi: f64::INFINITY,
            l: 0.0,
        })
    }

    fn widen(v: Iv, error: f64) -> Self {
        if !error.is_finite() || v.lo < -f64::from(f32::MAX) || v.hi > f64::from(f32::MAX) {
            return Self::full();
        }
        let floor = f64::from(f32::MIN_POSITIVE);
        let low = ((v.lo - error) as f32).next_down();
        let high = ((v.hi + error) as f32).next_up();
        if !low.is_finite() || !high.is_finite() {
            return Self::full();
        }
        Self(Iv {
            lo: if f64::from(low).abs() < floor {
                f64::from(low).min(0.0)
            } else {
                low.into()
            },
            hi: if f64::from(high).abs() < floor {
                f64::from(high).max(0.0)
            } else {
                high.into()
            },
            l: 0.0,
        })
    }

    fn rounded(v: Iv, scale: f64) -> Self {
        Self::widen(v, 8.0 * f64::from(f32::EPSILON) * scale)
    }
}

impl Scalar for Floats {
    fn c(x: f64) -> Self {
        let x = x as f32;
        if !x.is_finite() {
            return Self::full();
        }
        let v = Iv::c(f64::from(x));
        if x.is_subnormal() {
            Self(v.join(&Iv::c(0.0)))
        } else {
            Self(v)
        }
    }
    fn frame_time() -> Self {
        Self::full()
    }
    fn add(&self, o: &Self) -> Self {
        let mut v = Self::rounded(self.0.add(&o.0), self.mag() + o.mag());
        if v.mag().is_finite() && self.0.lo >= 0.0 && o.0.lo >= 0.0 {
            v.0.lo = v.0.lo.max(0.0);
        }
        v
    }
    fn sub(&self, o: &Self) -> Self {
        Self::rounded(self.0.sub(&o.0), self.mag() + o.mag())
    }
    fn mul(&self, o: &Self) -> Self {
        if std::ptr::eq(self, o) {
            return self.sqr();
        }
        let v = self.0.mul(&o.0);
        Self::rounded(v, v.mag())
    }
    fn div(&self, o: &Self) -> Self {
        if o.mig() < f64::from(f32::MIN_POSITIVE) || o.mag() > 2f64.powi(126) {
            return Self::full();
        }
        let v = self.0.div(&o.0);
        Self::rounded(v, v.mag())
    }
    fn sqr(&self) -> Self {
        let v = self.0.sqr();
        let mut result = Self::rounded(v, v.mag());
        if result.mag().is_finite() {
            result.0.lo = result.0.lo.max(0.0);
        }
        result
    }
    fn neg(&self) -> Self {
        Self(self.0.neg())
    }
    fn un(&self, f: Un) -> Self {
        if !self.mag().is_finite() {
            return Self::full();
        }
        let v = self.0.un(f);
        match f {
            Un::Sign if self.0.lo <= 0.0 && self.0.hi >= 0.0 => Self(Iv {
                lo: -1.0,
                hi: 1.0,
                l: 0.0,
            }),
            Un::Abs | Un::Sign | Un::Floor => Self(v),
            Un::Sqrt if self.0.lo >= 0.0 => Self::rounded(v, v.mag()),
            Un::Sin | Un::Cos if self.mag() <= std::f64::consts::PI => Self::widen(v, 1.0 / 2048.0),
            Un::Fract => Self::rounded(v, self.mag() + 1.0),
            _ => Self::full(),
        }
    }
    fn min(&self, o: &Self) -> Self {
        // WGSL permits indeterminate results outside the finite domain.
        // A later clamp cannot restore a certificate for that computation.
        if !self.mag().is_finite() || !o.mag().is_finite() {
            return Self::full();
        }
        Self(self.0.min(&o.0))
    }
    fn max(&self, o: &Self) -> Self {
        if !self.mag().is_finite() || !o.mag().is_finite() {
            return Self::full();
        }
        Self(self.0.max(&o.0))
    }
    fn pow(&self, _: &Self) -> Self {
        Self::full()
    }
    fn atan2(&self, _: &Self) -> Self {
        Self::full()
    }
    fn cmp(&self, op: BinOp, o: &Self) -> Option<bool> {
        self.0.cmp(op, &o.0)
    }
    fn mag(&self) -> f64 {
        self.0.mag()
    }
    fn mig(&self) -> f64 {
        self.0.mig()
    }
    fn lip(&self) -> f64 {
        0.0
    }
    fn with_lip(self, _: f64) -> Self {
        self
    }
    fn exact(&self) -> Option<f64> {
        self.0.exact()
    }
    fn range(&self) -> (f64, f64) {
        self.0.range()
    }
    fn join(&self, o: &Self) -> Self {
        Self(self.0.join(&o.0))
    }
}
