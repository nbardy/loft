//! A second local interval for primary visibility. The existing proxy determines
//! rasterization and the first sample. This interval bounds samples the marcher
//! could accept, including positive distances inside its depth-dependent band.
#[path = "guard_range.rs"]
mod guard_range;
use crate::eval::{Eval, RowVal, Val};
use crate::lip::{Derived, Iv, PAD, PAD_REL};
use guard_range::Floats;
use loft_ir::{Res, Span};
use loft_parse::ast::{Block, Expr, ExprKind, FnDecl, Stmt, Ty};

// The stage-0 evaluator folds Xform-valued expressions into a concrete CPU
// matrix, outside the scalar interval domain. Only stored matrices and exact
// inverses can cross that boundary in a GPU certificate.
pub fn supports_expr(types: &[Ty], e: &Expr) -> bool {
    use ExprKind::*;
    let visit = |e: &Expr| supports_expr(types, e);
    if matches!(types.get(e.id as usize), Some(Ty::Xform(..))) {
        match &e.kind {
            Name(_) | Member(..) => {}
            Call { callee, .. } if callee.name == "inv" => {}
            _ => return false,
        }
    }
    match &e.kind {
        Num { .. } | Bool(_) | Name(_) => true,
        Member(e, _) | Unary(_, e) => visit(e),
        Index(a, b) | Binary(_, a, b) => visit(a) && visit(b),
        If {
            condition,
            then,
            otherwise,
            cases,
        } => {
            visit(condition)
                && cases.as_ref().is_none_or(|cases| cases.rest.iter().all(|(_, value)| visit(value)))
                && [then, otherwise]
                    .iter()
                    .all(|arm| supports_block(types, &arm.body) && visit(&arm.value))
        }
        Call { args, .. } | Ctor { args, .. } => args.iter().all(|a| visit(&a.value)),
        FiniteDiff { .. } | Unproved { .. } | String(_) => false,
    }
}

pub fn supports_block(types: &[Ty], block: &Block) -> bool {
    let visit = |e: &Expr| supports_expr(types, e);
    block.stmts.iter().all(|s| match s {
        Stmt::Let { value, .. } | Stmt::Expr(value) => visit(value),
        Stmt::Assign { target, value, .. } => visit(target) && visit(value),
        Stmt::Map { head, stages, .. } => {
            visit(head)
                && stages.iter().all(|(op, args)| {
                    matches!(op.name.as_str(), "translate" | "rot_x" | "rot_y" | "rot_z")
                        && args.iter().all(visit)
                })
        }
        Stmt::Return(e, _) => e.as_ref().is_none_or(visit),
        Stmt::If {
            cond,
            then,
            otherwise,
        } => {
            visit(cond)
                && supports_block(types, then)
                && otherwise.as_ref().is_none_or(|b| supports_block(types, b))
        }
        Stmt::Each { values, body, .. } => values.iter().all(visit) && supports_block(types, body),
        Stmt::For { lo, hi, body, .. } => visit(lo) && visit(hi) && supports_block(types, body),
        Stmt::Emit { .. } | Stmt::ImportTable { .. } => false,
    })
}

pub fn stored_transforms_exact(fields: &[&RowVal]) -> bool {
    fields.iter().all(|v| match v {
        RowVal::X(x) if x.t != [0.0; 3] => x.r.iter().all(|r| {
            r.iter().filter(|&&v| v.abs() == 1.0).count() == 1
                && r.iter().all(|v| *v == 0.0 || v.abs() == 1.0)
        }),
        _ => true,
    })
}

type Cell = ([f64; 3], [f64; 3]);
const AXES: [usize; 3] = [0, 1, 2];
const CELL_WIDTH: f64 = 1.0 / 32.0;
// The GPU checks the same eye radius. A more distant camera uses the outer interval.
pub const EYE_REACH: f32 = 32.0;

fn centre((lo, hi): Cell) -> [f64; 3] {
    AXES.map(|i| (lo[i] + hi[i]) * 0.5)
}

