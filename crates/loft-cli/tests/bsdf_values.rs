//! A material may blend let-bound Bsdf values and helpers' Bsdf results. Before Bsdf values composed, this
//! scene failed with E405 "material has no static lobe layout", the commonest Improve failure in crab-tools-v1c.
use std::{fs, path::Path, process::Command};

#[test]
fn materials_blend_bsdf_lets_and_helper_results() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let scratch = tempfile::TempDir::new().unwrap();
    fs::copy(root.join("../../Cargo.toml"), scratch.path().join("Cargo.toml")).unwrap();
    let scene = fs::read_to_string(root.join("tests/bsdf_values/composed.loft")).unwrap();
    let check = |text: &str| {
        fs::write(scratch.path().join("scene.loft"), text).unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_loft")).current_dir(scratch.path()).args(["check", "scene.loft"]).output().unwrap();
        (out.status.success(), String::from_utf8_lossy(&out.stderr).into_owned())
    };
    let (ok, stderr) = check(&scene);
    assert!(ok, "{stderr}");
    // A var holds one struct, so reassigning it to a different lobe stack is refused at the author site.
    let (ok, stderr) = check(&scene.replace("    lit\n", "    lit = lit + coat(0.5, 0.1)\n    lit\n"));
    assert!(!ok && stderr.contains("a Bsdf var keeps the lobes"), "{stderr}");
}
