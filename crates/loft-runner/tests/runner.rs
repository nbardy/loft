#![cfg(not(target_arch = "wasm32"))]

use loft_ir::*;
use loft_runner::{FrameIdentity, Runner, RunnerError, read_buffer, render_bundle_stale};
use std::sync::Arc;


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
fn submit(
    r: &mut Runner, device: &wgpu::Device, queue: &wgpu::Queue, id: InstanceId, view: &wgpu::TextureView,
) -> FrameIdentity {
    let mut enc = device.create_command_encoder(&Default::default());
    let frame = r.encode_into(id, &mut enc, view, None, 0., 0.).unwrap();
    queue.submit([enc.finish()]);
    frame
}

fn read(r: &Runner, device: &wgpu::Device, queue: &wgpu::Queue, id: InstanceId) -> Vec<u32> {
    read_buffer(device, queue, &r.export_buffer(id, TABLE).unwrap())
        .unwrap()
        .chunks_exact(4)
        .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
        .collect()
}

fn step(
    r: &mut Runner, device: &wgpu::Device, queue: &wgpu::Queue, id: InstanceId, view: &wgpu::TextureView,
) -> Vec<u32> {
    submit(r, device, queue, id, view);
    read(r, device, queue, id)
}

fn device() -> (Runner, Arc<wgpu::Device>, Arc<wgpu::Queue>, wgpu::TextureView) {
    let api = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(api.request_adapter(&Default::default())).unwrap();
    let (d, q) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::empty(),
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
    let (d, q) = (Arc::new(d), Arc::new(q));
    let r = Runner::attach(
        d.clone(),
        q.clone(),
        HostCaps {
            depth_convention: DepthConvention::StandardZeroToOne,
            msaa_samples: 1,
            features: GpuFeatures::empty(),
        },
    );
    (r, d, q, view)
}

#[test]
fn arena_rebind_and_layout_keyed_swaps() {
    let (mut r, d, q, view) = device();
    let mut b = bundle();
    let id = r.load_bundle(&b).unwrap();
    assert_eq!(submit(&mut r, &d, &q, id, &view).resource_epoch, 0);
    assert_eq!(read(&r, &d, &q, id), [2, 0]);
    assert_eq!(
        r.upsert_rows(id, KIND, 1, &[3]),
        Err(RunnerError::AbiMismatch)
    );
    assert_eq!(r.set_row_count(id, KIND, 3), Err(RunnerError::Capacity));
    r.upsert_rows(id, KIND, 1, &12u32.to_le_bytes()).unwrap();
    assert_eq!(r.resize_table(id, TABLE, 1), Err(RunnerError::Capacity));
    assert_eq!(r.resize_table(id, TABLE, 9), Err(RunnerError::Capacity));
    r.resize_table(id, TABLE, 3).unwrap();
    r.set_row_count(id, KIND, 3).unwrap();
    assert_eq!(submit(&mut r, &d, &q, id, &view).resource_epoch, 1);
    assert_eq!(read(&r, &d, &q, id), [3, 13, 1]);
    let live = r.export_buffer(id, TABLE).unwrap();
    assert_eq!(
        r.hot_swap_module(id, MODULE, &SOURCE.replace("@binding(5)", "@binding(6)")),
        Err(RunnerError::AbiMismatch)
    );
    r.hot_swap_module(id, MODULE, &SOURCE.replace("+=1u", "+=3u"))
        .unwrap();
    assert_eq!(step(&mut r, &d, &q, id, &view), [6, 16, 4]);
    assert_eq!(live, r.export_buffer(id, TABLE).unwrap());
    r.hot_swap_bundle(id, &b).unwrap();
    assert_eq!(live, r.export_buffer(id, TABLE).unwrap());
    assert_eq!(step(&mut r, &d, &q, id, &view), [7, 17, 5]);
    b.state.as_mut().unwrap().layout_hash = [2; 32];
    r.hot_swap_bundle(id, &b).unwrap();
    assert_ne!(live, r.export_buffer(id, TABLE).unwrap());
    assert_eq!(step(&mut r, &d, &q, id, &view), [2, 0]);
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
    let (mut r, d, q, view) = device();
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
            step(&mut r, &d, &q, id, &view)[2..],
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
    let (mut r, d, q, view) = device();
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
    let mut encoder = d.create_command_encoder(&Default::default());
    assert_eq!(
        r.encode_into(id, &mut encoder, &view, None, 0., 0.),
        Err(RunnerError::UnknownResource)
    );
}

