use std::{fs, path::Path, process::Command};

#[test]
fn cli_distinguishes_missing_input_frames_and_failed_truth() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let scratch = tempfile::TempDir::new().unwrap();
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_loft"))
            .current_dir(scratch.path()).args(args).output().unwrap();
        (output.status.code(), String::from_utf8_lossy(&output.stderr).into_owned())
    };
    assert_eq!(run(&["check", "absent.loft"]).0, Some(5));
    fs::copy(root.join("Cargo.toml"), scratch.path().join("Cargo.toml")).unwrap();
    for folder in ["lib", "engines"] {
        fs::create_dir(scratch.path().join(folder)).unwrap();
        for file in fs::read_dir(root.join(folder)).unwrap() {
            let file = file.unwrap();
            if file.path().extension().is_some_and(|e| e == "loft") {
                fs::copy(file.path(), scratch.path().join(folder).join(file.file_name())).unwrap();
            }
        }
    }
    let manifest = fs::read_to_string(root.join("engines/gates.json")).unwrap()
        .replace("\"timing_frames\": 300", "\"timing_frames\": 8");
    fs::write(scratch.path().join("engines/gates.json"), manifest).unwrap();
    let parity = run(&["gate", "--only", "capsules-1"]);
    assert_eq!(parity.0, Some(3), "a missing approved PNG must fail: {parity:?}");
    for verdict in ["missed(1.0)", "unknown(MissingSurface)"] {
        fs::write(scratch.path().join("check.loft"), format!("space World
kind Data @World () {{}}
check result(h: Data) -> f {{ {verdict} }}
build {{ emit Data() }}
schedule Main {{ check(result) }}")).unwrap();
        let truth = run(&["check", "check.loft", "--t", "0.5"]);
        assert_eq!(truth.0, Some(2), "{verdict}: {truth:?}");
    }
}