fn norm(p: [f64; 3]) -> f64 {
    p[0].hypot(p[1]).hypot(p[2])
}

fn include(hull: &mut Cell, cell: Cell) {
    for i in AXES {
        hull.0[i] = hull.0[i].min(cell.0[i]);
        hull.1[i] = hull.1[i].max(cell.1[i]);
    }
}

fn proof_cube(outer: &Derived, posed: bool) -> ([f64; 3], f64) {
    let origin = centre((outer.lo, outer.hi));
    let half = AXES.map(|i| (outer.hi[i] - outer.lo[i]) * 0.5);
    let radius = if posed {
        norm(half) * 3.0_f64.sqrt()
    } else {
        half.into_iter().fold(0.0, f64::max)
    };
    (origin, radius + PAD + PAD_REL * (radius + norm(origin)))
}

fn hit_band(origin: [f64; 3], cell: Cell) -> f64 {
    let mid = centre(cell);
    let r = 0.5 * norm(AXES.map(|i| cell.1[i] - cell.0[i]));
    let max_t = f64::from(EYE_REACH) + norm(AXES.map(|i| mid[i] - origin[i])) + r;
    0.0001 + 0.000201 * max_t
}

/// Try to shave a coarse hull without spending any more work on its interior.
/// Each cut needs a proof for its entire slab. An undecided leaf or exhausted
/// budget leaves the last proved face in place.
fn trim_faces(hull: &mut Cell, mut empty: impl FnMut(Cell) -> bool) -> u32 {
    let mut evals = 0;
    for axis in AXES {
        for low in [true, false] {
            let limit = evals + 2048;
            let face = |c: Cell| if low { c.0[axis] } else { c.1[axis] };
            let mut stop = if low { hull.1[axis] } else { hull.0[axis] };
            while (face(*hull) - stop).abs() > CELL_WIDTH && evals < limit {
                let split = 0.5 * (face(*hull) + stop);
                let mut slab = *hull;
                if low { slab.1[axis] = split; } else { slab.0[axis] = split; }
                let mut pending = vec![slab];
                let mut proved = true;
                while let Some(cell) = pending.pop() {
                    if evals == limit { proved = false; break; }
                    evals += 1;
                    if empty(cell) { continue; }
                    let widths = AXES.map(|i| cell.1[i] - cell.0[i]);
                    let i = AXES.into_iter().max_by(|&i, &j| widths[i].total_cmp(&widths[j])).unwrap();
                    if widths[i] <= CELL_WIDTH { proved = false; break; }
                    let mid = 0.5 * (cell.0[i] + cell.1[i]);
                    let (mut a, mut b) = (cell, cell);
                    a.1[i] = mid;
                    b.0[i] = mid;
                    pending.extend([a, b]);
                }
                if proved {
                    if low { hull.0[axis] = split; } else { hull.1[axis] = split; }
                } else {
                    stop = split;
                }
            }
        }
    }
    evals
}

