//! Regressions for two latent hazards of the guard table and the per-tile row lists.
use loft_passes::{CompileOptions, compile_with, emit::Aov, source::Graph};
use std::path::Path;

fn root() -> std::path::PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).join("../..") }

/// A kind with a guard and a kind whose field reads `time` (no guard), in one scene.
const BOTH: &str = r#"
use stdlib::materials::bsdf::{emit_lobe}
space World
kind Guarded @World () {
  trace(p) = length(p.xyz) - 0.2
  material(p,n) = emit_lobe([1.0,1.0,1.0])
}
kind Moving @World () {
  trace(p) = length(p.xyz) - select(0.2, 0.3, time > 3.0)
  material(p,n) = emit_lobe([1.0,1.0,1.0])
}
build {
  emit Guarded()
  emit Moving()
}
camera Main { at: [0,0,0]@World, radius: 3, fov: 40deg }
"#;

fn program(text: &str, aov: Aov) -> loft_passes::program::Program {
    let src = Graph::text(&root(), Path::new("scene.loft"), text);
    let cache = tempfile::tempdir().unwrap();
    let opts = CompileOptions::default().with_cache(cache.path()).with_aov(aov);
    compile_with(&src, &opts).unwrap_or_else(|e| panic!("{}", e.render(&src))).0
}

/// Bug caught: a table record for every traced row (40 bytes each) even for kinds with no guard, which pushes a large
/// scene of law-driven kinds past the storage-binding limit. Guarded records must be unchanged.
#[test]
fn unguarded_kind_has_no_records_and_guarded_records_are_unchanged() {
    let both = program(BOTH, Aov::Beauty);
    let marker = "Restrict only the exit";
    let guarded: Vec<_> = both.kinds.iter().filter(|k| both.modules[k.module].wgsl.contains(marker)).collect();
    assert_eq!(guarded.len(), 1, "exactly the static kind is guarded");
    // Header of one word per kind, then the guarded kind's single 10-word record, then nothing.
    assert_eq!(both.luts.len(), 2 + 10);
    let unguarded = both.kinds.iter().position(|k| !both.modules[k.module].wgsl.contains(marker)).unwrap();
    assert_eq!(both.luts[unguarded].to_bits(), 0);

    let alone = program(&BOTH.replace("  emit Moving()\n", "").replace("kind Moving", "kind Unused"), Aov::Beauty);
    assert_eq!(alone.luts.len(), 1 + 10);
    assert_eq!(&both.luts[2..], &alone.luts[1..]);

    let moving_only = program(&BOTH.replace("  emit Guarded()\n", ""), Aov::Beauty);
    assert!(moving_only.luts.is_empty(), "no guarded kind, no table");
}

/// Bug caught: stage 0 collected tile rows for any aov while only a Beauty module has the tile-aware vertex entry, so
/// compiling Albedo and loading Beauty drew tile lists through a shader that ignored them.
#[test]
fn tile_rows_exist_only_for_modules_emitted_in_tile_form() {
    let text = std::fs::read_to_string(root().join("stdlib/engines/capsules.loft")).unwrap();
    let beauty = program(&text, Aov::Beauty);
    assert!(!beauty.segments.is_empty(), "precondition: the capsule scene is tiled under Beauty");
    for &id in beauty.segments.keys() {
        assert!(beauty.modules[beauty.kinds[id as usize].module].wgsl.contains("vi / 36u"));
    }
    assert!(program(&text, Aov::Albedo).segments.is_empty());
}
