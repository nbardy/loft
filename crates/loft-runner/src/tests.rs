use super::*;
use wgpu::BufferUsages as U;

const TABLE: TableId = TableId(7);
const KIND: KindId = KindId(9);
const MODULE: ModuleId = ModuleId(5);
const SOURCE: &str = "@group(2) @binding(5) var<storage,read_write> rows:array<u32>; @compute @workgroup_size(1) fn update(@builtin(global_invocation_id) i:vec3u){rows[i.x]+=1u;}";

fn bundle() -> Bundle {
    Bundle {
        tables: vec![TableDecl {
            id: TABLE,
            format: ElemFormat::U32,
            lifetime: TableLifetime::DynamicArena { max_rows: 8 },
            stride_bytes: 4,
            capacity: 2,
            initial: 1u32.to_le_bytes().into(),
        }],
        modules: vec![ModuleBlob {
            id: MODULE,
            target: ShaderTarget::Wgsl,
            hash: *blake3::hash(SOURCE.as_bytes()).as_bytes(),
            bytes: SOURCE.as_bytes().into(),
        }],
        rows: [(KIND, TABLE)].into(),
        passes: vec![PassDecl {
            id: PassId(1),
            wave: 0,
            queue: QueueClass::AsyncCompute,
            bindings: vec![PassBinding {
                group: 2,
                slot: 5,
                resource: TABLE,
                access: BindAccess::ReadWriteStorage,
            }]
            .into(),
            run: PassRun::Compute {
                module: MODULE,
                entry: "update".into(),
                dispatch: DispatchExtent::FromRowCount {
                    table: TABLE,
                    workgroup_x: 1,
                },
            },
        }],
        state: Some(StateStoreDecl {
            table: TABLE,
            layout_hash: [1; 32],
            checkpoint_slots: 2,
        }),
        ..Default::default()
    }
}
fn read(r: &Runner, buf: &wgpu::Buffer) -> Vec<u32> {
    let result = r.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: buf.size(),
        usage: U::COPY_DST | U::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = r.device.create_command_encoder(&Default::default());
    enc.copy_buffer_to_buffer(buf, 0, &result, 0, Some(buf.size()));
    r.queue.submit([enc.finish()]);
    result.map_async(wgpu::MapMode::Read, .., |status| status.unwrap());
    r.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    result
        .get_mapped_range(..)
        .unwrap()
        .chunks_exact(4)
        .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
        .collect()
}
fn step(r: &mut Runner, id: InstanceId, view: &wgpu::TextureView) -> Vec<u32> {
    let mut enc = r.device.create_command_encoder(&Default::default());
    r.encode_into(id, &mut enc, view, None, 0., 0.).unwrap();
    r.queue.submit([enc.finish()]);
    read(r, &r.export_buffer(id, TABLE).unwrap())
}
fn device() -> (Runner, wgpu::TextureView) {
    let api = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(api.request_adapter(&Default::default())).unwrap();
    let (d, q) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let view = d
        .create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default());
    let r = Runner::attach(
        Arc::new(d),
        Arc::new(q),
        HostCaps {
            depth_convention: DepthConvention::StandardZeroToOne,
            msaa_samples: 1,
            features: GpuFeatures::empty(),
        },
    );
    (r, view)
}

