//! Stage 0 lowering for plan declarations. Offsets are word indices in LutTables.
use loft_abi::{VerdictReason, VerdictRecord, VerdictTag};
use loft_plan::{COLLOCATION, Plan};
pub(crate) fn lower_source(
    p: &crate::Prog<'_>, decl: &loft_parse::ast::PlanDecl, words: &mut Vec<f32>,
) -> loft_ir::Res<Option<PriorRef>> {
    loft_check::plan_source::lower_source(p, decl, words, lower)
}

const HEADER_WORDS: usize = 8;
const SAMPLE_WORDS: usize = 12;

#[derive(Debug)]
pub struct LutCapacity;

pub struct PriorRef {
    pub header_word: u32,
    pub verdict: VerdictRecord,
}

/// Append the 32-byte header and the uniform 48-byte Hermite rows. A failed
/// plan has a header with zero rows and the frozen failure verdict.
pub fn lower(plan: &Plan<'_>, compliance: f32, luts: &mut Vec<f32>) -> Result<PriorRef, LutCapacity> {
    let duration = plan.duration as f32;
    let samples = (|| {
        if !duration.is_finite() || duration <= 0.0 || !compliance.is_finite() || compliance < 0.0 {
            return None;
        }
        let curve = loft_plan::solve(plan).ok()?;
        let [sin, cos] = [curve.pitch().sin(), curve.pitch().cos()];
        // Pitch is about Body X. Use the half angle for the quaternion.
        let half = 0.5 * sin.atan2(cos);
        (0..=COLLOCATION)
            .map(|i| {
                let t = (i as f64 / COLLOCATION as f64) * plan.duration;
                let [p, v, _, _] = curve.at(t)?;
                let [x, y, z] = p.map(|x| x as f32);
                let [vx, vy, vz] = v.map(|v| v as f32);
                let row = [
                    x,
                    y,
                    z,
                    compliance,
                    vx,
                    vy,
                    vz,
                    1.0,
                    half.sin() as f32,
                    0.0,
                    0.0,
                    half.cos() as f32,
                ];
                row.into_iter().all(f32::is_finite).then_some(row)
            })
            .collect::<Option<Vec<_>>>()
    })();

    let header = luts.len().checked_next_multiple_of(4).ok_or(LutCapacity)?;
    let table = header.checked_add(HEADER_WORDS).ok_or(LutCapacity)?;
    let end = table.checked_add(samples.as_ref().map_or(0, |s| s.len() * SAMPLE_WORDS)).ok_or(LutCapacity)?;
    u32::try_from(end).map_err(|_| LutCapacity)?;
    let header_word = u32::try_from(header).map_err(|_| LutCapacity)?;
    let table_word = u32::try_from(table).map_err(|_| LutCapacity)?;
    luts.try_reserve(end - luts.len()).map_err(|_| LutCapacity)?;
    let (tag, reason) = if samples.is_some() {
        (VerdictTag::Held, VerdictReason::None)
    } else {
        (VerdictTag::Unknown, VerdictReason::SolveFailed)
    };
    let n = samples.as_ref().map_or(0, |v| v.len() as u32);
    luts.resize(header, 0.0);
    luts.extend([
        if n == 0 { 0.0 } else { duration },
        f32::from_bits(n),
        f32::from_bits(table_word),
        f32::from_bits(tag as u32),
        0.0,
        0.0,
        0.0,
        0.0,
    ]);
    luts.extend(samples.into_iter().flatten().flatten());
    Ok(PriorRef {
        header_word,
        verdict: VerdictRecord {
            tag: tag as u32,
            reason_code: reason as u32,
            ..VerdictRecord::default()
        },
    })
}

/// Cpu twin of the LUT reader. Words in each returned row are position and
/// compliance, velocity and active, then the pitch quaternion.
pub fn sample_prior_cpu(luts: &[f32], header_word: u32, time: f32) -> Option<[[f32; 4]; 3]> {
    let header = luts.get(header_word as usize..)?.get(..HEADER_WORDS)?;
    if header[3].to_bits() != VerdictTag::Held as u32 || !time.is_finite() {
        return None;
    }
    let duration = header[0];
    let spans = header[1].to_bits().checked_sub(1)?;
    if duration <= 0.0 || !duration.is_finite() || spans == 0 {
        return None;
    }
    let q = ((time / duration) * spans as f32).clamp(0.0, spans as f32);
    let i = (q as u32).min(spans - 1);
    let u = q - i as f32;
    let dt = duration / spans as f32;
    let first = (i as usize)
        .checked_mul(SAMPLE_WORDS)?
        .checked_add(header[2].to_bits() as usize)?;
    let rows = luts.get(first..)?.get(..2 * SAMPLE_WORDS)?;
    let mut out = [[0.0; 4]; 3];
    for axis in 0..3 {
        let p = rows[axis];
        let d = rows[SAMPLE_WORDS + axis] - p;
        let v0 = rows[4 + axis] * dt;
        let v1 = rows[SAMPLE_WORDS + 4 + axis] * dt;
        // Use the same explicit FMAs as WGSL. Metal otherwise contracts the
        // Hermite polynomial and rounds differently from the Rust twin.
        let b = 3.0f32.mul_add(d, (-2.0f32).mul_add(v0, -v1));
        let c = (-2.0f32).mul_add(d, v0 + v1);
        out[0][axis] = u.mul_add(u.mul_add(u.mul_add(c, b), v0), p);
        out[1][axis] = u.mul_add(u.mul_add(3.0 * c, 2.0 * b), v0) / dt;
    }
    out[0][3] = rows[3];
    out[1][3] = rows[7];
    out[2].copy_from_slice(&rows[8..12]);
    Some(out)
}

