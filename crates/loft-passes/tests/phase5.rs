use loft_abi::MeshAsset;
use loft_ir::{Code, Sources, Span};
use loft_passes::{Prog, front, gltf::GltfScene};

#[test]
fn scene_names_resolve_to_bundle_indices() {
    let mut scene = GltfScene::default();
    scene.meshes.insert("Anchor".into(), MeshAsset(0xff12_3456));
    assert_eq!(scene.mesh("Anchor", Span::NONE).unwrap(), MeshAsset(0xff12_3456));
    let site = Span { file: 3, lo: 5, hi: 9 };
    let err = scene.mesh("anchor", site).unwrap_err();
    assert_eq!((err.code, err.span), (Code::E602, site));
}

#[test]
fn mesh_constructor_uses_asset_and_optional_lod() {
    let mut src = Sources::default();
    src.add("lib/mesh.loft", include_str!("../../../lib/mesh.loft"));
    src.add("scene.loft", "build { emit Mesh(pose: place([1m,2m,3m]@World), asset: 9) }");
    let (modules, _) = front(&src).unwrap_or_else(|d| panic!("{}", d.render(&src)));
    let p = Prog::index(&modules).unwrap();
    let fields = &p.kinds["Mesh"].row;
    assert_eq!(fields.iter().map(|f| f.name.name.as_str()).collect::<Vec<_>>(), ["pose", "asset", "lod"]);
    let default = fields[2].default.as_ref().unwrap();
    assert!(matches!(default.kind, loft_parse::ast::ExprKind::Num { value: 0., .. }));
}

#[test]
fn imported_instances_share_one_mesh_module() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut src = Sources::default();
    let mut libs: Vec<_> = std::fs::read_dir(root.join("lib"))
        .unwrap()
        .map(|p| p.unwrap().path())
        .filter(|p| p.extension().is_some_and(|s| s == "loft"))
        .collect();
    libs.sort();
    for path in libs {
        src.add(path.display().to_string(), std::fs::read_to_string(&path).unwrap());
    }
    src.add(
        root.join("crates/loft-passes/tests/phase5/anchor.loft").display().to_string(),
        include_str!("phase5/anchor.loft"),
    );
    let (program, _, twin) = loft_passes::compile(&src, &[]).unwrap_or_else(|d| panic!("{}", d.render(&src)));
    let meshes: Vec<_> = program.kinds.iter().filter(|k| k.name == "Mesh").collect();
    assert_eq!(meshes.len(), 1);
    assert_eq!(meshes[0].n_rows, 20);
    for (i, row) in meshes[0].rows.chunks_exact(meshes[0].stride as usize).enumerate() {
        assert_eq!(&row[9..12], &[2.0 * i as f32, 0., 0.]);
        assert_eq!(row[12].to_bits(), meshes[0].rows[12].to_bits());
        assert_eq!(row[13].to_bits(), 0);
        let k = program.kinds.iter().position(|k| k.name == "Mesh").unwrap();
        let hit = twin.cpu_query(k, i, [2. * i as f32 + 0.25, 0.25, 1.], [0., 0., -1.]).unwrap();
        assert!((hit.visible - 1.).abs() <= 1e-5);
    }
}