#[test]
fn arena_rebind_and_layout_keyed_swaps() {
    let (mut r, view) = device();
    let mut b = bundle();
    let id = r.load_bundle(&b).unwrap();
    assert_eq!(step(&mut r, id, &view), [2, 0]);
    assert_eq!(
        r.upsert_rows(id, KIND, 1, &[3]),
        Err(RunnerError::AbiMismatch)
    );
    assert_eq!(r.set_row_count(id, KIND, 3), Err(RunnerError::Capacity));
    r.upsert_rows(id, KIND, 1, &12u32.to_le_bytes()).unwrap();
    assert_eq!(r.resize_table(id, TABLE, 1), Err(RunnerError::Capacity));
    assert_eq!(r.resize_table(id, TABLE, 9), Err(RunnerError::Capacity));
    assert_eq!(r.instances[0].resource_epoch, 0);
    r.resize_table(id, TABLE, 3).unwrap();
    r.set_row_count(id, KIND, 3).unwrap();
    assert_eq!(r.instances[0].resource_epoch, 1);
    assert_eq!(r.instances[0].tables[&TABLE].epoch, 1);
    assert_eq!(step(&mut r, id, &view), [3, 13, 1]);
    let live = r.export_buffer(id, TABLE).unwrap();
    let rings = r.instances[0].checkpoints.clone();
    r.queue.write_buffer(&rings[0], 0, &123u32.to_le_bytes());
    assert_eq!(
        r.hot_swap_module(id, MODULE, &SOURCE.replace("@binding(5)", "@binding(6)")),
        Err(RunnerError::AbiMismatch)
    );
    r.hot_swap_module(id, MODULE, &SOURCE.replace("+=1u", "+=3u"))
        .unwrap();
    assert_eq!(step(&mut r, id, &view), [6, 16, 4]);
    assert_eq!(live, r.export_buffer(id, TABLE).unwrap());
    assert_eq!(rings, r.instances[0].checkpoints);
    r.hot_swap_bundle(id, &b).unwrap();
    assert_eq!(live, r.export_buffer(id, TABLE).unwrap());
    assert_eq!(rings, r.instances[0].checkpoints);
    assert_eq!(read(&r, &rings[0])[0], 123);
    assert_eq!(step(&mut r, id, &view), [7, 17, 5]);
    b.state.as_mut().unwrap().layout_hash = [2; 32];
    r.hot_swap_bundle(id, &b).unwrap();
    assert_ne!(live, r.export_buffer(id, TABLE).unwrap());
    assert_ne!(rings, r.instances[0].checkpoints);
    assert_eq!(read(&r, &r.instances[0].checkpoints[0]), [0, 0]);
    assert_eq!(step(&mut r, id, &view), [2, 0]);
}
#[test]
fn vertex_or_index_replacement_invalidates_recorded_draws() {
    let draws = [IndirectDraw {
        module: MODULE,
        vertex_entry: "vs".into(),
        fragment_entry: "fs".into(),
        args: TableId(0),
        offset_bytes: 0,
        vertex: Some(TableId(1)),
        index: Some(TableId(2)),
    }];
    for id in 0..3 {
        assert_eq!(
            render_bundle_stale(&draws, 4, |t| if t.0 == id { 5 } else { 4 }),
            id != 0
        );
    }
}

#[test]
fn shader_matches_frozen_math() {
    use loft_abi::det::*;
    let (mut r, view) = device();
    let mut b = bundle();
    b.tables[0].capacity = 7;
    let source = format!(
        "{}\n{}",
        loft_abi::DET_WGSL,
        r#"
        @group(2) @binding(5) var<storage,read_write> data:array<u32>;
        @compute @workgroup_size(1) fn update() {
            let x=bitcast<f32>(data[0]); let y=bitcast<f32>(data[1]);
            data[2]=bitcast<u32>(det_sin(x)); data[3]=bitcast<u32>(det_cos(x));
            data[4]=bitcast<u32>(det_atan2(y,x)); data[5]=bitcast<u32>(det_acos(x));
            data[6]=bitcast<u32>(det_exp(x));
        }"#
    );
    b.modules[0].bytes = source.as_bytes().into();
    b.modules[0].hash = *blake3::hash(source.as_bytes()).as_bytes();
    if let PassRun::Compute { dispatch, .. } = &mut b.passes[0].run {
        *dispatch = DispatchExtent::Fixed([1, 1, 1])
    }
    let id = r.load_bundle(&b).unwrap();
    for (x, y) in [
        (0.125_f32, -0.75_f32),
        (-0.9375, 0.0625),
        (0.75, 0.75),
        (0., 0.),
    ] {
        r.upsert_rows(
            id,
            KIND,
            0,
            [x.to_le_bytes(), y.to_le_bytes()].as_flattened(),
        )
        .unwrap();
        assert_eq!(
            step(&mut r, id, &view)[2..],
            [
                det_sin(x),
                det_cos(x),
                det_atan2(y, x),
                det_acos(x),
                det_exp(x)
            ]
            .map(f32::to_bits)
        );
    }
}
#[test]
fn reflected_texture_group_accepts_sparse_slots() {
    let (mut r, view) = device();
    let mut b = bundle();
    let src = format!(
        "@group(3) @binding(11) var img:texture_2d<u32>; {}",
        SOURCE.replace("+=1u", "+=textureLoad(img,vec2i(0),0).x")
    );
    b.modules[0].bytes = src.as_bytes().into();
    b.modules[0].hash = *blake3::hash(src.as_bytes()).as_bytes();
    b.tables.push(TableDecl {
        id: TableId(21),
        lifetime: TableLifetime::External,
        format: ElemFormat::U32,
        stride_bytes: 4,
        capacity: 1,
        initial: Box::default(),
    });
    let mut bindings = b.passes[0].bindings.to_vec();
    bindings.push(PassBinding {
        group: 3,
        slot: 11,
        resource: TableId(21),
        access: BindAccess::SampledTex,
    });
    b.passes[0].bindings = bindings.into();
    let id = r.load_bundle(&b).unwrap();
    let mut encoder = r.device.create_command_encoder(&Default::default());
    assert!(matches!(
        r.encode_into(id, &mut encoder, &view, None, 0., 0.),
        Err(RunnerError::Unsupported(_))
    ));
}