/// The emitter supplies its existing `luts: array<f32>` binding.
pub const PRIOR_WGSL: &str = r"
struct PriorSample {
  pos_compliance: vec4f,
  vel_active: vec4f,
  rot: vec4f,
}

fn prior_row(o: u32) -> vec4f {
  return vec4f(luts[o], luts[o + 1u], luts[o + 2u], luts[o + 3u]);
}

fn sample_prior(header: u32, t: f32) -> PriorSample {
  if bitcast<u32>(luts[header + 3u]) != 1u {
    return PriorSample(vec4f(0.0), vec4f(0.0), vec4f(0.0, 0.0, 0.0, 1.0));
  }
  let duration = luts[header];
  let spans = bitcast<u32>(luts[header + 1u]) - 1u;
  let q = clamp(t / duration * f32(spans), 0.0, f32(spans));
  let i = min(u32(q), spans - 1u);
  let u = q - f32(i);
  let dt = duration / f32(spans);
  let o = bitcast<u32>(luts[header + 2u]) + i * 12u;
  let p = prior_row(o);
  let v = prior_row(o + 4u);
  let d = prior_row(o + 12u).xyz - p.xyz;
  let v0 = v.xyz * dt;
  let v1 = prior_row(o + 16u).xyz * dt;
  let b = fma(vec3f(3.0), d, fma(vec3f(-2.0), v0, -v1));
  let c = fma(vec3f(-2.0), d, v0 + v1);
  let x = vec3f(u);
  return PriorSample(
    vec4f(fma(x, fma(x, fma(x, c, b), v0), p.xyz), p.w),
    vec4f(fma(x, fma(x, 3.0 * c, 2.0 * b), v0) / dt, v.w),
    prior_row(o + 8u)
  );
}
";

#[cfg(test)]
mod tests {
    use super::*;
    use loft_plan::{Hold, Objective};

    #[test]
    fn prior_format_and_cpu_twin() {
        let gravity = [0.0, -9.81, 0.0];
        let mut lut = vec![7.0, 8.0, 9.0];
        let request = Plan {
            duration: 1.0,
            gravity,
            flight: Some([0.0, 1.0]),
            objectives: &[Objective::Effort(1.0), Objective::Jerk(1.0)],
            holds: &[
                Hold::ArriveAt {
                    time: 0.0,
                    position: [0.0; 3],
                    velocity: [0.5, 4.0, 0.0],
                },
                Hold::PitchIn { lo: 0.1, hi: 0.2 },
            ],
        };
        let prior = lower(&request, 0.125, &mut lut).unwrap();
        assert_eq!(prior.verdict.tag, VerdictTag::Held as u32);
        assert_eq!(prior.header_word, 4);
        assert_eq!(&lut[..4], &[7.0, 8.0, 9.0, 0.0]);
        assert_eq!(
            &lut[4..12].iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            &[
                1.0f32.to_bits(),
                97,
                12,
                VerdictTag::Held as u32,
                0,
                0,
                0,
                0
            ]
        );
        assert_eq!(lut.len(), 12 + 48 * 97 / 4);
        let mut max_error = 0.0f32;
        for i in 0..=4096 {
            let t = i as f32 / 4096.0;
            let [p, v, q] = sample_prior_cpu(&lut, prior.header_word, t).unwrap();
            max_error = max_error.max((p[1] - t * (4.0 - 4.905 * t)).abs());
            assert!((p[0] - 0.5 * t).abs() < 1e-6);
            assert!((v[1] - (4.0 - 9.81 * t)).abs() < 2e-4);
            assert_eq!(p[3], 0.125);
            assert_eq!(v[3], 1.0);
            assert_eq!(q, [0.05f32.sin(), 0.0, 0.0, 0.05f32.cos()]);
        }
        println!("f32 Hermite flight position error {max_error:.3e} m");
        assert!(max_error < 1e-6);
        let bad = lower(
            &Plan {
                duration: 0.0,
                ..request
            },
            0.125,
            &mut lut,
        ).unwrap();
        assert_eq!(bad.verdict.tag, VerdictTag::Unknown as u32);
        assert_eq!(bad.verdict.reason_code, VerdictReason::SolveFailed as u32);
        assert_eq!(lut[bad.header_word as usize + 1].to_bits(), 0);
        assert_eq!(
            lut[bad.header_word as usize + 3].to_bits(),
            VerdictTag::Unknown as u32
        );
        assert!(sample_prior_cpu(&lut, bad.header_word, 0.5).is_none());
    }

    #[test]
    fn prior_reader_validates_in_naga() {
        let shader = format!(
            r"
@group(1) @binding(3) var<storage, read> luts: array<f32>;
@group(0) @binding(0) var<storage, read_write> result: array<vec4f>;
{PRIOR_WGSL}
@compute @workgroup_size(1)
fn fixture() {{ result[0] = sample_prior(0u, 0.3125).pos_compliance; }}
"
        );
        let module = naga::front::wgsl::parse_str(&shader).unwrap();
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap();
    }
}

#[cfg(test)]
mod source_tests {
    use super::*;
    use crate::Prog;
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
                super::sample_prior_cpu(
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
