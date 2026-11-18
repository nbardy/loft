//! Lane 2.3 pass/fail (lean Rust plan §8.2): the same verdicts as `src/ranged.ts` on its tests' checks and the
//! examples' (tunnelling claw: sampled pass, proved fail; the stone's forall-exists), margins within
//! `Tol::f32_rel(1e-4)`, witnesses within 0.02 s or 1% of the range. Input: `bun rust/crates/loft-core/dump_checks.ts >
//! out/lr23/checks.json`. Run with `--release -- --nocapture` for the table.
use super::ALL_PRIMS;
use loft_core::check::{Check, Formula, Quant, Scope, Strategy, Truth, Why, Work, check};
use loft_core::tape::{Id, Ssa};
use serde_json::Value;

fn u(v: &Value) -> usize {
    v.as_u64().unwrap() as usize
}
pub(super) fn graph(c: &Value) -> (Ssa, Vec<Vec<Id>>) {
    let mut s = Ssa::default();
    let mut comps: Vec<Vec<Id>> = Vec::new();
    for n in c["nodes"].as_array().unwrap() {
        let cs = match n["k"].as_str().unwrap() {
            "lit" => n["v"].as_array().unwrap().iter().map(|x| s.lit(x.as_f64().unwrap() as f32)).collect(),
            "leaf" => vec![s.leaf(u(&n["slot"]) as u32)],
            "call" => {
                let args: Vec<Vec<Id>> = n["args"].as_array().unwrap().iter().map(|a| comps[u(a)].clone()).collect();
                let refs: Vec<&[Id]> = args.iter().map(|a| a.as_slice()).collect();
                s.apply(ALL_PRIMS[u(&n["fn"])], &refs)
            }
            "vec" => {
                let parts: Vec<Id> = n["args"].as_array().unwrap().iter().flat_map(|a| comps[u(a)].clone()).collect();
                if parts.len() == 1 { vec![parts[0]; u(&n["w"])] } else { parts }
            }
            "get" => n["sel"].as_array().unwrap().iter().map(|k| comps[u(&n["a"])][u(k)]).collect(),
            k => panic!("node kind {k}"),
        };
        comps.push(cs);
    }
    (s, comps)
}
fn formula(f: &Value, comps: &[Vec<Id>]) -> Formula {
    let r = |v: &Value| comps[u(v)][0];
    match f["q"].as_str() {
        None => Formula::Atom { margin: r(&f["margin"]), test: r(&f["test"]) },
        Some(k) => {
            let q = Quant {
                leaf: u(&f["leaf"]) as u32,
                lo: r(&f["lo"]),
                hi: r(&f["hi"]),
                body: Box::new(formula(&f["body"], comps)),
            };
            if k == "always" { Formula::Always(q) } else { Formula::Eventually(q) }
        }
    }
}

