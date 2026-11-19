use loft_abi::{VerdictReason, VerdictTag};
use loft_plan::{COLLOCATION, Hold, Objective, Plan, solve};

const ZERO: [f64; 3] = [0.0; 3];
const GRAVITY: [f64; 3] = [0.0, -9.81, 0.0];
const TWO_DECIMAL_METRES: f64 = 0.005;

fn y(value: f64) -> [f64; 3] {
    [0.0, value, 0.0]
}

#[test]
fn heron_crouch_and_ballistic_flight() {
    // Scientist's strength-limited hop boundary data (DUMP §4.6).
    let takeoff: f64 = 0.770112;
    let touchdown = takeoff + 0.78;
    let duration = 2.4;
    let rest = 0.85;
    let speed = 3.876;
    let curve = solve(&Plan {
        duration,
        gravity: GRAVITY,
        flight: Some([takeoff, touchdown]),
        objectives: &[Objective::Effort(1.0), Objective::Jerk(0.25)],
        holds: &[
            Hold::PosAt {
                time: 0.0,
                position: y(rest),
                tolerance: 0.0,
            },
            Hold::VelAt {
                time: 0.0,
                velocity: ZERO,
                tolerance: 0.0,
            },
            Hold::AccAt {
                time: 0.0,
                acceleration: ZERO,
            },
            Hold::ArriveAt {
                time: takeoff,
                position: y(rest),
                velocity: y(speed),
            },
            Hold::AccAt {
                time: takeoff.next_down(),
                acceleration: ZERO,
            },
            Hold::LandOn {
                time: duration,
                height: rest,
                surface_velocity: ZERO,
            },
        ],
    })
    .unwrap();
    let min = (0..=12_000)
        .map(|i| curve.at(takeoff * f64::from(i) / 12_000.0).unwrap()[0][1])
        .fold(f64::INFINITY, f64::min);
    let depth = rest - min;
    println!(
        "heron crouch {depth:.9} m, maximum hold residual {:.3e}",
        curve.residual
    );
    assert!((depth - 0.59).abs() < TWO_DECIMAL_METRES);

    let [launch, velocity, _, _] = curve.at(takeoff).unwrap();
    assert!((launch[1] - rest).abs() <= 1e-5);
    assert!((velocity[1] - speed).abs() <= 1e-5);
    let mut max_error = 0.0f64;
    for i in 0..512 {
        let dt = 0.78 * f64::from(i) / 512.0;
        let [p, v, a, j] = curve.at(takeoff + dt).unwrap();
        let expected = launch[1] + dt * (velocity[1] - 4.905 * dt);
        max_error = max_error.max((p[1] - expected).abs());
        assert!((v[1] - (velocity[1] - 9.81 * dt)).abs() < 1e-12);
        assert_eq!(a, GRAVITY);
        assert_eq!(j, ZERO);
    }
    println!("flight position error {max_error:.3e} m");
    assert!(max_error < 1e-12);
    for switch in [takeoff, touchdown] {
        let left = curve.at(switch.next_down()).unwrap();
        let right = curve.at(switch).unwrap();
        for d in 0..2 {
            for axis in 0..3 {
                assert!((left[d][axis] - right[d][axis]).abs() < 1e-12);
            }
        }
    }
    let [p, v, _, _] = curve.at(duration).unwrap();
    assert!((p[1] - rest).abs() <= 1e-5);
    assert!(v.into_iter().all(|v| v.abs() <= 1e-5));
}

