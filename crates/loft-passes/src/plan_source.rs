//! Source binding for the Stage 0 prior solver. The solver itself owns the
//! numerical representation and the convergence verdict.
use loft_ir::{Code, Diag, Res, Span};
use loft_parse::ast::*;
use loft_plan::{Hold, Objective, Plan};
use std::collections::{BTreeMap, HashSet};

use super::PriorRef;
use crate::Prog;

fn invalid(span: Span, message: &str) -> Diag {
    Diag::new(
        Code::E003,
        span,
        message,
        "use the typed plan objectives and holds in spec §9",
    )
}

fn objective_terms<'a>(e: &'a Expr, terms: &mut Vec<Weight<'a>>) -> Res<()> {
    if let ExprKind::Binary(BinOp::Add, left, right) = &e.kind {
        objective_terms(left, terms)?;
        objective_terms(right, terms)?;
    } else {
        terms.push(weight(e)?);
    }
    Ok(())
}

fn call(e: &Expr) -> Res<(&str, &[Arg])> {
    match &e.kind {
        ExprKind::Call { callee, args } => Ok((&callee.name, args)),
        _ => Err(invalid(e.span, "expected a plan objective or hold call")),
    }
}

fn bind<'a, const N: usize>(
    names: [&str; N],
    required: usize,
    args: &'a [Arg],
    span: Span,
) -> Res<[Option<&'a Expr>; N]> {
    let mut bound = [None; N];
    for (i, arg) in args.iter().enumerate() {
        let slot = match &arg.name {
            Some(name) => names.iter().position(|n| *n == name.name),
            None => (i < N).then_some(i),
        }
        .ok_or_else(|| invalid(arg.value.span, "unknown or excess plan argument"))?;
        if bound[slot].replace(&arg.value).is_some() {
            return Err(invalid(arg.value.span, "duplicate plan argument"));
        }
    }
    if bound[..required].iter().any(Option::is_none) {
        return Err(invalid(span, "required plan argument is missing"));
    }
    Ok(bound)
}

enum Weight<'a> {
    Effort(&'a Expr),
    Jerk(&'a Expr),
    Near(&'a Expr, [Option<&'a Expr>; 3]),
}

enum Constraint<'a> {
    Position(&'a Expr, &'a Expr, Option<&'a Expr>),
    Velocity(&'a Expr, &'a Expr, Option<&'a Expr>),
    Acceleration(&'a Expr, &'a Expr),
    Arrival(&'a Expr, &'a Expr, &'a Expr),
    Clearance(&'a Expr, &'a Expr),
    Landing(&'a Expr, &'a Expr),
    Pitch(&'a Expr, &'a Expr),
    Flight(&'a Expr, &'a Expr),
}

fn weight(e: &Expr) -> Res<Weight<'_>> {
    let (f, args) = call(e)?;
    if f == "near" {
        let [spline, pos, vel, acc] = bind(["spline", "pos", "vel", "acc"], 2, args, e.span)?;
        return Ok(Weight::Near(spline.unwrap(), [pos, vel, acc]));
    }
    let [w] = bind(["w"], 1, args, e.span)?;
    match f {
        "effort" => Ok(Weight::Effort(w.unwrap())),
        "jerk" => Ok(Weight::Jerk(w.unwrap())),
        _ => Err(invalid(e.span, "unknown plan objective")),
    }
}

fn constraint(e: &Expr) -> Res<Constraint<'_>> {
    let (f, args) = call(e)?;
    let s = e.span;
    let pair = |a, b| -> Res<(&Expr, &Expr)> {
        let [a, b] = bind([a, b], 2, args, s)?;
        Ok((a.unwrap(), b.unwrap()))
    };
    Ok(match f {
        "pos_at" => {
            let [t, p, tol] = bind(["t", "p", "tol"], 2, args, s)?;
            Constraint::Position(t.unwrap(), p.unwrap(), tol)
        }
        "vel_at" => {
            let [t, v, tol] = bind(["t", "v", "tol"], 2, args, s)?;
            Constraint::Velocity(t.unwrap(), v.unwrap(), tol)
        }
        "acc_at" => {
            let (t, a) = pair("t", "a")?;
            Constraint::Acceleration(t, a)
        }
        "arrive_at" => {
            let [t, p, v] = bind(["t", "p", "v"], 3, args, s)?;
            Constraint::Arrival(t.unwrap(), p.unwrap(), v.unwrap())
        }
        "clear" => {
            let (b, m) = pair("b", "margin")?;
            Constraint::Clearance(b, m)
        }
        "land_on" => {
            let (surf, t) = pair("surf", "at")?;
            Constraint::Landing(surf, t)
        }
        "pitch_in" => {
            let (lo, hi) = pair("lo", "hi")?;
            Constraint::Pitch(lo, hi)
        }
        "flight_ballistic" => {
            let (lo, hi) = pair("from", "to")?;
            Constraint::Flight(lo, hi)
        }
        _ => return Err(invalid(e.span, "unknown plan hold")),
    })
}

