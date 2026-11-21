use super::*;
use loft_wgpu::edits::Pipelines;
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
pub(super) fn read(r: &Runner, buf: &wgpu::Buffer) -> Vec<u32> {
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
    device_with_features(wgpu::Features::empty())
}

pub(super) fn device_with_features(features: wgpu::Features) -> (Runner, wgpu::TextureView) {
    let api = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(api.request_adapter(&Default::default())).unwrap();
    let (d, q) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: features,
        required_limits: wgpu::Limits {
            max_binding_array_elements_per_shader_stage: if features.contains(wgpu::Features::TEXTURE_BINDING_ARRAY) {
                adapter.limits().max_binding_array_elements_per_shader_stage.min(16)
            } else { 0 },
            ..Default::default()
        },
        ..Default::default()
    }))
    .unwrap();
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
@group(1) @binding(0) var<storage,read> host_words:array<u32>;
@group(2) @binding(5) var<storage,read_write> rows:array<u32>;
@group(3) @binding(0) var image:texture_2d<u32>;
@compute @workgroup_size(1) fn update() {
    rows[0] = host_words[0] + textureLoad(image,vec2i(0),0).x
        + u32(params[0].x*8.0) + u32(params[1].y*16.0);
}
@compute @workgroup_size(1) fn second() { rows[1] = u32(params[1].y*16.0); }";
    b.modules[0].bytes = src.as_bytes().into();
    b.modules[0].hash = *blake3::hash(src.as_bytes()).as_bytes();
    let mut independent = b.passes[0].clone();
    independent.id = PassId(2);
    independent.run = PassRun::Compute {
        module: MODULE,
        entry: "second".into(),
        dispatch: DispatchExtent::Fixed([1, 1, 1]),
    };
    b.passes.push(independent);
    for (id, group, access) in [
        (20, 1, BindAccess::ReadStorage),
        (21, 3, BindAccess::SampledTex),
    ] {
        b.tables.push(TableDecl {
            id: TableId(id),
            lifetime: TableLifetime::External,
            format: ElemFormat::U32,
            stride_bytes: 4,
            capacity: 1,
            initial: Box::default(),
        });
        let mut bindings = b.passes[0].bindings.to_vec();
        bindings.push(PassBinding {
            group,
            slot: 0,
            resource: TableId(id),
            access,
        });
        b.passes[0].bindings = bindings.into();
    }
    for (id, name, ty) in [(0, "roughness", IrTy::F32), (1, "sun", IrTy::Vec3)] {
        b.params.push(ParamMeta {
            id: ParamId(id),
            name: name.into(),
            ty,
            offset_bytes: id * 16,
            default: [0.; 4],
            range: Some(([0.; 4], [1.; 4])),
        });
    }
    let id = r.load_bundle(&b).unwrap();
    let pipe = r.instances[0].compute.pipeline(PassId(1)).unwrap().clone();
    let desc = wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R32Uint,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    };
    r.declare_external_texture(id, TableId(21), &desc).unwrap();
    let wrong = Arc::new(r.device.create_texture(&wgpu::TextureDescriptor {
        size: wgpu::Extent3d { width: 2, ..desc.size },
        ..desc.clone()
    }));
    assert_eq!(r.bind_external_texture(id, TableId(21), wrong), Err(RunnerError::AbiMismatch));
    assert!(r.export_texture(id, TableId(21)).is_none());
    let texture = Arc::new(r.device.create_texture(&desc));
    r.bind_external_texture(id, TableId(21), texture.clone())
        .unwrap();
    assert!(Arc::ptr_eq(
        &texture,
        &r.export_texture(id, TableId(21)).unwrap()
    ));
    let host = allocate(&r.device, 4, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC);
    r.bind_external_buffer(id, TableId(20), host.clone())
        .unwrap();
    assert!(Arc::ptr_eq(
        &host,
        &r.export_buffer(id, TableId(20)).unwrap()
    ));
    for (i, v) in [0., 0.25, 0.5, 0.75, 1.].into_iter().enumerate() {
        r.set_param(id, ParamId(0), &[v]).unwrap();
        r.set_param(id, ParamId(1), &[0., v, 0.]).unwrap();
        r.queue
            .write_buffer(&host, 0, &(100 + i as u32).to_le_bytes());
        assert_eq!(
            step(&mut r, id, &view)[0],
            100 + i as u32 + (v * 24.) as u32
        );
    }
    let before = r.instances[0].compute.bound_groups(PassId(1));
    let unrelated = r.instances[0].compute.bound_groups(PassId(2));
    let other = allocate(&r.device, 4, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC);
    r.queue.write_buffer(&other, 0, &230u32.to_le_bytes());
    r.bind_external_buffer(id, TableId(20), other).unwrap();
    assert_eq!(step(&mut r, id, &view)[0], 254);
    let after = r.instances[0].compute.bound_groups(PassId(1));
    assert_ne!(before[0], after[0]);
    assert_eq!(&before[1..], &after[1..]);
    assert_eq!(unrelated, r.instances[0].compute.bound_groups(PassId(2)));
    let new_tex = Arc::new(r.device.create_texture(&desc));
    r.queue.write_texture(
        new_tex.as_image_copy(),
        &11u32.to_le_bytes(),
        Default::default(),
        desc.size,
    );
    r.bind_external_texture(id, TableId(21), new_tex).unwrap();
    assert_eq!(step(&mut r, id, &view), [265, 16]);
    let rebound = r.instances[0].compute.bound_groups(PassId(1));
    assert_eq!(&after[..2], &rebound[..2]);
    assert_ne!(after[2], rebound[2]);
    assert_eq!(unrelated, r.instances[0].compute.bound_groups(PassId(2)));
    assert_eq!(r.instances[0].compute.pipeline(PassId(1)).unwrap(), &pipe);
}

