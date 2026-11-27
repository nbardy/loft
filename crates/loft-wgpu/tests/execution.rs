use loft_ir::{
    execution::{BlendMode, BufferView, CullMode, DESCRIPTOR, Execution, ImageStorage, RasterState, SamplerSpec},
    *,
};
use loft_wgpu::{error::RunnerError, schedule, storage};

fn descriptor(words: &[u32]) -> TableDecl {
    TableDecl {
        id: DESCRIPTOR,
        format: ElemFormat::U8,
        lifetime: TableLifetime::Static,
        stride_bytes: 1,
        capacity: words.len() as u32 * 4,
        initial: words.iter().flat_map(|word| word.to_le_bytes()).collect(),
    }
}

#[test]
fn legacy_descriptors_and_v3_round_trip() {
    assert!(Execution::default().table().is_none());

    let v1 = descriptor(&[
        1, 1, 0, 1, 7, 0, 1, 3, 64, 32, 1, 2, 1, 4,
    ]);
    let decoded = Execution::read(&v1).unwrap();
    assert_eq!(decoded.initializers[0].passes, [PassId(7)]);
    assert!(matches!(decoded.initializers[0].image.as_ref().unwrap().format, ImageStorage::LegacyReflect));
    assert_eq!(decoded.additive, [PassId(4)]);
    assert!(decoded.ranges.is_empty());
    let migrated = Execution::read(&decoded.table().unwrap()).unwrap();
    assert_eq!(migrated.additive, decoded.additive);
    assert!(matches!(migrated.initializers[0].image.as_ref().unwrap().format, ImageStorage::LegacyReflect));

    let v2 = descriptor(&[
        2, 1, 0, 1, 7, 0, 1, 3, 64, 32, 1, 2, 0, 1, 4, 1, 5, 2, 3, 256, 0, 12, 0,
    ]);
    assert_eq!(Execution::read(&v2).unwrap().table().unwrap().initial, v2.initial);
    let mut execution = Execution::read(&v2).unwrap();
    execution.raster.push(RasterState { pass: PassId(4), cull: CullMode::Back, blend: BlendMode::Alpha });
    execution.samplers.push(SamplerSpec { table: TableId(2), nearest: true, repeat: true });
    let bundle = Bundle { tables: vec![execution.table().unwrap()], ..Bundle::default() };
    let encoded = loft_format::bundle::encode(&bundle);
    assert_eq!(&encoded[..8], b"LOFTB001");
    let bundle = loft_format::bundle::decode(&encoded).unwrap();
    assert_eq!(&bundle.tables[0].initial[..4], &3_u32.to_le_bytes());
    let decoded = Execution::read(&bundle.tables[0]).unwrap();
    assert_eq!(decoded.raster, execution.raster);
    assert_eq!(decoded.samplers, execution.samplers);
    assert!(matches!(decoded.ranges.as_slice(), [BufferView { pass: PassId(5), group: 2, slot: 3, offset: 256, size: 12 }]));
    assert_eq!(decoded.table().unwrap().initial, bundle.tables[0].initial);
}

#[test]
fn link_uses_per_pass_state_and_checks_optional_vertex_tables() {
    let raster = RasterState { pass: PassId(0), cull: CullMode::Back, blend: BlendMode::Alpha };
    let sampler = SamplerSpec { table: TableId(2), nearest: true, repeat: true };
    let exec = Execution { raster: vec![raster], samplers: vec![sampler], ..Execution::default() };
    let mut bundle = Bundle {
        modules: vec![ModuleBlob {
            id: ModuleId(0), hash: [0; 32], target: ShaderTarget::Wgsl,
            bytes: br"
                @vertex fn vs() -> @builtin(position) vec4f { return vec4f(0.0); }
                @fragment fn fs() -> @location(0) vec4f { return vec4f(1.0); }
            ".to_vec().into_boxed_slice(),
        }],
        tables: (0..4).map(|id| TableDecl {
            id: TableId(id), format: ElemFormat::U32,
            lifetime: if id < 2 { TableLifetime::Static } else { TableLifetime::External },
            stride_bytes: 4, capacity: 0, initial: Box::default(),
        }).chain(exec.table()).collect(),
        passes: vec![PassDecl {
            id: PassId(0), wave: 0, queue: QueueClass::Graphics, bindings: Box::default(),
            run: PassRun::RenderBundle {
                draws: [IndirectDraw { module: ModuleId(0), vertex_entry: "vs".into(), fragment_entry: "fs".into(),
                    args: TableId(0), offset_bytes: 0, vertex: Some(TableId(1)), index: None }].into(),
                color_targets: [TargetDesc { table: TableId(3) }].into(), depth: None, msaa: 1,
            },
        }],
        ..Bundle::default()
    };
    bundle.resources.comparison_samplers.push(sampler.table);
    let link = |bundle: &Bundle| schedule::Schedule::new(bundle, wgpu::Features::empty());
    let schedule = link(&bundle).unwrap();
    let schedule::Pass::Draw(draw, _) = &schedule.passes[&raster.pass] else { panic!("expected raster") };
    assert_eq!(draw.draws[0].state, Some(raster));
    let allocations = storage::allocations(&bundle, &wgpu::Limits::default()).unwrap();
    assert!(matches!(allocations[2].2, storage::Allocation::Sampler(Some(spec)) if spec == sampler));
    assert_eq!(allocations.len(), 4);

    bundle.tables[1].stride_bytes = 0;
    assert!(matches!(link(&bundle), Err(RunnerError::AbiMismatch)));
    bundle.tables.remove(1);
    assert!(matches!(link(&bundle), Err(RunnerError::UnknownResource)));
    let PassRun::RenderBundle { draws, .. } = &mut bundle.passes[0].run else { unreachable!() };
    draws[0].vertex = None;
    assert!(link(&bundle).is_ok());
}