/// The proof covers the whole raster box, even when rotating a local box enlarged
/// its world AABB. Cells left by the work limit are retained whole.
pub fn derive<'a>(
    ev: &mut Eval<'_, 'a, Iv>,
    trace: &'a FnDecl,
    kind: usize,
    row: usize,
    span: Span,
    outer: &Derived,
    posed: bool,
) -> Res<Derived> {
    let (origin, radius) = proof_cube(outer, posed);
    let root = (origin.map(|x| x - radius), origin.map(|x| x + radius));
    // The guard only accepts row-local geometry. The f32 evaluator needs a
    // single row even when the scene contains many thousands of other rows.
    let mut table = vec![vec![]; ev.tables.len()];
    table[kind].push(ev.tables[kind][row].clone());
    let mut gpu = Eval::<Floats>::new(ev.p, ev.types, table, ev.overrides);
    let mut outside = |cell: Cell| {
        let p = Val::V(AXES.map(|i| Floats(Iv { lo: cell.0[i], hi: cell.1[i], l: 0.0 })).to_vec(), 0.0);
        matches!(gpu.call_fn(trace, vec![Val::Row(kind, 0), p], span),
            Ok(Val::S(d)) if d.0.lo > hit_band(origin, cell) + PAD)
    };
    let mut evals = 0;
    let mut hull = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    let mut pending = std::collections::VecDeque::from([root]);
    while let Some(cell) = pending.pop_front() {
        if AXES
            .iter()
            .all(|&i| hull.0[i] <= cell.0[i] && hull.1[i] >= cell.1[i])
        {
            continue;
        }
        if evals >= 24_000 {
            include(&mut hull, cell);
            continue;
        }
        let mid = centre(cell);
        let widths = AXES.map(|i| cell.1[i] - cell.0[i]);
        let r = 0.5 * norm(widths);
        let band = hit_band(origin, cell);
        let mut evaluate = |(lo, hi): Cell| -> Res<Iv> {
            evals += 1;
            let point = Val::V(
                AXES.map(|i| Iv {
                    lo: lo[i],
                    hi: hi[i],
                    l: 1.0,
                })
                .to_vec(),
                1.0,
            );
            let Val::S(value) = ev.call_fn(trace, vec![Val::Row(kind, row), point], span)? else {
                unreachable!("checked scalar trace")
            };
            Ok(value)
        };
        let Ok(range) = evaluate(cell) else {
            // Some queries are proved only on the original local box.
            include(&mut hull, cell);
            continue;
        };
        // The outer L is only certified inside its original local box. Use the
        // cell's own L in the extra corners of a rotated raster proxy.
        let inside = AXES
            .iter()
            .all(|&i| cell.0[i] >= outer.lo[i] && cell.1[i] <= outer.hi[i]);
        let l = if inside {
            range.l.min(outer.l)
        } else {
            range.l
        };
        if range.lo > band
            || (l.is_finite() && evaluate((mid, mid)).is_ok_and(|d| d.lo - l * r > band))
        {
            // Confirm the proposed cut with outward arithmetic. In particular,
            // giant cancelling terms can move an f32 zero far outside the f64 hull.
            evals += 1;
            if outside(cell) {
                continue;
            }
        }
        if range.hi <= band || widths.into_iter().fold(0.0, f64::max) <= CELL_WIDTH {
            include(&mut hull, cell);
            continue;
        }
        // Resolve distant empty space first. A depth-first search can exhaust
        // its budget on one near octant and retain seven enormous siblings.
        let children: [Cell; 8] = std::array::from_fn(|k| (
            AXES.map(|i| if k >> i & 1 == 0 { cell.0[i] } else { mid[i] }),
            AXES.map(|i| if k >> i & 1 == 0 { mid[i] } else { cell.1[i] }),
        ));
        pending.extend(children);
    }
    if evals >= 24_000 && hull.0[0].is_finite() {
        evals += trim_faces(&mut hull, outside);
    }
    // An empty proof can use the outer box; this keeps the row record finite.
    let (mut lo, mut hi) = if hull.0[0].is_finite() { hull } else { root };
    for i in AXES {
        let pad = PAD
            + PAD_REL * (hi[i] - lo[i])
            + 16.0 * f64::from(f32::EPSILON) * lo[i].abs().max(hi[i].abs());
        lo[i] -= pad;
        hi[i] += pad;
    }
    Ok(Derived {
        lo,
        hi,
        l: outer.l,
        evals,
    })
}

/// Six local faces followed by the proof cube (centre and half-width).
pub fn words(outer: &Derived, guard: &Derived, posed: bool) -> [f32; 10] {
    let (origin, radius) = proof_cube(outer, posed);
    let [x, y, z] = origin.map(|x| x as f32);
    let [lx, ly, lz] = guard.lo.map(|x| (x as f32).next_down());
    let [hx, hy, hz] = guard.hi.map(|x| (x as f32).next_up());
    [lx, ly, lz, hx, hy, hz, x, y, z, (radius as f32).next_down()]
}