#[test]
fn material_texture_ids_cross_pages_and_mip_edges() {
    const FEATURES: wgpu::Features = wgpu::Features::TEXTURE_BINDING_ARRAY.union(
        wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING);
    const HEAD: &str = r"
struct Material { indices: vec4u, base: vec4f, coat: vec4f, grain: vec4f }
@group(1) @binding(0) var<storage, read> materials: array<Material>;
@group(2) @binding(5) var<storage, read_write> rows: array<u32>;
";
    const PORTABLE: &str = r"
@group(3) @binding(0) var page0: texture_2d_array<u32>;
@group(3) @binding(1) var page1: texture_2d_array<u32>;
fn texel(id: u32, xy: vec2i, lod: i32) -> u32 {
    if id < 4u { return textureLoad(page0, xy, i32(id), lod).x; }
    return textureLoad(page1, xy, i32(id - 4u), lod).x;
}";
    const NATIVE: &str = r"
@group(3) @binding(0) var pages: binding_array<texture_2d<u32>, 8>;
fn texel(id: u32, xy: vec2i, lod: i32) -> u32 { return textureLoad(pages[id], xy, lod).x; }
";
    const ENTRY: &str = r"
@compute @workgroup_size(16) fn update(@builtin(global_invocation_id) invocation: vec3u) {
    let i = invocation.x;
    let id = materials[i / 2u].indices.x;
    if id >= 8u {
        rows[i] = 0xffffffffu;
    } else {
        let lod = i32(i % 2u);
        let last = vec2i(3i >> u32(lod));
        rows[i] = texel(id, last, lod);
    }
}";
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let native = adapter.features().contains(FEATURES)
        && adapter.limits().max_binding_array_elements_per_shader_stage >= 8;
    let mut observed = Vec::new();
    for bindless in [false, true].into_iter().filter(|&bindless| !bindless || native) {
        let (mut r, view) = device_with_features(if bindless { FEATURES } else { wgpu::Features::empty() });
        let layout = |layers| wgpu::TextureDescriptor {
            label: None, size: wgpu::Extent3d { width: 4, height: 4, depth_or_array_layers: layers },
            mip_level_count: 2, sample_count: 1, dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST, view_formats: &[],
        };
        let upload = |first, layers| {
            let desc = layout(layers);
            let tex = Arc::new(r.device.create_texture(&desc));
            for level in 0..2 {
                let side = 4 >> level;
                let values = (first..first + layers).flat_map(|id| {
                    (0..side * side).flat_map(move |xy| u32::to_le_bytes(100 * id + 20 * level + xy))
                }).collect::<Vec<_>>();
                r.queue.write_texture(
                    wgpu::TexelCopyTextureInfo { mip_level: level, ..tex.as_image_copy() },
                    &values, wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(side * 4), rows_per_image: Some(side) },
                    wgpu::Extent3d { width: side, height: side, depth_or_array_layers: layers },
                );
            }
            tex
        };
        let textures = if bindless { (0..8).map(|i| upload(i, 1)).collect() } else { vec![upload(0, 4), upload(4, 4)] };
        let tex0 = TableId(22);
        let tex1 = TableId(23);
        let data = TableId(24);
        let mut b = bundle();
        b.state = None;
        b.tables[0].capacity = 16;
        // This probe needs sixteen output words rather than the two from the shared fixture.
        b.tables[0].lifetime = TableLifetime::DynamicArena { max_rows: 16 };
        b.tables.push(TableDecl {
            id: data, format: ElemFormat::U32, stride_bytes: 64, capacity: 8,
            lifetime: TableLifetime::DynamicArena { max_rows: 8 }, initial: Box::new([]),
        });
        b.rows.insert(KindId(10), data);
        let source = if bindless {
            ["enable wgpu_binding_array;\n", HEAD, NATIVE, ENTRY].concat()
        } else {
            [HEAD, PORTABLE, ENTRY].concat()
        };
        b.modules[0].bytes = source.into_bytes().into_boxed_slice();
        b.modules[0].hash = *blake3::hash(&b.modules[0].bytes).as_bytes();
        let mut bindings = b.passes[0].bindings.to_vec();
        bindings.push(PassBinding { group: 1, slot: 0, resource: data, access: BindAccess::ReadStorage });
        for (slot, resource) in [tex0, tex1].into_iter().take(if bindless { 1 } else { 2 }).enumerate() {
            b.tables.push(TableDecl { id: resource, lifetime: TableLifetime::External, format: ElemFormat::U32,
                stride_bytes: 4, capacity: 0, initial: Box::new([]) });
            bindings.push(PassBinding { group: 3, slot: slot as u32, resource,
                access: if bindless { BindAccess::BindlessTexArray } else { BindAccess::SampledTex } });
        }
        b.passes[0].bindings = bindings.into_boxed_slice();
        let PassRun::Compute { dispatch, .. } = &mut b.passes[0].run else { unreachable!() };
        *dispatch = DispatchExtent::Fixed([1, 1, 1]);
        let id = r.load_bundle_bytes(&crate::bundle::encode(&b)).unwrap();
        if bindless {
            r.bind_external_textures(id, tex0, &textures).unwrap();
            assert_eq!(r.bind_external_textures(id, tex0, &textures[..7]), Err(RunnerError::AbiMismatch));
        } else {
            for (table, texture) in [tex0, tex1].into_iter().zip(textures) {
                r.declare_external_texture(id, table, &layout(4)).unwrap();
                r.bind_external_texture(id, table, texture).unwrap();
            }
        }
        let mut samples = Vec::new();
        for ids in [[0, 1, 2, 3, 4, 5, 6, 7], [7, 3, 6, 0, 4, 2, 5, 8]] {
            let records = ids.into_iter().flat_map(|id: u32| {
                let mut row = [0; 64];
                row[..4].copy_from_slice(&id.to_le_bytes());
                row
            }).collect::<Vec<_>>();
            r.upsert_rows(id, KindId(10), 0, &records).unwrap();
            let values = step(&mut r, id, &view);
            let expected = ids.into_iter().flat_map(|id| {
                if id == 8 { [u32::MAX; 2] } else { [100 * id + 15, 100 * id + 23] }
            }).collect::<Vec<_>>();
            assert_eq!(values, expected);
            samples.extend(values);
        }
        observed.push(samples);
    }
    if native {
        assert_eq!(observed[0], observed[1]);
    }
    println!("two portable texture pages, sorted and shuffled IDs, mip edges, bindless supported: {native}");
}
