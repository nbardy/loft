use std::{fs, path::Path, process::Command};

#[test]
fn cli_distinguishes_missing_input_and_failed_truth() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let scratch = tempfile::TempDir::new().unwrap();
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_loft"))
            .current_dir(scratch.path()).args(args).output().unwrap();
        (output.status.code(), String::from_utf8_lossy(&output.stderr).into_owned())
    };
    assert_eq!(run(&["check", "absent.loft"]).0, Some(5));
    fs::copy(root.join("Cargo.toml"), scratch.path().join("Cargo.toml")).unwrap();
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
