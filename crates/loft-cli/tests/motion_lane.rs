#[test]
fn profile_json_reports_requested_frames() {
    let scene = concat!(env!("CARGO_MANIFEST_DIR"), "/../../stdlib/engines/capsules.loft");
    let dir = tempfile::tempdir().unwrap();
    let json = dir.path().join("profile.json");
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_loft"))
        .args(["profile", scene, "--frames", "3", "--size", "8x8", "--json"])
        .arg(&json)
        .status()
        .unwrap();
    assert!(status.success());
    let report: serde_json::Value = serde_json::from_slice(&std::fs::read(json).unwrap()).unwrap();
    assert_eq!(report["gpu_frame_ms"].as_array().unwrap().len(), 3);
}
