use loft_ir::*;
use loft_wgpu::{bindings, error::*};
use std::collections::BTreeMap;
use wgpu::ShaderStages;

fn check_targets(passes: &[PassDecl]) -> Result<()> {
    let mut ids = std::collections::BTreeSet::new();
    for pass in passes {
        if let PassRun::RenderBundle {
            color_targets, depth, ..
        } = &pass.run
        {
            ids.extend(color_targets.iter().map(|t| t.table));
            ids.extend(depth.iter().map(|t| t.table));
        }
    }
    let bundle = Bundle {
        tables: ids
            .into_iter()
            .map(|id| TableDecl {
                id,
                format: ElemFormat::U32,
                lifetime: TableLifetime::External,
                capacity: 0,
                stride_bytes: 4,
                initial: Box::default(),
            })
            .collect(),
        passes: passes.to_vec(),
        ..Bundle::default()
    };
    loft_std::bundle::Resources::new(&bundle)?;
    Ok(())
}

fn pass(id: u32, run: PassRun) -> PassDecl {
    PassDecl {
        id: PassId(id),
        wave: 0,
        queue: QueueClass::Graphics,
        bindings: Box::default(),
        run,
    }
}

fn render(color: u32, depth: u32) -> PassRun {
    PassRun::RenderBundle {
        draws: Box::default(),
        color_targets: [TargetDesc { table: TableId(color) }].into(),
        depth: Some(DepthDesc {
            table: TableId(depth),
            write: true,
        }),
        msaa: 1,
    }
}

#[test]
fn distinct_logical_targets_cannot_alias_the_host_views() {
    let passes = [pass(0, render(1, 2)), pass(1, render(1, 2))];
    check_targets(&passes).unwrap();
    for next in [render(3, 2), render(1, 3)] {
        assert!(matches!(
            check_targets(&[passes[0].clone(), pass(1, next)]),
            Err(RunnerError::Unsupported(_))
        ));
    }
    assert_eq!(check_targets(&[pass(0, render(1, 1))]), Err(RunnerError::AbiMismatch));
}

#[test]
fn render_shader_validates_and_reflects_storage_in_both_stages() {
    use bindings::merge_bindings;
    let (shader, info) = loft_format::wgsl::validate(
        r#"
        @group(1) @binding(5) var<storage, read> data: array<vec4f>;
        @vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
            return data[i];
        }
        @fragment fn fs() -> @location(0) vec4f {
            return data[3];
        }
    "#,
        &mut naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty()),
    )
    .unwrap();
    let mut decl = pass(1, render(2, 3));
    decl.bindings = [PassBinding {
        group: 1,
        slot: 5,
        resource: TableId(1),
        access: BindAccess::ReadStorage,
    }]
    .into();
    let mut reflected = BTreeMap::new();
    for (i, stage) in [ShaderStages::VERTEX, ShaderStages::FRAGMENT].into_iter().enumerate() {
        let entries = bindings::reflect(&shader, info.get_entry_point(i), &decl).unwrap();
        merge_bindings(&mut reflected, entries, stage).unwrap();
    }
    assert_eq!(reflected[&(1, 5)].visibility, ShaderStages::VERTEX_FRAGMENT);
}