#[test]
fn boss_windup_from_arrival_only() {
    let duration = 0.7;
    let curve = solve(&Plan {
        duration,
        gravity: GRAVITY,
        flight: None,
        objectives: &[Objective::Jerk(1.0)],
        holds: &[
            Hold::PosAt {
                time: 0.0,
                position: ZERO,
                tolerance: 0.0,
            },
            Hold::VelAt {
                time: 0.0,
                velocity: ZERO,
                tolerance: 0.0,
            },
            Hold::AccAt {
                time: 0.0,
                acceleration: ZERO,
            },
            Hold::ArriveAt {
                time: duration,
                position: y(-3.0),
                velocity: y(-20.0),
            },
            Hold::AccAt {
                time: duration,
                acceleration: ZERO,
            },
        ],
    })
    .unwrap();
    let (windup, time) = (0..=12_000)
        .map(|i| {
            let t = duration * f64::from(i) / 12_000.0;
            (curve.at(t).unwrap()[0][1], t)
        })
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .unwrap();
    println!(
        "boss windup {windup:.9} m at {time:.7} s, maximum hold residual {:.3e}",
        curve.residual
    );
    assert!((windup - 0.69).abs() < TWO_DECIMAL_METRES);
    assert!((time - 0.366).abs() < 0.001);
    let [p, v, a, _] = curve.at(duration).unwrap();
    assert!((p[1] + 3.0).abs() <= 1e-5);
    assert!((v[1] + 20.0).abs() <= 1e-5);
    assert!(a[1].abs() <= 1e-5);
}

#[test]
fn effort_has_gravity_as_its_zero() {
    let curve = solve(&Plan {
        duration: 1.0,
        gravity: GRAVITY,
        flight: None,
        objectives: &[Objective::Effort(1.0), Objective::Jerk(1e-7)],
        holds: &[
            Hold::PosAt {
                time: 0.0,
                position: ZERO,
                tolerance: 0.0,
            },
            Hold::VelAt {
                time: 0.0,
                velocity: ZERO,
                tolerance: 0.0,
            },
        ],
    })
    .unwrap();
    let [p, v, a, j] = curve.at(0.75).unwrap();
    assert!((p[1] + 4.905 * 0.75f64.powi(2)).abs() <= 1e-5);
    assert!((v[1] + 9.81 * 0.75).abs() <= 1e-5);
    assert!((a[1] + 9.81).abs() <= 1e-5);
    assert!(j[1].abs() <= 1e-5);
}

#[test]
fn active_clearance_and_tolerance() {
    let curve = solve(&Plan {
        duration: 1.0,
        gravity: ZERO,
        flight: None,
        objectives: &[
            Objective::Near {
                controls: &[ZERO; 4],
                weights: [1.0, 0.5, 0.01],
            },
            Objective::Jerk(1e-3),
        ],
        holds: &[
            Hold::Clear {
                center: ZERO,
                half: [0.1, 10.0, 10.0],
                margin: 0.2,
            },
            Hold::PosAt {
                time: 0.0,
                position: ZERO,
                tolerance: 0.4,
            },
            Hold::PitchIn { lo: 0.1, hi: 0.2 },
        ],
    })
    .unwrap();
    for i in 0..COLLOCATION {
        let p = curve.at((i as f64 + 0.5) / COLLOCATION as f64).unwrap()[0];
        assert!((p[0] - 0.3).abs() <= 1e-5, "{p:?}");
    }
    assert_eq!(curve.pitch(), 0.1);
}

#[test]
fn invalid_and_infeasible_requests_fail_closed() {
    for (duration, holds) in [
        (f64::NAN, &[][..]),
        (
            1.0,
            &[
                Hold::PosAt {
                    time: 0.5,
                    position: ZERO,
                    tolerance: 0.0,
                },
                Hold::PosAt {
                    time: 0.5,
                    position: y(1.0),
                    tolerance: 0.0,
                },
            ][..],
        ),
    ] {
        let failure = solve(&Plan {
            duration,
            gravity: GRAVITY,
            flight: None,
            objectives: &[Objective::Effort(1.0)],
            holds,
        })
        .err()
        .expect("failed solve");
        assert_eq!(failure.tag, VerdictTag::Unknown as u32);
        assert_eq!(failure.reason_code, VerdictReason::SolveFailed as u32);
    }
}
