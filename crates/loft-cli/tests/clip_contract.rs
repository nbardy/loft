use serde_json::Value;
use std::{fs, path::Path, process::Command};

fn loft(args: &[&str], code: i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_loft")).args(args).output().unwrap();
    assert_eq!(out.status.code(), Some(code), "{}", String::from_utf8_lossy(&out.stderr));
}

const SCENE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../stdlib/engines/capsules.loft");

#[test]
fn corpus_build_and_clip_input_failures() {
    loft(&["build", SCENE, "--set", "count=1"], 0);
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("frames");
    let out = out.to_str().unwrap();
    let mut args =
        vec!["clip", SCENE, "--size", "16x16", "--frames", "1", "--fps", "1", "--out", out, "--max-build-ms", "0"];
    loft(&args, 7);
    args[1] = concat!(env!("CARGO_MANIFEST_DIR"), "/../loft-abi/tests/negative/E104.loft");
    loft(&args, 1);
    args[1] = "absent.loft";
    loft(&args, 5);
    args[1] = SCENE;
    args[7] = "NaN";
    loft(&args, 5);
    assert!(!Path::new(out).exists());
}

#[test]
fn training_command_writes_sixteen_pngs_and_measured_metadata() {
    let root = tempfile::tempdir().unwrap();
    let out = root.path().join("frames");
    fs::create_dir(&out).unwrap();
    let meta = out.join("meta.json");
    loft(
        &[
            "clip",
            SCENE,
            "--set",
            "count=1",
            "--out",
            out.to_str().unwrap(),
            "--json",
            meta.to_str().unwrap(),
            "--size",
            "512x512",
            "--frames",
            "16",
            "--fps",
            "16",
            "--max-build-ms",
            "2000",
            "--max-frame-ms",
            "50",
        ],
        0,
    );
    assert_eq!(out.read_dir().unwrap().count(), 17);
    for i in 0..16 {
        let file = fs::File::open(out.join(format!("frame_{i:03}.png"))).unwrap();
        let reader = png::Decoder::new(std::io::BufReader::new(file)).read_info().unwrap();
        assert_eq!((reader.info().width, reader.info().height), (512, 512));
    }
    let meta: Value = serde_json::from_slice(&fs::read(meta).unwrap()).unwrap();
    assert_eq!(meta.as_object().unwrap().len(), 9);
    for key in
        ["build_cpu_ms", "load_gpu_ms", "frame_gpu_p50_ms", "frame_gpu_p95_ms", "modules", "pipelines", "commands"]
    {
        assert!(meta[key].as_f64().unwrap() > 0., "{key}");
    }
    assert!(meta["build_cpu_ms"].as_f64().unwrap() <= 2000.);
    let p95 = meta["frame_gpu_p95_ms"].as_f64().unwrap();
    assert!((meta["frame_gpu_p50_ms"].as_f64().unwrap()..=50.).contains(&p95));
    assert_eq!(meta["identity_hash"].as_str().unwrap().len(), 64);
    assert!(meta["verdicts"].as_array().unwrap().is_empty());
}
