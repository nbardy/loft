//! Evaluates the same checked kind queries used by the emitted draw modules.
use super::{Bvh2, Ray};
use crate::eval::{Eval, Rigid, RowVal, Val};
use crate::program::{Draw, KindTable, Program, abi};
use crate::{Compiled, Prog, pose_field, rot, unit, v3};
use loft_abi::LeafRef;
use loft_ir::{Code, Diag, Res, Span};
use loft_parse::ast::{FnDecl, QueryKind};

const STEPS: usize = 192;
const HIT_EPS: f32 = 1e-6;
const NORMAL_EPS: f32 = 1e-4;

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub leaf: LeafRef,
    pub t: f32,
    pub p: [f32; 3],
    pub local_p: [f32; 3],
    pub n: [f32; 3],
}

enum Intersect<'a> {
    Analytic(&'a FnDecl),
    Traced,
}

struct Kind<'p, 'a> {
    table: usize,
    rows: &'p KindTable,
    pose: Option<usize>,
    trace: &'a FnDecl,
    intersect: Intersect<'a>,
}

impl<'a> Kind<'_, 'a> {
    fn pose(&self, ev: &Eval<'_, 'a, f32>, leaf: LeafRef) -> Rigid {
        self.pose.map_or_else(
            || Rigid::yaw([0.0; 3], 0.0),
            |column| {
                let RowVal::X(pose) = ev.tables[self.table][leaf.row_id as usize][column] else {
                    unreachable!("a checked pose column")
                };
                pose
            },
        )
    }

    fn distance(&self, ev: &mut Eval<'_, 'a, f32>, leaf: LeafRef, p: [f32; 3]) -> Res<f32> {
        let span = self.trace.name.span;
        scalar(
            ev.call_fn(
                self.trace,
                vec![Val::Row(self.table, leaf.row_id as usize), v3(p)],
                span,
            )?,
            span,
        )
    }

    fn hit(&self, ev: &mut Eval<'_, 'a, f32>, leaf: LeafRef, ray: Ray) -> Res<Option<f32>> {
        let stride = self.rows.stride as usize;
        let start = leaf.row_id as usize * stride + stride - abi::HIDDEN_WORDS as usize;
        let [lx, ly, lz, hx, hy, hz, lip] = self.rows.rows[start..start + 7] else {
            unreachable!("packed derived columns")
        };
        let Some(span) = ray.clip([lx, ly, lz], [hx, hy, hz]) else {
            return Ok(None);
        };
        let xf = self.pose(ev, leaf).inv();
        let direction = rot(&xf, ray.direction());
        let local = |p| add(rot(&xf, p), xf.t);
        match self.intersect {
            Intersect::Analytic(f) => {
                // Shift only by the requested near plane. The closed form picks
                // its first forward root, which may be an exit after clipping.
                let at = f.name.span;
                let t = scalar(
                    ev.call_fn(
                        f,
                        vec![
                            Val::Row(self.table, leaf.row_id as usize),
                            v3(local(ray.at(*ray.interval().start()))),
                            v3(direction),
                        ],
                        at,
                    )?,
                    at,
                )?;
                let world_t = *ray.interval().start() + t;
                Ok((t >= 0.0 && span.accepts(world_t)).then_some(world_t))
            }
            Intersect::Traced => {
                let mut t = *span.interval().start();
                for _ in 0..STEPS {
                    // abs(d) is conservative on either side of the surface,
                    // including for rays that start in solid material.
                    let d = self.distance(ev, leaf, local(ray.at(t)))?.abs();
                    if d <= HIT_EPS {
                        return Ok(Some(t));
                    }
                    let next = t + d / lip;
                    if next > *span.interval().end() {
                        return Ok(None);
                    }
                    if next <= t {
                        break;
                    }
                    t = next;
                }
                Err(Diag::new(
                    Code::E701,
                    self.trace.name.span,
                    "world ray did not resolve its surface",
                    "inspect the field, interval and Lipschitz bound",
                ))
            }
        }
    }

    fn unpack(&self, ev: &mut Eval<'_, 'a, f32>, ray: Ray, leaf: LeafRef, t: f32) -> Res<Hit> {
        let p = ray.at(t);
        let pose = self.pose(ev, leaf);
        let xf = pose.inv();
        let local_p = add(rot(&xf, p), xf.t);
        let mut gradient = [0.0; 3];
        for i in 0..3 {
            let (mut lo, mut hi) = (local_p, local_p);
            lo[i] -= NORMAL_EPS;
            hi[i] += NORMAL_EPS;
            gradient[i] = self.distance(ev, leaf, hi)? - self.distance(ev, leaf, lo)?;
        }
        let n = rot(&pose, unit(gradient));
        if !n.into_iter().all(f32::is_finite) {
            return Err(Diag::new(
                Code::E701,
                self.trace.name.span,
                "world hit has no resolved normal",
                "inspect the signed field around the hit",
            ));
        }
        Ok(Hit {
            leaf,
            t,
            p,
            local_p,
            n,
        })
    }
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scalar(value: Val<f32>, span: Span) -> Res<f32> {
    let Val::S(d) = value else {
        unreachable!("checked scalar query")
    };
    if !d.is_finite() {
        return Err(Diag::new(
            Code::E701,
            span,
            "nonfinite world geometry query",
            "keep the field and hit distance finite",
        ));
    }
    Ok(d)
}