pub(crate) fn check_source(plan: &PlanDecl) -> Res<()> {
    if !matches!(&plan.ret, TyExpr::Name(n) if n.name == "PriorTable") {
        return Err(invalid(plan.name.span, "plan must return PriorTable"));
    }
    if let Some((knots, span)) = &plan.horizon
        && *knots as usize != loft_plan::KNOTS
    {
        return Err(invalid(span.span, "the v1 solver requires 24 knots"));
    }
    let mut names = HashSet::new();
    for field in &plan.params {
        if !names.insert(&field.name.name) {
            return Err(invalid(field.name.span, "duplicate plan parameter"));
        }
    }
    PlanSource::bind(plan).map(|_| ())
}

struct PlanSource<'a> {
    objectives: Vec<Weight<'a>>,
    holds: Vec<Constraint<'a>>,
}

impl<'a> PlanSource<'a> {
    fn bind(plan: &'a PlanDecl) -> Res<Self> {
        let mut objectives = vec![];
        objective_terms(&plan.objective, &mut objectives)?;
        let mut flight = false;
        let mut holds = Vec::with_capacity(plan.holds.len());
        for e in &plan.holds {
            let hold = constraint(e)?;
            if matches!(hold, Constraint::Flight(..)) && std::mem::replace(&mut flight, true) {
                return Err(invalid(e.span, "only one ballistic interval is supported"));
            }
            holds.push(hold);
        }
        Ok(Self { objectives, holds })
    }
}

enum Asset {
    Scalar(f64),
    Spline(Vec<[f64; 3]>),
    Box { center: [f64; 3], half: [f64; 3] },
    Surface { height: f64, velocity: [f64; 3] },
}

type Defaults<'a> = BTreeMap<&'a str, Asset>;

fn asset_args<'a>(e: &'a Expr, ctor: &str) -> Res<&'a [Arg]> {
    match &e.kind {
        ExprKind::Call { callee, args } if callee.name == ctor => Ok(args),
        _ => Err(invalid(
            e.span,
            &format!("expected {ctor}(...) for this plan input"),
        )),
    }
}

fn asset(e: &Expr, ty: &TyExpr, defaults: &Defaults<'_>) -> Res<Asset> {
    let name = match ty {
        TyExpr::Name(name) => name.name.as_str(),
        _ => return Err(invalid(ty.span(), "expected a scalar or plan asset type")),
    };
    Ok(match name {
        "PriorSpline" => {
            let args = asset_args(e, name)?;
            if args.len() < 4 || args.iter().any(|arg| arg.name.is_some()) {
                return Err(invalid(
                    e.span,
                    "a prior spline needs at least four control points",
                ));
            }
            Asset::Spline(
                args.iter()
                    .map(|arg| vector(&arg.value, defaults))
                    .collect::<Res<_>>()?,
            )
        }
        "Box" => {
            let [center, half] = bind(["center", "half"], 2, asset_args(e, name)?, e.span)?;
            Asset::Box {
                center: vector(center.unwrap(), defaults)?,
                half: vector(half.unwrap(), defaults)?,
            }
        }
        "SurfaceRow" => {
            let [height, velocity] = bind(["height", "velocity"], 2, asset_args(e, name)?, e.span)?;
            Asset::Surface {
                height: scalar(height.unwrap(), defaults)?,
                velocity: vector(velocity.unwrap(), defaults)?,
            }
        }
        "m" | "s" | "f" | "f32" | "rad" => Asset::Scalar(scalar(e, defaults)?),
        _ => return Err(invalid(ty.span(), "unknown plan input type")),
    })
}