#[test]
fn host_edits_parameters_and_external_handles_at_frame_boundary() {
    let (mut r, d, q, view) = device();
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
    let pipeline_count = r.pipeline_creates(id).unwrap().compute.len();
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
    let wrong = Arc::new(d.create_texture(&wgpu::TextureDescriptor {
        size: wgpu::Extent3d { width: 2, ..desc.size },
        ..desc.clone()
    }));
    assert_eq!(r.bind_external_texture(id, TableId(21), wrong), Err(RunnerError::AbiMismatch));
    assert!(r.export_texture(id, TableId(21)).is_none());
    let texture = Arc::new(d.create_texture(&desc));
    r.bind_external_texture(id, TableId(21), texture.clone())
        .unwrap();
    assert!(Arc::ptr_eq(
        &texture,
        &r.export_texture(id, TableId(21)).unwrap()
    ));
    let host = host_buffer(&d);
    r.bind_external_buffer(id, TableId(20), host.clone())
        .unwrap();
    assert!(Arc::ptr_eq(
        &host,
        &r.export_buffer(id, TableId(20)).unwrap()
    ));
    for (i, v) in [0., 0.25, 0.5, 0.75, 1.].into_iter().enumerate() {
        r.set_param(id, ParamId(0), &[v]).unwrap();
        r.set_param(id, ParamId(1), &[0., v, 0.]).unwrap();
        q
            .write_buffer(&host, 0, &(100 + i as u32).to_le_bytes());
        assert_eq!(
            step(&mut r, &d, &q, id, &view)[0],
            100 + i as u32 + (v * 24.) as u32
        );
    }
    let other = host_buffer(&d);
    q.write_buffer(&other, 0, &230u32.to_le_bytes());
    r.bind_external_buffer(id, TableId(20), other).unwrap();
    assert_eq!(step(&mut r, &d, &q, id, &view)[0], 254);
    let new_tex = Arc::new(d.create_texture(&desc));
    q.write_texture(
        new_tex.as_image_copy(),
        &11u32.to_le_bytes(),
        Default::default(),
        desc.size,
    );
    r.bind_external_texture(id, TableId(21), new_tex).unwrap();
    assert_eq!(step(&mut r, &d, &q, id, &view), [265, 16]);
    assert_eq!(r.pipeline_creates(id).unwrap().compute.len(), pipeline_count);
}

fn host_buffer(d: &wgpu::Device) -> Arc<wgpu::Buffer> {
    use wgpu::BufferUsages as U;
    Arc::new(d.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 4,
        usage: U::STORAGE | U::COPY_DST | U::COPY_SRC,
        mapped_at_creation: false,
    }))
}

#[test]
fn checkpoint_contents_follow_the_state_layout() {
    let (mut r, d, q, _) = device();
    let mut b = bundle();
    let args = TableId(8);
    b.tables.push(TableDecl {
        id: args, format: ElemFormat::U32, lifetime: TableLifetime::Transient,
        stride_bytes: 4, capacity: 6, initial: Box::default(),
    });
    let source = br"
        @group(1) @binding(0) var<storage,read> loft_advance_args: array<u32>;
        @group(2) @binding(5) var<storage,read_write> rows: array<u32>;
        @compute @workgroup_size(1) fn update(@builtin(global_invocation_id) i: vec3u) {
            let row = loft_advance_args[3] + i.x;
            if row < loft_advance_args[4] {
                rows[row] += loft_advance_args[1] - loft_advance_args[0];
            }
        }
    ";
    b.modules[0].bytes = source.as_slice().into();
    b.modules[0].hash = *blake3::hash(source).as_bytes();
    b.passes[0].run = PassRun::ComputeFusedTicks {
        module: MODULE, entry: "update".into(), max_ticks: 2, instances: TABLE,
    };
    b.passes[0].bindings = [PassBinding {
        group: 1, slot: 0, resource: args, access: BindAccess::ReadStorage,
    }, b.passes[0].bindings[0]].into();

    let id = r.load_bundle(&b).unwrap();
    let mut enc = d.create_command_encoder(&Default::default());
    r.advance_into(id, &mut enc, 2, 1).unwrap();
    q.submit([enc.finish()]);
    r.hot_swap_bundle(id, &b).unwrap();
    r.restore_checkpoint(id, 1).unwrap();
    assert_eq!(read(&r, &d, &q, id), [2, 0]);

    b.state.as_mut().unwrap().layout_hash = [2; 32];
    r.hot_swap_bundle(id, &b).unwrap();
    assert_eq!(r.restore_checkpoint(id, 1), Err(RunnerError::UnknownResource));
    assert_eq!(read(&r, &d, &q, id), [1, 0]);
}