/// A selected world over a compiled snapshot. Independent worlds can share a
/// snapshot, with distinct geometry and a separately supplied medium query.
pub struct CpuWorld<'p, 'a> {
    bvh: Bvh2,
    kinds: Vec<Option<Kind<'p, 'a>>>,
    ev: Eval<'p, 'a, f32>,
}

impl<'p, 'a> CpuWorld<'p, 'a> {
    pub fn new(
        prog: &'p Prog<'a>,
        compiled: &'p Compiled,
        program: &'p Program,
        kinds: &[u32],
    ) -> Res<Self> {
        let mut selected: Vec<_> = program.kinds.iter().map(|_| None).collect();
        for &id in kinds {
            let rows = program.kinds.get(id as usize).ok_or_else(|| {
                Diag::new(
                    Code::E602,
                    Span::NONE,
                    format!("world kind ID {id} is absent"),
                    "select a kind in the compiled snapshot",
                )
            })?;
            let (table, name) = &compiled.drawn[id as usize];
            let decl = prog.kinds[name.as_str()];
            let intersect = match rows.draw {
                Draw::Analytic => {
                    Intersect::Analytic(decl.query(QueryKind::Visible).expect("checked visible"))
                }
                Draw::Traced => Intersect::Traced,
                Draw::Surface { .. } | Draw::Mesh { .. } => {
                    return Err(Diag::new(
                        Code::E405,
                        decl.name.span,
                        "raster surfaces and mesh assets have no CPU World intersector",
                        "supply a trace or analytic proxy for this world",
                    ));
                }
            };
            selected[id as usize] = Some(Kind {
                table: *table,
                rows,
                pose: pose_field(decl),
                trace: decl.query(QueryKind::Trace).expect("checked trace"),
                intersect,
            });
        }
        let instances: Vec<_> = program
            .instances
            .iter()
            .copied()
            .filter(|item| selected[item.kind as usize].is_some())
            .collect();
        let bvh = Bvh2::build(&instances).map_err(|e| {
            Diag::new(
                Code::E602,
                Span::NONE,
                format!("cannot build world bounds: {e:?}"),
                "inspect the selected instances",
            )
        })?;
        let mut ev = Eval::new(prog, &compiled.types, compiled.tables.clone(), &[]);
        ev.luts = &compiled.luts;
        Ok(Self {
            bvh,
            kinds: selected,
            ev,
        })
    }

    pub fn bvh(&self) -> &Bvh2 {
        &self.bvh
    }

    pub fn trace(&mut self, ray: Ray) -> Res<Option<Hit>> {
        let kinds = &self.kinds;
        let ev = &mut self.ev;
        let kind = |leaf: LeafRef| {
            kinds[leaf.kind_id as usize]
                .as_ref()
                .expect("selected kind")
        };
        self.bvh
            .trace(ray, |leaf, ray| kind(leaf).hit(ev, leaf, ray))?
            .map(|(leaf, t)| kind(leaf).unpack(ev, ray, leaf, t))
            .transpose()
    }

    /// Visibility of the open segment, multiplied by the caller's RGB medium
    /// transmittance. Bias the endpoints outward at the shading site as needed.
    pub fn visible(
        &mut self,
        a: [f32; 3],
        b: [f32; 3],
        transmittance: impl FnOnce([f32; 3], [f32; 3]) -> Res<[f32; 3]>,
    ) -> Res<[f32; 3]> {
        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let length = d.into_iter().map(|x| x * x).sum::<f32>().sqrt();
        if a == b && a.into_iter().all(f32::is_finite) {
            return Ok([1.0; 3]);
        }
        let ray = Ray::new(
            a,
            d.map(|x| x / length),
            0.0_f32.next_up(),
            length.next_down(),
        )
        .map_err(|e| {
            Diag::new(
                Code::E602,
                Span::NONE,
                format!("cannot trace visibility segment: {e:?}"),
                "supply two finite world points",
            )
        })?;
        let kinds = &self.kinds;
        let ev = &mut self.ev;
        if self.bvh.occluded(ray, |leaf, ray| {
            kinds[leaf.kind_id as usize]
                .as_ref()
                .expect("selected kind")
                .hit(ev, leaf, ray)
        })? {
            Ok([0.0; 3])
        } else {
            transmittance(a, b)
        }
    }
}
