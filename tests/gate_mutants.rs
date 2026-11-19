//! Run the production gate decisions on one changed observation at a time.
use super::*;
use std::process::Command;

#[test]
fn missing_frame() {
    let mut manifest: Manifest =
        serde_json::from_str(include_str!("../engines/gates.json")).unwrap();
    assert!(manifest.select(None, false).is_ok());
    manifest.refs.remove("capsules-1@0").unwrap();
    assert_eq!(manifest.select(None, false).err().unwrap().exit().0, 3);
}
fn score(gate: Gate, v: Verdict) -> Result<(), Failure> {
    result(&[Check {
        gate,
        scene: "fixture".into(),
        what: String::new(),
        verdict: v,
        contended: false,
    }])
}
#[test]
fn fake_foot_20mm() {
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/pin-drift.loft"
    );
    let check = |drift: &str| {
        crate::command(vec![
            "check".into(),
            fixture.into(),
            "--set".into(),
            format!("drift_m={drift}"),
        ])
    };
    check("0").unwrap();
    assert_eq!(check("0.020").unwrap_err().exit().0, 2);
}
#[test]
fn one_vs_20_module_drift() {
    let one = ["const N=1u;".into()];
    score(Gate::Load, module_parity(&one, &one)).unwrap();
    assert_eq!(
        score(Gate::Load, module_parity(&one, &["const N=20u;".into()]))
            .unwrap_err()
            .exit()
            .0,
        4
    );
}
#[test]
fn over_budget_51ms() {
    budget::measured(50., 50.).unwrap();
    assert_eq!(budget::measured(51., 50.).unwrap_err().exit().0, 7);
}
#[test]
fn core_domain_word_leak() {
    let root = tempfile::tempdir().unwrap();
    for dir in ["loft-runner", "loft-ir"] {
        std::fs::create_dir_all(root.path().join("crates").join(dir).join("src")).unwrap();
    }
    let scan = || {
        Command::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tools/check_core_words.sh"
        ))
        .arg(root.path())
        .status()
        .unwrap()
        .code()
    };
    assert_eq!(scan(), Some(0));
    std::fs::write(
        root.path().join("crates/loft-runner/src/lib.rs"),
        "pub fn gait() {}",
    )
    .unwrap();
    assert_eq!(scan(), Some(1));
}
