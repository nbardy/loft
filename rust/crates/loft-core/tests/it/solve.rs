//! Lane 2.3's solve pass/fail (lean Rust plan §8.2): on every corpus scene with holds, the same outcome as
//! `src/geom/solve.ts` (the curves, a certified conflict naming the same holds, or a stopped search naming the same
//! hold), the returned curves within `Tol::f32_rel(1e-4)` of the TS curves at 181 times, and each hold's proof the same
//! truth as the TS ranged check of it. Input: `bun rust/crates/loft-core/dump_solves.ts > out/lr23/solves.json`.
//! Run with `--release -- solve --nocapture` for the table. Proof budget: `LR23_BOXES` (default 200,000).
use super::analyses::graph;
use loft_core::check::{Truth, Why, Work};
use loft_core::solve::{Family, Hold, Kind, Prefer, Solve, curve, solve};
use loft_core::tape::{Ssa, run};
use loft_ir::error::{Error, SceneFault};
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
    let path = std::env::var("LR23_SOLVES")
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../../out/lr23/solves.json").into());
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("{path}: {e}. Write it first: bun rust/crates/loft-core/dump_solves.ts > out/lr23/solves.json")
    });
    let doc: Value = serde_json::from_str(&text).unwrap();
    let mut bad = vec![];
    for c in doc["cases"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let (ssa, comps) = graph(c);
        let (p, r) = (&c["solve"], |v: &Value| comps[u(v)][0]);
        assert_eq!(c["slots"][0], "time");
        let holds: Vec<Hold> = p["holds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| Hold {
                site: site(&h["site"]),
                kind: h["within"].as_f64().map_or(Kind::Bound, |within| Kind::Equal { within }),
                margin: comps[u(&h["margin"])].clone(),
                test: r(&h["test"]),
                robust: r(&h["check"]),
                times: [f(&h["times"][0]), f(&h["times"][1])],
            })
            .collect();
        let s = Solve {
            ssa,
            held: c["held"].as_array().unwrap().iter().map(|v| f(v) as f32).collect(),
            time: 0,
            jets: 1,
            over: [f(&p["over"][0]), f(&p["over"][1])],
            vary: p["vary"].as_array().unwrap().iter().map(|v| Family { n: u(&v["n"]), from: r(&v["from"]) }).collect(),
            holds,
            prefer: Prefer {
                jerk: f(&p["jerk"]),
                acceleration: f(&p["acceleration"]),
                near_weight: f(&p["near_weight"]),
                near: p["near"].as_array().unwrap().iter().map(|z| (u(&z[0]), r(&z[1]))).collect(),
            },
            delta: f(&p["delta"]),
            tape_ops: u64::MAX,
            boxes: Work { boxes: std::env::var("LR23_BOXES").map_or(200_000, |v| v.parse().unwrap()) },
            precision: 1e-4,
        };
        let start = std::time::Instant::now();
        let got = solve(&s);
        let ms = start.elapsed().as_secs_f64() * 1e3;
        let ts = &c["ts"];
        let named = |i: &Value| s.holds[u(i)].site.clone();
        match (ts["outcome"].as_str().unwrap(), got) {
            ("solved", Ok(sol)) => {
                // The returned curves at the TS times, through the f32 twin.
                let mut v = Ssa::default();
                let t = v.leaf(0);
                let roots: Vec<_> = sol.controls.iter().map(|k| curve(&mut v, t, s.over, k)).collect();
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
                // TS's own answer moves by `spread` when delta moves by 1e-9 relative (heron moved: 0.21 m). Beyond the
                // tolerance the reference does not determine its curves, so the gate there is the reference's spread.
                let spread = f(&ts["spread"]);
                let indeterminate = !Tol::f32_rel(1e-4).within(spread, 0.0, scale);
                let mut worst = 0.0f64;
                for (k, w) in want.iter().enumerate() {
                    for (l, &x) in w.iter().enumerate() {
                        let y = regs[roots[k] as usize * at.len() + l] as f64;
                        worst = worst.max((x - y).abs() / scale);
                        if !Tol::f32_rel(1e-4).within(x, y, scale) && !(indeterminate && (x - y).abs() <= spread) {
                            bad.push(format!("{name}: unknown {k} at t = {}: {y} vs TS {x}", at[l]));
                        }
                    }
                }
                let mut same = 0;
                for (i, (vd, tv)) in sol.verdicts.iter().zip(ts["verdicts"].as_array().unwrap()).enumerate() {
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
                let boxes: u32 = sol.verdicts.iter().map(|v| v.work.boxes).sum();
                println!(
                    "{name:<20} solved: curves within {worst:.2e} of scale {scale:.2} (TS spread {spread:.1e}); verdicts {same}/{}; rounds {} (TS {}), steps {} (TS {}); {ms:.0} ms (TS solve {:.0} ms), {boxes} proof boxes",
                    sol.verdicts.len(),
                    sol.rounds,
                    ts["rounds"],
                    sol.steps,
                    ts["steps"],
                    f(&ts["ms"])
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
                let ok = want == got;
                println!("{name:<20} infeasible: {a:?} and {b:?} (TS holds {}), {ms:.0} ms", ts["holds"]);
                if !ok {
                    bad.push(format!("{name}: conflict {a:?} {b:?}, TS holds {}", ts["holds"]));
                }
            }
            ("unsolved search", Err(Error::Scene { site: Some(at), fault: SceneFault::Unsolved { fix } })) => {
                println!("{name:<20} unsolved at {at:?}: {fix} (TS hold {}), {ms:.0} ms", ts["holds"]);
                if at != named(&ts["holds"][0]) {
                    bad.push(format!("{name}: stopped at {at:?}, TS hold {}", ts["holds"]));
                }
            }
            (want, got) => bad.push(format!("{name}: {:?}, TS {want}", got.map(|s| s.rounds))),
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}
