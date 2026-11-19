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
    assert_eq!(
        r.encode_into(id, &mut encoder, &view, None, 0., 0.),
        Err(RunnerError::UnknownResource)
    );
}

#[test]
fn host_edits_parameters_and_external_handles_at_frame_boundary() {
    let (mut r, view) = device();
    let mut b = bundle();
    let src = "@group(0) @binding(2) var<storage,read> params:array<vec4f>;
@group(1) @binding(0) var<storage,read> external:array<u32>;
@group(2) @binding(5) var<storage,read_write> rows:array<u32>;
@group(3) @binding(0) var image:texture_2d<u32>;
@compute @workgroup_size(1) fn update() {
    rows[0] = external[0] + textureLoad(image,vec2i(0),0).x
        + u32(params[0].x*8.0) + u32(params[1].y*16.0);
}";
    b.modules[0].bytes = src.as_bytes().into();
    b.modules[0].hash = *blake3::hash(src.as_bytes()).as_bytes();
    for (id, group, access) in [(20,1,BindAccess::ReadStorage),(21,3,BindAccess::SampledTex)] {
        b.tables.push(TableDecl { id: TableId(id), lifetime: TableLifetime::External,
            format: ElemFormat::U32, stride_bytes: 4, capacity: 1, initial: Box::default() });
        let mut bindings = b.passes[0].bindings.to_vec();
        bindings.push(PassBinding {group, slot:0, resource:TableId(id), access});
        b.passes[0].bindings = bindings.into();
    }
    for (id,name,ty) in [(0,"roughness",IrTy::F32),(1,"sun",IrTy::Vec3)] {
        b.params.push(ParamMeta {id:ParamId(id),name:name.into(),ty,offset_bytes:id*16,
            default:[0.;4],range:Some(([0.;4],[1.;4]))});
    }
    let id = r.load_bundle(&b).unwrap();
    let pipe = r.instances[0].compute.pipeline(PassId(1)).unwrap().clone();
    let texture = Arc::new(r.device.create_texture(&wgpu::TextureDescriptor {
        label:None, size:wgpu::Extent3d {width:1,height:1,depth_or_array_layers:1},
        mip_level_count:1, sample_count:1, dimension:wgpu::TextureDimension::D2,
        format:wgpu::TextureFormat::R32Uint, usage:wgpu::TextureUsages::TEXTURE_BINDING, view_formats:&[],
    }));
    r.bind_external_texture(id, TableId(21), texture.clone()).unwrap();
    assert!(Arc::ptr_eq(&texture, &r.export_texture(id,TableId(21)).unwrap()));
    let host = allocate(&r.device,4);
    r.bind_external_buffer(id,TableId(20),host.clone()).unwrap();
    assert!(Arc::ptr_eq(&host,&r.export_buffer(id,TableId(20)).unwrap()));
    for (i,v) in [0.,0.25,0.5,0.75,1.].into_iter().enumerate() {
        r.set_param(id, ParamId(0), &[v]).unwrap();
        r.set_param(id, ParamId(1), &[0.,v,0.]).unwrap();
        r.queue.write_buffer(&host,0,&(100+i as u32).to_le_bytes());
        assert_eq!(step(&mut r,id,&view)[0],100+i as u32+(v*24.) as u32);
    }
    let before = r.instances[0].compute.bound_groups(PassId(1));
    let other = allocate(&r.device,4);
    r.queue.write_buffer(&other,0,&230u32.to_le_bytes());
    r.bind_external_buffer(id,TableId(20),other).unwrap();
    assert_eq!(step(&mut r,id,&view)[0],254);
    let after = r.instances[0].compute.bound_groups(PassId(1));
    assert_ne!(before[0],after[0]);
    assert_eq!(&before[1..],&after[1..]);
    assert_eq!(r.instances[0].compute.pipeline(PassId(1)).unwrap(), &pipe);
}