#[test]
fn bundle_load_uses_raster_and_sampler_overrides_with_vertex_tables() {
    use loft_ir::execution::{BlendMode, CullMode, Execution, RasterState, SamplerSpec};
    use wgpu::{BufferUsages as B, TextureUsages as T, util::DeviceExt};

    let (mut r, d, q, _) = device();
    let [args, vertices, image, sampler, color] = [0, 1, 2, 3, 4].map(TableId);
    let pass = PassId(0);
    let exec = Execution {
        raster: vec![RasterState { pass, cull: CullMode::Back, blend: BlendMode::Alpha }],
        samplers: vec![SamplerSpec { table: sampler, nearest: true, repeat: true }],
        ..Execution::default()
    };
    let source = br"
        @group(3) @binding(0) var image: texture_2d<f32>;
        @group(3) @binding(1) var image_sampler: sampler;
        @vertex fn vs(@location(0) pos: vec2f) -> @builtin(position) vec4f {
            return vec4f(pos, 0.0, 1.0);
        }
        @fragment fn fs() -> @location(0) vec4f {
            return vec4f(textureSample(image, image_sampler, vec2f(1.125, 0.5)).rgb, 0.5);
        }
    ";
    let mut bundle = Bundle {
        modules: vec![ModuleBlob {
            id: MODULE, target: ShaderTarget::Wgsl, hash: *blake3::hash(source).as_bytes(),
            bytes: source.as_slice().into(),
        }],
        tables: vec![
            TableDecl { id: args, format: ElemFormat::U32, lifetime: TableLifetime::Static,
                stride_bytes: 4, capacity: 4, initial: [6_u32, 1, 0, 0].into_iter().flat_map(u32::to_le_bytes).collect() },
            TableDecl { id: vertices, format: ElemFormat::F32, lifetime: TableLifetime::External,
                stride_bytes: 8, capacity: 6, initial: Box::default() },
            TableDecl { id: image, format: ElemFormat::U32, lifetime: TableLifetime::External,
                stride_bytes: 4, capacity: 0, initial: Box::default() },
            TableDecl { id: sampler, format: ElemFormat::U32, lifetime: TableLifetime::External,
                stride_bytes: 4, capacity: 0, initial: Box::default() },
            exec.table().unwrap(),
        ],
        passes: vec![PassDecl {
            id: pass, wave: 0, queue: QueueClass::Graphics,
            bindings: [
                PassBinding { group: 3, slot: 0, resource: image, access: BindAccess::SampledTex },
                PassBinding { group: 3, slot: 1, resource: sampler, access: BindAccess::Sampler },
            ].into(),
            run: PassRun::RenderBundle {
                draws: [IndirectDraw { module: MODULE, vertex_entry: "vs".into(), fragment_entry: "fs".into(),
                    args, offset_bytes: 0, vertex: Some(vertices), index: None }].into(),
                color_targets: [TargetDesc { table: color }].into(),
                depth: None, msaa: 1,
            },
        }],
        ..Bundle::default()
    };
    let id = r.load_bundle_bytes(&loft_runner::bundle::encode(&bundle)).unwrap();
    let desc = |width, usage| wgpu::TextureDescriptor {
        label: None, size: wgpu::Extent3d { width, height: 1, depth_or_array_layers: 1 },
        mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm, usage, view_formats: &[],
    };
    let sampled_desc = desc(2, T::TEXTURE_BINDING | T::COPY_DST);
    let sampled = Arc::new(d.create_texture(&sampled_desc));
    q.write_texture(sampled.as_image_copy(), &[255, 0, 0, 255, 0, 255, 0, 255],
        Default::default(), sampled_desc.size);
    r.declare_external_texture(id, image, &sampled_desc).unwrap();
    r.bind_external_texture(id, image, sampled).unwrap();
    let target = d.create_texture(&desc(1, T::RENDER_ATTACHMENT | T::COPY_SRC));
    let view = target.create_view(&Default::default());
    let front = [[-1.0_f32, -1.0], [3.0, -1.0], [-1.0, 3.0]];
    let back = [front[0], front[2], front[1]];
    let mut draw = |positions: [[f32; 2]; 6]| {
        let data: Vec<_> = positions.into_iter().flatten().flat_map(f32::to_le_bytes).collect();
        let vertex = Arc::new(d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None, contents: &data, usage: B::VERTEX,
        }));
        r.bind_external_buffer(id, vertices, vertex).unwrap();
        let mut enc = d.create_command_encoder(&Default::default());
        drop(enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view, depth_slice: None, resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLUE), store: wgpu::StoreOp::Store },
            })],
            ..Default::default()
        }));
        r.encode_into(id, &mut enc, &view, None, 0., 0.).unwrap();
        q.submit([enc.finish()]);
        loft_runner::read_texture(&d, &q, &target, 0).unwrap()
    };
    // Exactly one triangle blends nearest/repeated red over opaque blue.
    let pixel = draw([front[0], front[1], front[2], back[0], back[1], back[2]]);
    assert!((127..=128).contains(&pixel[0]) && pixel[1] == 0 && (127..=128).contains(&pixel[2]) && pixel[3] == 255,
        "expected half red over blue, got {pixel:?}");
    assert_eq!(draw([back[0], back[1], back[2], back[0], back[1], back[2]]), [0, 0, 255, 255]);

    bundle.tables.push(exec.table().unwrap());
    assert_eq!(r.load_bundle(&bundle), Err(RunnerError::AbiMismatch));
}