fn named_asset<'a>(e: &Expr, defaults: &'a Defaults<'_>) -> Res<&'a Asset> {
    match &e.kind {
        ExprKind::Name(name) => defaults.get(name.as_str()),
        _ => None,
    }
    .ok_or_else(|| invalid(e.span, "expected a named plan asset parameter"))
}

pub(crate) fn closed(p: &Prog<'_>, plan: &PlanDecl) -> bool {
    plan.params.iter().all(|param| {
        param.default.is_some()
            || p.consts
                .get(param.name.name.as_str())
                .is_some_and(|c| c.comptime)
    })
}

fn scalar(e: &Expr, defaults: &Defaults<'_>) -> Res<f64> {
    let value = match &e.kind {
        ExprKind::Name(_) => match named_asset(e, defaults)? {
            Asset::Scalar(value) => *value,
            _ => return Err(invalid(e.span, "expected a scalar plan parameter")),
        },
        ExprKind::Num {
            value,
            suffix: NumSuffix::Deg,
            ..
        } => value.to_radians(),
        ExprKind::Num { value, .. } => *value,
        ExprKind::Unary(UnOp::Neg, inner) => -scalar(inner, defaults)?,
        ExprKind::Binary(op, a, b) => {
            let a = scalar(a, defaults)?;
            let b = scalar(b, defaults)?;
            match op {
                BinOp::Add => a + b,
                BinOp::Sub => a - b,
                BinOp::Mul => a * b,
                BinOp::Div => a / b,
                _ => return Err(invalid(e.span, "expected scalar arithmetic")),
            }
        }
        _ => return Err(invalid(e.span, "expected a numeric plan expression")),
    };
    if !value.is_finite() {
        return Err(invalid(e.span, "non-finite plan argument"));
    }
    Ok(value)
}

fn vector(e: &Expr, defaults: &Defaults<'_>) -> Res<[f64; 3]> {
    if let ExprKind::Ctor { args, .. } = &e.kind
        && let [x, y, z] = args.as_slice()
    {
        return Ok([
            scalar(&x.value, defaults)?,
            scalar(&y.value, defaults)?,
            scalar(&z.value, defaults)?,
        ]);
    }
    Err(invalid(e.span, "expected a three-component plan vector"))
}

