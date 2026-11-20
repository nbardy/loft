use loft_ir::{BindAccess, PassRun, Sources, Verdict};
use loft_parse::ast::{Determinism, GeometryProof, UnprovedReason};
use std::{fs, path::Path};

fn sources(scene: &str, body: &str) -> Sources {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = fs::read_dir(root.join("lib"))
        .unwrap()
        .map(|f| f.unwrap().path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "loft"))
        .collect::<Vec<_>>();
    files.sort();
    let mut src = Sources::default();
    for path in files {
        src.add(
            path.display().to_string(),
            fs::read_to_string(&path).unwrap(),
        );
    }
    src.add(
        root.join("crates/loft-passes/tests/escapes")
            .join(scene)
            .display()
            .to_string(),
        body,
    );
    src
}

#[test]
fn authored_compute_links_shader_access_and_reports_effects() {
    let src = sources("compute.loft", include_str!("escapes/compute.loft"));
    let (_, _, compiled) =
        loft_passes::compile(&src, &[]).unwrap_or_else(|d| panic!("{}", d.render(&src)));
    let out = compiled.compute;
    assert_eq!(out.determinism[&out.tables["Fixed"]], Determinism::Det);
    assert_eq!(out.determinism[&out.tables["Float"]], Determinism::NonDet);
    assert_eq!(
        out.truth["copy_rows"],
        Verdict::Unknown {
            reason: "NoCpuTwin".into()
        }
    );
    assert_eq!(out.bundle.passes.len(), 5);
    assert!(
        out.bundle.passes[2]
            .bindings
            .iter()
            .any(|b| b.group == 3 && b.access == BindAccess::SampledTex)
    );
    let pass = &out.bundle.passes[4];
    assert!(matches!(pass.run, PassRun::Compute { ref entry, .. } if entry == "copy_rows"));
    assert_eq!(
        pass.bindings
            .iter()
            .map(|b| (b.group, b.slot, b.access))
            .collect::<Vec<_>>(),
        [
            (1, 0, BindAccess::ReadStorage),
            (2, 0, BindAccess::ReadWriteStorage)
        ]
    );
}

#[test]
fn subgroup_operation_reaches_a_valid_compute_entry() {
    let scene = include_str!("escapes/compute.loft")
        .replace(
            "workgroup, dynamic_loops)",
            "workgroup, dynamic_loops, subgroups)",
        )
        .replace("i32(x + 1)", "i32(subgroup_add(x + 1))");
    let src = sources("compute.loft", &scene);
    let (_, _, compiled) =
        loft_passes::compile(&src, &[]).unwrap_or_else(|d| panic!("{}", d.render(&src)));
    let shader = std::str::from_utf8(&compiled.compute.bundle.modules[0].bytes).unwrap();
    assert!(shader.contains("subgroupAdd("));
}

#[test]
fn opted_out_geometry_has_an_explicit_step_and_numeric_normals() {
    let src = sources("geometry.loft", include_str!("escapes/geometry.loft"));
    let (program, report, compiled) =
        loft_passes::compile(&src, &[]).unwrap_or_else(|d| panic!("{}", d.render(&src)));
    for kind in &report.kinds {
        assert_eq!(
            kind.proof,
            GeometryProof::Unproved(UnprovedReason::AuthorOptOut)
        );
    }
    let sharp = program.kinds.iter().find(|k| k.name == "Sharp").unwrap();
    assert_eq!(sharp.rows[sharp.stride as usize - 1], 2.0);
    let patch = program
        .kinds
        .iter()
        .position(|k| k.name == "Patch")
        .unwrap();
    let query = compiled
        .cpu_query(patch, 0, [0.25, 0.5, 0.0], [0.0, 0.0, 1.0])
        .unwrap();
    assert!(query.surf_n[1] < -0.9);
    assert!(
        !program.modules[program.kinds[patch].module]
            .wgsl
            .contains("loft_surface_jvp")
    );
}

#[test]
fn baked_texture_sampling_emits_dimensioned_wgsl() {
    let src = sources(
        "bake.loft",
        r"
        space World
        bake_tex Mask 8x8 r16float on load mips(true) (uv: v2) { uv.x }
        fn mask(uv: v2) -> f = sample_lod(Mask, uv, 1.0)
        kind Shape @World () {
            trace(p) = sphere(p - [0, 0, 0]@World, 0.5) + 0.01 * sample(Mask, p.xz + [0.5, 0.5])
            material(p, n) = ggx(rgb(mask(p.xz) + sample(Mask, p.xz)), 0.7, 0)
        }
        build { emit Shape() }
    ",
    );
    let (program, _, compiled) =
        loft_passes::compile(&src, &[]).unwrap_or_else(|d| panic!("{}", d.render(&src)));
    let query = compiled.cpu_query(0, 0, [0.0; 3], [0.0, 1.0, 0.0]).unwrap();
    assert!((query.trace - (-0.495)).abs() < 1e-6);
    let shader = &program.modules[program.kinds[0].module].wgsl;
    assert!(shader.contains("textureSampleLevel(loft_bake_texture_Mask, loft_bake_sampler,"));
    assert!(shader.contains("@group(3) @binding(3) var loft_bake_texture_Mask: texture_2d<f32>"));
}
