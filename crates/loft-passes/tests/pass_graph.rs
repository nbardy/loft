use loft_ir::{DispatchExtent, ElemFormat, PassId, PassRun, execution::{DESCRIPTOR, Execution}};
use loft_passes::source::Graph;
use std::{collections::BTreeSet, path::Path};

fn checked(text: &str) -> (loft_passes::program::Program, loft_passes::Compiled) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = Graph::text(&root, Path::new("scenes/pass_graph_test.loft"), text);
    let (program, _, compiled) = loft_passes::compile(&src, &[]).unwrap_or_else(|d| panic!("{}", d.render(&src)));
    (program, compiled)
}

fn compile(text: &str) -> (loft_passes::program::Program, loft_ir::Bundle) {
    let (program, compiled) = checked(text);
    let bundle = loft_passes::bundle::lower(&program, &compiled, (64, 64), |_, _| vec![0; 256]).unwrap();
    (program, bundle)
}

#[test]
fn compute_passes_keep_their_position_among_raster_passes() {
    let (program, mut compiled) = checked(r"
        use stdlib::integrators::direct::{sun_sky}
        use stdlib::sky::environment::{sky_color}
        space World
        struct Word { value: i }
        kernel stamp(dst: [Word]) allow(atomics) { atomic_add(dst[0].value, 1) }

        kind Ball @World () {
            trace(p) = sphere(p - [0, 0, 0]@World, 0.5)
            material(p, n) = ggx(rgb(0.4), 0.8, 0)
        }
        build { emit Ball() }
        schedule Main {
            table(Output, row: Word, capacity: 2)
            table(DispatchArgs, row: Word, capacity: 3)
            orbit(at: [0, 0, 0], radius: 4, fov: 40deg)
            cull_indirect(Ball)
            pass stamp_before writes [Output] {
                compute(entry: stamp, extent: 1, dst: Output)
            }
            render_bundle(Vis64) { draw_indirect(Ball) }
            pass stamp_after writes [Output] {
                compute(entry: stamp, extent: 2, dst: Output)
            }
            tile_classify(Vis64, tile: 8)
            shade_tiles(Vis64, light: sun_sky, sun: [0.5, 0.75, 0.35])
            tonemap_aces(ColorHdr, sky: sky_color)
        }
    ");
    let lower = |compiled: &loft_passes::Compiled| {
        loft_passes::bundle::lower(&program, compiled, (64, 64), |_, _| vec![0; 256]).unwrap()
    };
    let bundle = lower(&compiled);
    assert_eq!(bundle.modules.iter().map(|m| m.id).collect::<BTreeSet<_>>().len(), bundle.modules.len());
    assert_eq!(bundle.tables.iter().map(|t| t.id).collect::<BTreeSet<_>>().len(), bundle.tables.len());
    assert_eq!(bundle.passes.iter().map(|p| p.id).collect::<BTreeSet<_>>().len(), bundle.passes.len());
    assert!(matches!(bundle.passes[2].run, PassRun::RenderBundle { .. }));
    assert!(bundle.passes.windows(2).all(|pair| pair[0].wave < pair[1].wave));
    let execution = Execution::read(bundle.tables.iter().find(|t| t.id == DESCRIPTOR).unwrap()).unwrap();
    for (slot, groups) in [(1, 1), (3, 2)] {
        let pass = &bundle.passes[slot];
        let PassRun::Compute { module, entry, dispatch: loft_ir::DispatchExtent::Fixed(extent) } = &pass.run
            else { panic!("expected authored compute") };
        assert_eq!(extent, &[groups, 1, 1]);
        let module = &bundle.modules[module.0 as usize];
        let shader = naga::front::wgsl::parse_str(std::str::from_utf8(&module.bytes).unwrap()).unwrap();
        assert!(shader.entry_points.iter().any(|point| point.name == *entry && point.stage == naga::ShaderStage::Compute));
        let [binding] = pass.bindings.as_ref() else { panic!("expected one binding") };
        let table = &bundle.tables[binding.resource.0 as usize];
        assert_eq!(table.id, bundle.passes[1].bindings[0].resource);
        let ElemFormat::Struct(layout) = table.format else { panic!("expected a struct table") };
        assert_eq!(bundle.struct_layouts[layout as usize].stride_bytes, 4);
        assert_eq!(table.capacity, 2);
        assert!(execution.ranges.iter().any(|range| range.pass == pass.id && range.size == 8));
    }

    // Extents and resets can refer to a table the shader never binds.
    let args = compiled.compute.tables["DispatchArgs"];
    let source = &mut compiled.compute.bundle;
    let descriptor = Execution { additive: vec![PassId(0)], ..Default::default() }.table().unwrap();
    source.tables.insert(0, descriptor);
    source.resources.resets.push(args);
    for (pass, extent) in source.passes.iter_mut().zip([
        DispatchExtent::FromRowCount { table: args, workgroup_x: 32 },
        DispatchExtent::Indirect { buf: args, offset_bytes: 0 },
    ]) {
        let PassRun::Compute { dispatch, .. } = &mut pass.run else { panic!("expected authored compute") };
        *dispatch = extent;
    }

    let remapped = lower(&compiled);
    let args = remapped.tables.iter().find(|t| t.capacity == 3 && matches!(t.format, ElemFormat::Struct(_))).unwrap().id;
    assert!(matches!(remapped.passes[1].run,
        PassRun::Compute { dispatch: DispatchExtent::FromRowCount { table, workgroup_x: 32 }, .. } if table == args));
    assert!(matches!(remapped.passes[3].run,
        PassRun::Compute { dispatch: DispatchExtent::Indirect { buf, offset_bytes: 0 }, .. } if buf == args));
    assert_eq!(remapped.tables.len(), bundle.tables.len());
    assert_eq!(remapped.tables.iter().filter(|t| t.id == DESCRIPTOR).count(), 1);
    assert!(remapped.resources.resets.contains(&args));
    assert!(bundle.resources.resets.iter().all(|id| remapped.resources.resets.contains(id)));
}

#[test]
fn screen_pass_needs_only_a_camera_and_callback_in_the_schedule() {
    let (program, bundle) = compile(r"
        use stdlib::integrators::restir::{reservoir_add, reservoir_scale, reservoir_reuse}
        use stdlib::lighting::world
        space World
        fn sample_pixel(origin: v3, direction: v3, seed: u, bounces: u) -> rgb = rgb(0.5)
        schedule Main {
            orbit(at: [0, 0, 0], radius: 4, fov: 40deg)
            screen_pass(sample_pixel, bounces: 0)
        }
    ");
    assert!(matches!(program.passes.as_slice(), [loft_passes::program::Pass::Path(_)]));
    assert!(matches!(bundle.passes.as_slice(), [loft_ir::PassDecl { run: PassRun::RenderBundle { .. }, .. }]));
}

#[test]
fn standalone_particle_schedules_export_without_raster_passes() {
    for scene in [
        include_str!("../../../scenes/particle_flip.loft"),
        include_str!("../../../scenes/particle_scatter.loft"),
    ] {
        let (program, bundle) = compile(scene);
        assert!(program.passes.is_empty());
        assert!(!bundle.passes.is_empty());
        assert!(bundle.passes.iter().all(|pass| matches!(pass.run, PassRun::Compute { .. })));
    }
}
