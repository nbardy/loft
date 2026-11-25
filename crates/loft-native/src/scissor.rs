//! Per-frame bounds for an eight-row direct draw of the original proxy triangles.
use super::{M4, look_at, mul, perspective};
use loft_passes::program::Orbit;
use loft_runner::frame::ScissoredDraw;

pub(super) struct Projection {
    vp: M4,
    gradient: [f32; 3],
    size: [u32; 2],
}

impl Projection {
    pub(super) fn new(camera: Orbit, t: f32, size: (u32, u32)) -> Self {
        let vp = mul(
            perspective(camera.fov, size.0 as f32 / size.1 as f32, 0.05, 400.0),
            look_at(camera.eye(t), camera.at),
        );
        let norm =
            |j: usize| (vp[0][j] * vp[0][j] + vp[1][j] * vp[1][j] + vp[2][j] * vp[2][j]).sqrt();
        Self {
            vp,
            gradient: [norm(0), norm(1), norm(3)],
            size: [size.0, size.1],
        }
    }

    /// Projection of an endpoint ball. With the ball wholly in front of the
    /// eye, its perspective denominator is at least w - r * |grad(w)|.
    fn ball(&self, [x, y, z, r]: [f32; 4]) -> Option<([f32; 2], [f32; 2])> {
        let clip =
            |j: usize| self.vp[0][j] * x + self.vp[1][j] * y + self.vp[2][j] * z + self.vp[3][j];
        let w = clip(3);
        let r = if r >= 0.0 {
            r + 0.00001 * (1.0 + x.abs() + y.abs() + z.abs() + w.abs())
        } else {
            return None;
        };
        let near = w - r * self.gradient[2];
        if !(near > 0.05) {
            return None;
        }
        let ndc = [clip(0) / w, -clip(1) / w];
        let radius =
            [0, 1].map(|i| r * (self.gradient[i] + ndc[i].abs() * self.gradient[2]) / near);
        let screen = [0, 1].map(|i| 0.5 * self.size[i] as f32);
        let lo = [0, 1].map(|i| screen[i] * (1.0 + ndc[i] - radius[i]) - 2.0);
        let hi = [0, 1].map(|i| screen[i] * (1.0 + ndc[i] + radius[i]) + 2.0);
        lo.iter()
            .chain(&hi)
            .all(|x| x.is_finite())
            .then_some((lo, hi))
    }

    pub(super) fn draw(&self, rows: &[[[f32; 4]; 2]]) -> ScissoredDraw {
        let mut instances = Vec::with_capacity(rows.len().div_ceil(8));
        for (i, group) in rows.chunks(8).enumerate() {
            let mut lo = self.size;
            let mut hi = [0u32; 2];
            for &ball in group.iter().flatten() {
                let Some((a, b)) = self.ball(ball) else {
                    lo = [0; 2];
                    hi = self.size;
                    break;
                };
                for j in 0..2 {
                    lo[j] = lo[j].min(a[j].floor() as u32);
                    hi[j] = hi[j].max((b[j].ceil() as u32).min(self.size[j]));
                }
            }
            if hi[0] > lo[0] && hi[1] > lo[1] {
                let first = (8 * i) as u32;
                instances.push((
                    first..first + group.len() as u32,
                    [lo[0], lo[1], hi[0] - lo[0], hi[1] - lo[1]],
                ));
            }
        }
        ScissoredDraw {
            size: self.size,
            vertices: 36..72,
            instances,
        }
    }
}