pub(crate) fn lower_source(
    p: &Prog<'_>,
    decl: &PlanDecl,
    words: &mut Vec<f32>,
) -> Res<Option<PriorRef>> {
    if !closed(p, decl) {
        // An uninstantiated template has no curve or Truth verdict yet.
        return Ok(None);
    }
    let mut defaults = Defaults::new();
    for param in &decl.params {
        let name = param.name.name.as_str();
        let value = param
            .default
            .as_ref()
            .unwrap_or_else(|| &p.consts[name].value);
        defaults.insert(name, asset(value, &param.ty, &defaults)?);
    }
    let (_, duration) = decl
        .horizon
        .as_ref()
        .ok_or_else(|| invalid(decl.name.span, "missing plan horizon"))?;
    let duration = scalar(duration, &defaults)?;
    let num = |e| scalar(e, &defaults);
    let vec = |e| vector(e, &defaults);
    let tol = |e: Option<&Expr>| {
        e.map(|v| scalar(v, &defaults))
            .transpose()
            .map(|v| v.unwrap_or(0.001))
    };
    let source = PlanSource::bind(decl)?;
    let mut objectives = Vec::with_capacity(source.objectives.len());
    for weight in source.objectives {
        objectives.push(match weight {
            Weight::Effort(w) => Objective::Effort(num(w)?),
            Weight::Jerk(w) => Objective::Jerk(num(w)?),
            Weight::Near(spline, weights) => {
                let Asset::Spline(controls) = named_asset(spline, &defaults)? else {
                    return Err(invalid(spline.span, "near requires a PriorSpline"));
                };
                let [pos, vel, acc] = weights;
                Objective::Near {
                    controls,
                    weights: [
                        num(pos.unwrap())?,
                        vel.map(num).transpose()?.unwrap_or(0.0),
                        acc.map(num).transpose()?.unwrap_or(0.0),
                    ],
                }
            }
        });
    }
    let (mut holds, mut flight) = (vec![], None);
    for hold in source.holds {
        holds.push(match hold {
            Constraint::Position(t, p, l) => Hold::PosAt {
                time: num(t)?,
                position: vec(p)?,
                tolerance: tol(l)?,
            },
            Constraint::Velocity(t, v, l) => Hold::VelAt {
                time: num(t)?,
                velocity: vec(v)?,
                tolerance: tol(l)?,
            },
            Constraint::Acceleration(t, a) => Hold::AccAt {
                time: num(t)?,
                acceleration: vec(a)?,
            },
            Constraint::Arrival(t, p, v) => Hold::ArriveAt {
                time: num(t)?,
                position: vec(p)?,
                velocity: vec(v)?,
            },
            Constraint::Pitch(a, b) => Hold::PitchIn {
                lo: num(a)?,
                hi: num(b)?,
            },
            Constraint::Flight(a, b) => {
                flight = Some([num(a)?, num(b)?]);
                continue;
            }
            Constraint::Clearance(b, margin) => {
                let Asset::Box { center, half } = named_asset(b, &defaults)? else {
                    return Err(invalid(b.span, "clear requires a Box"));
                };
                Hold::Clear {
                    center: *center,
                    half: *half,
                    margin: num(margin)?,
                }
            }
            Constraint::Landing(surface, time) => {
                let Asset::Surface { height, velocity } = named_asset(surface, &defaults)? else {
                    return Err(invalid(surface.span, "land_on requires a SurfaceRow"));
                };
                Hold::LandOn {
                    time: num(time)?,
                    height: *height,
                    surface_velocity: *velocity,
                }
            }
        });
    }
    super::lower(
        &Plan {
            duration,
            gravity: [0., -9.81, 0.],
            flight,
            objectives: &objectives,
            holds: &holds,
        },
        0.,
        words,
    )
    .map(Some)
    .map_err(|_| {
        Diag::new(
            Code::E602,
            decl.name.span,
            "prior table exceeds the LUT word address space",
            "reduce the number of baked tables",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use loft_abi::VerdictTag;

    #[test]
    fn authored_hop_binds_assets_and_retains_the_crouch() {
        let text = r"
plan HeronHop(
  keys: PriorSpline = PriorSpline([0, 0.85m, 0]@World, [0, 0.26m, 0]@World,
                                  [0, 1.62m, 0]@World, [0, 0.85m, 0]@World),
  rail: Box = Box([2.0m, 0, 0]@World, [0.1m, 0.3m, 0.1m]),
  block: SurfaceRow = SurfaceRow(0.85m, [0, 0, 0]),
  dur: s = 2.4s
) -> PriorTable {
  knots: 24, duration: dur
  minimize effort(1.0) + jerk(0.25) + near(keys, pos: 1e-8)
  subject_to {
    pos_at(0s, [0, 0.85m, 0]@World, tol: 0m)
    vel_at(0s, [0, 0, 0]@World, tol: 0m)
    acc_at(0s, [0, 0, 0]@World)
    arrive_at(0.770112s, [0, 0.85m, 0]@World, [0, 3.876, 0]@World)
    acc_at(0.7701119999999999s, [0, 0, 0]@World)
    clear(rail, margin: 0.08m)
    land_on(block, at: dur)
    pitch_in(-12deg, 12deg)
    flight_ballistic(from: 0.770112s, to: 1.550112s)
  }
}";
        let module = loft_parse::parse(0, text, &mut 0).unwrap();
        let p = Prog::index(std::slice::from_ref(&module)).unwrap();
        let mut words = Vec::new();
        let prior = lower_source(&p, p.plans["HeronHop"], &mut words)
            .unwrap()
            .unwrap();
        assert_eq!(prior.verdict.tag, VerdictTag::Held as u32);
        let min = (0..=1200)
            .map(|i| {
                super::super::sample_prior_cpu(
                    &words,
                    prior.header_word,
                    0.770112 * i as f32 / 1200.0,
                )
                .unwrap()[0][1]
            })
            .fold(f32::INFINITY, f32::min);
        let depth = 0.85 - min;
        println!("authored HeronHop crouch {depth:.9} m");
        assert!((depth - 0.59).abs() < 0.005);
    }
}
