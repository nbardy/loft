//! Lane 2.3's solve pass/fail (lean Rust plan §8.2), through the driver: each corpus scene with holds as the wire jobs
//! `solve()` will send (a Solve and a Check, written by `bun rust/crates/loft-core/dump_solves.ts > out/lr23/solves.json`
//! into out/lr23/jobs), run by `loft_core::run`. The outcome is `src/geom/solve.ts`'s (the curves, a certified conflict
//! naming the same holds, or a stopped search naming the same hold); the returned curves are within
//! `Tol::f32_rel(1e-4)` of the TS curves at 181 times; the Solve job leaves every hold `Unproved`, and the Check job's
//! proof of each has the same truth as the TS ranged check of it. Run with `--release -- solve --nocapture` for the
//! table. Proof budget: `LR23_BOXES` (default 200,000).
use loft_core::check::{Truth, Why};
use loft_core::solve::{Proof, curve};
use loft_core::tape::{Ssa, run};
use loft_core::{Build, Naga, Precision};
use loft_ir::error::{Error, Limits, SceneFault};
use loft_ir::graph::Site;
use serde_json::Value;

/// A tolerance relative to a scale, at least 1e-4: f32 holds noise near 1e-7, and tighter checks failed correct runs.
struct Tol(f64);
impl Tol {
    fn f32_rel(rel: f64) -> Tol {
        assert!(rel >= 1e-4, "an f32 relative tolerance below 1e-4");
        Tol(rel)
    }
    fn within(&self, a: f64, b: f64, scale: f64) -> bool {
        (a - b).abs() <= self.0 * scale
    }
}

fn u(v: &Value) -> usize {
    v.as_u64().unwrap() as usize
}
fn f(v: &Value) -> f64 {
    v.as_f64().unwrap()
}
fn site(v: &Value) -> Site {
    let s = v.as_str().unwrap();
    match s.rsplit_once(':').map(|(file, l)| (file, l.parse::<u32>())) {
        Some((file, Ok(line))) => Site { file: file.into(), line, col: 0 },
        Some((_, Err(_))) | None => Site { file: s.into(), line: 0, col: 0 },
    }
}

