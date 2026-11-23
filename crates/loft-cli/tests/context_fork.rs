use std::{fs, path::Path, process::{Command, Output}};

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
const GUIDE: &str = include_str!("../../../docs/context.md");

fn run(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_loft"))
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap()
}

fn stdout(output: Output) -> String {
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn context_is_compact_and_every_loft_fence_checks() {
    let root = Path::new(ROOT);
    let context = stdout(run(root, &["context"]));
    assert_eq!(context, stdout(run(root, &["context", "--section", "all"])));
    let words = context.split_whitespace().count();
    assert!(words <= 2400, "context has {words} words");
    assert!(context.starts_with("# Loft authoring context\n"));
    assert!(context.contains("pub fn sphere[S](v:v3@S,r:f)->f\n"));
    assert!(context.contains("pub kind Sun@World(dir:d3@World,color:rgb)"));
    assert!(context.contains("pub schedule Realtime"));
    assert!(context.contains("pub schedule SpectralCaustic {type Radiance=v4; orbit("));
    // Pass calls keep their arguments; a bare `find_shadow_map` hid its required `size:`.
    assert!(context.contains("find_shadow_map(Casters,size:1024)"));
    assert!(context.contains("# Hit.t is -1 on a miss and -2 on unresolved marches.\npub struct Hit"));
    let builtins = context.lines().find_map(|line| line.strip_prefix("Builtins (always in scope, never imported): ")).unwrap();
    for name in ["held", "missed"] {
        assert!(builtins.split_whitespace().any(|word| word == name));
    }
    assert!(context.contains("Types: f32 u32 i32"));
    // Lobes print at material-call arity; the declared n, v, l tail made models pass six arguments to ggx.
    assert!(context.contains("pub lobe ggx(albedo:rgb,rough:f,metal:f)\n"));
    assert!(context.contains("pub lobe lambert(albedo:rgb)\n"));
    assert!(context.contains("pub law step(w:Walker,dt:s) writes w.{"));

    let passes = stdout(run(root, &["context", "--section", "passes"]));
    assert!(passes.contains("pub schedule Realtime"));
    assert!(!passes.contains("pub fn sphere"));
    let invalid = run(root, &["context", "--section", "unknown"]);
    assert_eq!(invalid.status.code(), Some(5));

    let scratch = tempfile::tempdir().unwrap();
    let scene = scratch.path().join("snippet.loft");
    let mut count = 0;
    for fence in GUIDE.split("\n```loft\n").skip(1) {
        let (snippet, _) = fence.split_once("\n```").expect("closing Loft fence");
        fs::write(&scene, snippet).unwrap();
        stdout(run(root, &["check", scene.to_str().unwrap()]));
        count += 1;
    }
    assert!(count > 0, "no Loft examples to check");
}

#[test]
fn fork_is_editable_and_compiles_from_outside_the_workspace() {
    let root = Path::new(ROOT);
    let scratch = tempfile::tempdir().unwrap();
    let fork = scratch.path().join("my_engine.loft");
    let dest = fork.to_str().unwrap();
    let import = stdout(run(root, &["fork", "engines/realtime", dest]));
    assert_eq!(import.trim(), "use self::my_engine");
    let stock = fs::read_to_string(root.join("stdlib/engines/realtime.loft")).unwrap();
    assert_eq!(fs::read_to_string(&fork).unwrap(), stock);

    let edited = stock.replace("size: 1024", "size: 512");
    assert_ne!(edited, stock);
    fs::write(&fork, &edited).unwrap();
    stdout(run(scratch.path(), &["check", dest]));
    let scene = scratch.path().join("scene.loft");
    fs::write(&scene, format!("{import}
use stdlib::sdf::solid::{{sphere}}
use stdlib::materials::bsdf::{{lambert}}
kind Ball @World () {{
  trace(p) = sphere(p - [0, 0, 0]@World, 0.4)
  material(p, n) = lambert([0.4, 0.3, 0.2])
}}
build {{ emit Ball() }}
")).unwrap();
    stdout(run(scratch.path(), &["check", scene.to_str().unwrap()]));

    let clobber = run(root, &["fork", "stdlib/engines/realtime.loft", dest]);
    assert_eq!(clobber.status.code(), Some(5));
    assert_eq!(fs::read_to_string(&fork).unwrap(), edited);
    stdout(run(root, &["fork", "stdlib::engines::realtime", dest, "--force"]));
    assert_eq!(fs::read_to_string(&fork).unwrap(), stock);
}
