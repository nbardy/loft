use std::{fs, path::Path, process::Command};

#[test]
fn missing_approved_frame_is_a_parity_failure() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let scratch = tempfile::TempDir::new().unwrap();
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_loft-verify"))
            .current_dir(scratch.path()).args(args).output().unwrap();
        (output.status.code(), String::from_utf8_lossy(&output.stderr).into_owned())
    };
    fs::copy(root.join("Cargo.toml"), scratch.path().join("Cargo.toml")).unwrap();
    fn copy_tree(source: &Path, destination: &Path) {
        fs::create_dir(destination).unwrap();
        for file in fs::read_dir(source).unwrap() {
            let file = file.unwrap();
            let path = destination.join(file.file_name());
            if file.file_type().unwrap().is_dir() {
                copy_tree(&file.path(), &path);
            } else {
                fs::copy(file.path(), path).unwrap();
            }
        }
    }
    copy_tree(&root.join("stdlib"), &scratch.path().join("stdlib"));
    fs::create_dir(scratch.path().join("engines")).unwrap();
    let manifest = fs::read_to_string(root.join("engines/gates.json")).unwrap()
        .replace("\"timing_frames\": 300", "\"timing_frames\": 8");
    fs::write(scratch.path().join("engines/gates.json"), manifest).unwrap();
    let parity = run(&["gate", "--only", "capsules-1"]);
    assert_eq!(parity.0, Some(3), "a missing approved PNG must fail: {parity:?}");
}