#[test]
fn solve_same_as_ts() {
    let out = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../out/lr23");
    let path = std::env::var("LR23_SOLVES").unwrap_or_else(|_| format!("{out}/solves.json"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("{path}: {e}. Write it first: bun rust/crates/loft-core/dump_solves.ts > out/lr23/solves.json")
    });
    let doc: Value = serde_json::from_str(&text).unwrap();
    let boxes = std::env::var("LR23_BOXES").map_or(200_000, |v| v.parse().unwrap());
    let limits = Limits { nodes: u64::MAX, tape_ops: u64::MAX, boxes, gpu_nanos: u64::MAX };
    let build = Build { naga: Naga::Load, precision: Precision::F32 };
    let mut bad = vec![];
    for c in doc["cases"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let job = |ext: &str| {
            let file = format!("{out}/jobs/{}.{ext}.job", name.replace(' ', "_"));
            std::fs::read(&file).unwrap_or_else(|e| panic!("{file}: {e}. Write it: bun rust/crates/loft-core/dump_solves.ts"))
        };
        let p = &c["solve"];
        let over = [f(&p["over"][0]), f(&p["over"][1])];
        let holds: Vec<Site> = p["holds"].as_array().unwrap().iter().map(|h| site(&h["site"])).collect();
        let got = loft_core::run(&job("solve"), limits, build, "test").map(|r| r.0);
        let ts = &c["ts"];
        let named = |i: &Value| holds[u(i)].clone();
        match (ts["outcome"].as_str().unwrap(), got) {
            ("solved", Ok(report)) => {
                let sol = &report.solves[0];
                let ms = report.phases.solve as f64 / 1e6;
                if !sol.holds.iter().all(|(_, h)| *h == Proof::Unproved) {
                    bad.push(format!("{name}: the Solve job proved a hold"));
                }
                // The returned curves at the TS times, through the f32 twin.
                let mut v = Ssa::default();
                let t = v.leaf(0);
                let roots: Vec<_> = sol.curves.iter().map(|k| curve(&mut v, t, over, &k.1)).collect();
                let at: Vec<f32> = ts["at"].as_array().unwrap().iter().map(|x| f(x) as f32).collect();
                let mut regs = vec![];
                run(&v, at.len(), 1, &at, &mut regs).unwrap();
                let want: Vec<Vec<f64>> = ts["curves"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|k| k.as_array().unwrap().iter().map(f).collect())
                    .collect();
                let scale = want.iter().flatten().fold(1.0f64, |m, x| m.max(x.abs()));
                let mut worst = 0.0f64;
                for (k, w) in want.iter().enumerate() {
                    for (l, &x) in w.iter().enumerate() {
                        let y = regs[roots[k] as usize * at.len() + l] as f64;
                        worst = worst.max((x - y).abs() / scale);
                        if !Tol::f32_rel(1e-4).within(x, y, scale) {
                            bad.push(format!("{name}: unknown {k} at t = {}: {y} vs TS {x}", at[l]));
                        }
                    }
                }
                // The Check job: the same curves, every hold proved on them.
                let checked = match loft_core::run(&job("check"), limits, build, "test") {
                    Ok((r, _)) => r,
                    Err(e) => {
                        bad.push(format!("{name}: the Check job: {e:?}"));
                        continue;
                    }
                };
                let proved = &checked.solves[0];
                if proved.curves != sol.curves {
                    bad.push(format!("{name}: the Check job solved other curves than the Solve job"));
                }
                let mut same = 0;
                for (i, ((_, h), tv)) in proved.holds.iter().zip(ts["verdicts"].as_array().unwrap()).enumerate() {
                    let Proof::Checked(vd) = h else {
                        bad.push(format!("{name}: hold {i} unproved by the Check job"));
                        continue;
                    };
                    // Plan §2.2(b)'s listed exception: a TS margin within the tolerance may become WithinPrecision.
                    let close = tv["margin"].as_f64().is_some_and(|m| Tol::f32_rel(1e-4).within(m, 0.0, scale));
                    let ok = match (&vd.truth, tv["holds"].as_bool()) {
                        (Truth::Holds, Some(true)) | (Truth::Fails, Some(false)) | (Truth::Undecided(_), None) => true,
                        (Truth::Undecided(Why::WithinPrecision), Some(true)) if close => {
                            println!("  listed: {name} hold {i} WithinPrecision, TS margin {}", tv["margin"]);
                            true
                        }
                        (Truth::Holds | Truth::Fails | Truth::Undecided(_), _) => false,
                    };
                    same += ok as usize;
                    if !ok {
                        bad.push(format!("{name}: hold {i} {:?} (margin {}), TS {}", vd.truth, vd.margin, tv["holds"]));
                    }
                }
                let proof_boxes: u32 = proved
                    .holds
                    .iter()
                    .map(|(_, h)| match h {
                        Proof::Checked(v) => v.work.boxes,
                        Proof::Unproved => 0,
                    })
                    .sum();
                println!(
                    "{name:<20} solved: curves within {worst:.2e} of scale {scale:.2} (TS spread {:.1e}); verdicts {same}/{}; rounds {} (TS {}), steps {} (TS {}); solve {ms:.0} ms, {:.1}M tape ops (TS {:.0} ms); proofs {:.0} ms, {proof_boxes} boxes",
                    f(&ts["spread"]),
                    proved.holds.len(),
                    sol.rounds,
                    ts["rounds"],
                    sol.steps,
                    ts["steps"],
                    sol.tape_ops as f64 / 1e6,
                    f(&ts["ms"]),
                    checked.phases.prove as f64 / 1e6,
                );
            }
            ("infeasible", Err(Error::Scene { fault: SceneFault::Infeasible { a, b }, .. })) => {
                // As multisets: mistake 35's two holds are on one line.
                let (mut want, mut got): (Vec<Site>, _) =
                    (ts["holds"].as_array().unwrap().iter().map(named).collect(), vec![a.clone(), b.clone()]);
                got.dedup_by(|x, y| x == y && want.len() == 1);
                let key = |s: &Site| (s.file.clone(), s.line);
                want.sort_by_key(key);
                got.sort_by_key(key);
                println!("{name:<20} infeasible: {a:?} and {b:?} (TS holds {})", ts["holds"]);
                if want != got {
                    bad.push(format!("{name}: conflict {a:?} {b:?}, TS holds {}", ts["holds"]));
                }
            }
            ("unsolved search", Err(Error::Scene { site: Some(at), fault: SceneFault::Unsolved { fix } })) => {
                println!("{name:<20} unsolved at {at:?}: {fix} (TS hold {})", ts["holds"]);
                if at != named(&ts["holds"][0]) {
                    bad.push(format!("{name}: stopped at {at:?}, TS hold {}", ts["holds"]));
                }
            }
            (want, got) => bad.push(format!("{name}: {:?}, TS {want}", got.map(|r| r.solves.len()))),
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// `solve`'s fast f64 evaluation equals the interval rule at a point, bit for bit, for every atom: run each atom over
/// a grid of operands that meets the rules' edges (0, -0, signs, NaN, infinities, domain ends) through both.
#[test]
fn point_fast_same() {
    use loft_core::prim::atom;
    use loft_core::solve::{eval, point};
    let xs = [0.0, -0.0, 1.0, -1.0, 0.5, -2.5, 3.0, 1e-300, f64::INFINITY, f64::NEG_INFINITY, f64::NAN, 7.25];
    let mut bad = vec![];
    for &a in super::ALL_ATOMS.iter() {
        let arity = atom(a).arity as usize;
        let mut s = Ssa::default();
        let ops = [s.leaf(0), s.leaf(1), s.leaf(2)];
        let r = s.op(a, std::array::from_fn(|j| if j < arity { ops[j] } else { 0 }));
        let mut lanes = vec![];
        for &p in &xs {
            for &q in &xs {
                for &t in &xs {
                    lanes.push([p, q, t]);
                }
            }
        }
        let leaves: Vec<f64> = lanes.iter().flatten().copied().collect();
        let mut got = vec![];
        eval(&s, lanes.len(), 3, &leaves, &mut got);
        for (l, x) in lanes.iter().enumerate() {
            let x = std::array::from_fn(|j| if j < arity { x[j] } else { 0.0 });
            let (fast, want) = (got[r as usize * lanes.len() + l], point(a, x));
            if fast.to_bits() != want.to_bits() && !(fast.is_nan() && want.is_nan()) {
                bad.push(format!("{a:?}{x:?}: {fast} vs rule {want}"));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}