#[test]
fn analyses_same_verdicts_as_ranged_ts() {
    let path = std::env::var("LR23_CHECKS")
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../../out/lr23/checks.json").into());
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("{path}: {e}. Write it first: bun rust/crates/loft-core/dump_checks.ts > out/lr23/checks.json")
    });
    let doc: Value = serde_json::from_str(&text).unwrap();
    let mut bad = vec![];
    println!(
        "{:<20} {:>9} {:>9} {:>12} {:>12} {:>24} {:>6} {:>6} {:>8}",
        "case", "ts", "rust", "ts margin", "rust margin", "rust witness", "ts box", "rs box", "rust ms"
    );
    for c in doc["cases"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let (ssa, comps) = graph(c);
        let held: Vec<f32> = c["held"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap() as f32).collect();
        let slots: Vec<&str> = c["slots"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        let body = formula(&c["formula"], &comps);
        let boxes = u(&c["boxes"]) as u32;
        let how = Strategy::Prove { budget: Work { boxes }, precision: c["precision"].as_f64().unwrap() };
        let t0 = std::time::Instant::now();
        let v = check(&ssa, &held, &Check { site: None, body, how }).unwrap();
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        let ts = &c["ts"];
        let want = match ts["holds"].as_bool() {
            Some(true) => "holds",
            Some(false) => "fails",
            None => "undecided",
        };
        let got = match v.truth {
            Truth::Holds => "holds",
            Truth::Fails => "fails",
            Truth::Undecided(_) => "undecided",
        };
        let wit: Vec<String> = v.witness.iter().map(|(k, x)| format!("{}={x:.6}", slots[*k as usize])).collect();
        println!(
            "{name:<20} {want:>9} {got:>9} {:>12.7} {:>12.7} {:>24} {:>6} {:>6} {ms:>8.2}",
            ts["margin"].as_f64().unwrap_or(f64::NAN),
            v.margin,
            wit.join(","),
            ts["boxes"],
            v.work.boxes
        );
        if want != got {
            bad.push(format!("{name}: truth {got}, TS {want}"));
        }
        if let Some(m) = ts["margin"].as_f64()
            // f32_rel(1e-4) with a floor of one f32 ulp at 1 for the zero margins (no `Tol` type exists yet).
            && ((v.margin as f64 - m).abs() > 1e-4 * m.abs() + f32::EPSILON as f64 || v.margin.is_nan())
        {
            bad.push(format!("{name}: margin {} vs TS {m}", v.margin));
        }
        // Witnesses: within 0.02 s for time (`--explain`), 1% of the range otherwise, for the leaves TS reports.
        if let Some(w) = ts["witness"].as_object() {
            for (label, x) in w {
                let k = slots.iter().position(|s| s == label).unwrap() as u32;
                let tol = if label == "time" { 0.02 } else { 0.01 * range_of(&c["formula"], k, &comps, &ssa, &held) };
                match v.witness.iter().find(|(j, _)| *j == k) {
                    Some((_, got)) if (*got as f64 - x.as_f64().unwrap()).abs() <= tol => {}
                    other => bad.push(format!("{name}: witness {label} {other:?} vs TS {x}")),
                }
            }
        }
        if v.work.boxes > boxes {
            bad.push(format!("{name}: {} boxes over the cap {boxes}", v.work.boxes));
        }
        if name.starts_with("budget") && !matches!(v.truth, Truth::Undecided(Why::Budget(_))) {
            bad.push(format!("{name}: {:?}, want Undecided(Budget)", v.truth));
        }
        if name == "claw" {
            println!("claw bounds {:?} truth {:?}", v.bounds, v.truth);
            // The tunnelling claw: 60 samples a second pass between two frames; the proof finds the pass-through.
            let (ssa2, comps2) = graph(c);
            let s = check(
                &ssa2,
                &held,
                &Check { site: None, body: formula(&c["formula"], &comps2), how: Strategy::Sample { n: 60 } },
            )
            .unwrap();
            println!("claw sampled at 60/s: {:?}, least margin {:.4} at t={:.4}", s.truth, s.margin, s.witness[0].1);
            if s.truth != Truth::Holds || s.scope != (Scope::Sampled { n: 61 }) || v.truth != Truth::Fails {
                bad.push(format!("claw: sampled {:?}, proved {:?}", s.truth, v.truth));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

fn range_of(f: &Value, k: u32, comps: &[Vec<Id>], ssa: &Ssa, held: &[f32]) -> f64 {
    let mut regs = vec![];
    loft_core::tape::run(ssa, 1, held.len(), held, &mut regs).unwrap();
    let mut q = f;
    while q["q"].is_string() {
        if u(&q["leaf"]) as u32 == k {
            return (regs[comps[u(&q["hi"])][0] as usize] - regs[comps[u(&q["lo"])][0] as usize]) as f64;
        }
        q = &q["body"];
    }
    0.0
}

/// A hit runs nothing; any change of job, options or commit is a different key; a refusal is not stored.
#[test]
fn analyses_program_cache() {
    use loft_core::cache::{Hit, key, through};
    let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../out/lr23/cache-test"));
    let _ = std::fs::remove_dir_all(&dir);
    let k = key(b"job", b"opts", "c0ffee");
    for other in [key(b"jo", b"bopts", "c0ffee"), key(b"job", b"opts!", "c0ffee"), key(b"job", b"opts", "c0ffef")] {
        assert_ne!(k, other);
    }
    let mut rounds = 0;
    let mut solve = || {
        rounds += 1;
        Ok(b"curves".to_vec())
    };
    assert_eq!(through(&dir, &k, &mut solve).unwrap(), (b"curves".to_vec(), Hit::Ran));
    assert_eq!(through(&dir, &k, &mut solve).unwrap(), (b"curves".to_vec(), Hit::Cached));
    assert_eq!(rounds, 1);
    let refused = key(b"bad", b"", "c0ffee");
    let fail = || Err(loft_ir::error::Error::Usage("no".into()));
    assert!(through(&dir, &refused, fail).is_err());
    assert_eq!(through(&dir, &refused, || Ok(vec![1])).unwrap().1, Hit::Ran);
    std::fs::remove_dir_all(&dir).unwrap();
}
