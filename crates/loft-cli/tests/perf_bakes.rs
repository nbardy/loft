use loft_passes::{source::Graph, bake_tex::Clock};
use std::path::Path;

#[test]
fn hermit_shade_samples_cube_and_rebakes_on_sky_edit() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../stdlib/engines/hermit.loft");
    let graph = Graph::file(&path, "").unwrap();
    let (program, _, _) = loft_passes::compile(&graph, &[("count".into(), 1.)])
        .unwrap_or_else(|e| panic!("{}", e.render(&graph)));
    let [bake] = &program.bakes[..] else { panic!("expected one Hermit sky bake"); };
    let Clock::Dirty(words) = &bake.clock else { panic!("sky must have a dirty clock"); };
    assert_eq!(words, &[4, 5, 6, 7, 8, 9, 10]);
    for k in &program.kinds {
        let shade = &program.modules[k.module];
        assert!(shade.wgsl.contains("texture_cube<f32>"));
        assert!(shade.wgsl.contains("textureSampleLevel(loft_bake_texture_HermitSkyPatches"));
    }
    let g = loft_cli::native::gpu().unwrap();
    let loaded = loft_cli::native::Loaded::new(&g, &program, (128, 80), None).unwrap();
    let first = loaded.frame(&g, 0.);
    let clean = loaded.frame(&g, 0.);
    assert!(first > clean);
    assert_eq!(loaded.frame(&g, 1.), clean, "camera motion must leave the bake clean");
    // Both live edits may invalidate the shadow cache. Only a sky edit
    // should add a bake dispatch to that common cost.
    loaded.set_param_word(0, 1.9).unwrap();
    let unrelated = loaded.frame(&g, 1.);
    assert_eq!(loaded.frame(&g, 1.), clean);
    loaded.set_param_word(4, 0.9).unwrap();
    assert_eq!(loaded.frame(&g, 1.), unrelated + 1);
    assert_eq!(loaded.baked_texture(&bake.name).unwrap().depth_or_array_layers(), 6);
    assert_eq!(loaded.frame(&g, 1.), clean);
}
