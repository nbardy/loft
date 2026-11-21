//! Run GPU probes one at a time under tools/gpu_lock.sh.
use loft_cli::native::{self, Loaded};
use loft_ir::{execution::*, *};
use loft_runner::{Runner, frame::FrameOptions, read_buffer, read_texture};
use std::{fs, path::{Path, PathBuf}, sync::Arc};
use wgpu::{self as gpu, BufferUsages as B, TextureFormat as F, TextureUsages as T};

fn device(adapter: &gpu::Adapter, features: gpu::Features) -> (Arc<gpu::Device>, Arc<gpu::Queue>) {
    let (device, queue) = pollster::block_on(adapter.request_device(&gpu::DeviceDescriptor {
        required_features: features, ..Default::default()
    })).unwrap();
    (Arc::new(device), Arc::new(queue))
}

fn adapter() -> gpu::Adapter {
    let instance = gpu::Instance::new(gpu::InstanceDescriptor::new_without_display_handle_from_env());
    pollster::block_on(instance.request_adapter(&Default::default())).unwrap()
}

fn host(d: &Arc<gpu::Device>, q: &Arc<gpu::Queue>) -> Runner {
    Runner::attach(d.clone(), q.clone(), HostCaps { depth_convention: DepthConvention::StandardZeroToOne,
        msaa_samples: 1, features: GpuFeatures::empty() })
}

fn image(d: &gpu::Device, extent: [u32; 3]) -> gpu::Texture {
    d.create_texture(&gpu::TextureDescriptor { label: None,
        size: gpu::Extent3d { width: extent[0], height: extent[1], depth_or_array_layers: 1 },
        mip_level_count: 1, sample_count: 1, dimension: gpu::TextureDimension::D2,
        format: F::Rgba8UnormSrgb, usage: T::RENDER_ATTACHMENT | T::COPY_SRC, view_formats: &[],
    })
}

fn shader(id: u32, source: &str) -> ModuleBlob {
    ModuleBlob { id: ModuleId(id), target: ShaderTarget::Wgsl, hash: *blake3::hash(source.as_bytes()).as_bytes(),
        bytes: source.as_bytes().into() }
}

#[test]
fn padded_offset_adopted_and_arena_capacity() {
    let (d, q) = device(&adapter(), gpu::Features::empty());
    let alignment = d.limits().min_storage_buffer_offset_alignment;
    let source = r"
        @group(1) @binding(0) var<storage, read> input_words: array<u32>;
        @group(2) @binding(0) var<storage, read_write> output_words: array<u32>;
        @compute @workgroup_size(1) fn run() {
            output_words[0] = arrayLength(&input_words);
            output_words[1] = input_words[0];
        }";
    let input = TableId(0);
    let output = TableId(1);
    let mut b = Bundle {
        modules: vec![shader(0, source)],
        tables: vec![
            TableDecl { id: input, format: ElemFormat::U32, lifetime: TableLifetime::External,
                stride_bytes: 4, capacity: alignment / 4 + 3, initial: Box::new([]) },
            TableDecl { id: output, format: ElemFormat::U32, lifetime: TableLifetime::Transient,
                stride_bytes: 4, capacity: 2, initial: Box::new([]) },
        ],
        passes: vec![PassDecl { id: PassId(0), wave: 0, queue: QueueClass::Graphics,
            bindings: Box::new([
                PassBinding { group: 1, slot: 0, resource: input, access: BindAccess::ReadStorage },
                PassBinding { group: 2, slot: 0, resource: output, access: BindAccess::ReadWriteStorage },
            ]),
            run: PassRun::Compute { module: ModuleId(0), entry: "run".into(), dispatch: DispatchExtent::Fixed([1; 3]) },
        }], ..Default::default()
    };
    b.tables.extend(Execution { ranges: vec![BufferView { pass: PassId(0), group: 1, slot: 0,
        offset: u64::from(alignment), size: 12 }], ..Default::default() }.table());
    let mut r = host(&d, &q);
    let id = r.load_bundle(&b).unwrap();
    let target = image(&d, [1, 1, 1]).create_view(&Default::default());
    let sample = |r: &mut Runner, id| {
        let mut enc = d.create_command_encoder(&Default::default());
        let frame = r.encode_into(id, &mut enc, &target, None, 0., 1. / 120.).unwrap();
        q.submit([enc.finish()]);
        let words = read_buffer(&d, &q, &r.export_buffer(id, output).unwrap()).unwrap();
        (frame.resource_epoch, words.as_chunks::<4>().0.iter().copied().map(u32::from_le_bytes).collect::<Vec<_>>())
    };
    for (i, value) in [71_u32, 93].into_iter().enumerate() {
        let buffer = Arc::new(d.create_buffer(&gpu::BufferDescriptor { label: None, size: 1024,
            usage: B::STORAGE | B::COPY_DST, mapped_at_creation: false }));
        q.write_buffer(&buffer, 0, &19_u32.to_le_bytes());
        q.write_buffer(&buffer, alignment.into(), &value.to_le_bytes());
        r.bind_external_buffer(id, input, buffer).unwrap();
        assert_eq!(sample(&mut r, id), (i as u64 + 1, vec![3, value]), "binding range {alignment}..{}", alignment + 12);
    }
    b.tables.pop();
    b.tables[0].lifetime = TableLifetime::DynamicArena { max_rows: 16 };
    b.tables[0].capacity = 7;
    b.tables[0].initial = 5_u32.to_le_bytes().as_slice().into();
    b.rows.insert(KindId(0), input);
    let id = r.load_bundle(&b).unwrap();
    assert_eq!(sample(&mut r, id), (0, vec![7, 5]));
    r.resize_table(id, input, 12).unwrap();
    assert_eq!(sample(&mut r, id), (1, vec![12, 5]));
}

fn compile(file: &str, text: &str, overrides: &[(String, f64)]) -> (loft_passes::program::Program, loft_passes::Compiled) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = loft_passes::source::Graph::text(&root, Path::new(file), text);
    let (program, _, checked) = loft_passes::compile(&src, overrides).unwrap_or_else(|e| panic!("{}", e.render(&src)));
    (program, checked)
}

fn lower(program: &loft_passes::program::Program, checked: &loft_passes::Compiled, size: (u32, u32)) -> Bundle {
    loft_passes::bundle::lower(program, checked, size, |sun, alternate| {
        let light = alternate.map(|(lo, hi, side)| native::sun_camera(program.sun, lo, hi, side));
        native::frame_bytes(program.camera, program.sun, size, 0., sun, light)
    }).unwrap()
}

fn draw(runner: &mut Runner, id: InstanceId, d: &gpu::Device, q: &gpu::Queue, target: &gpu::Texture, time: f64) -> u32 {
    let mut enc = d.create_command_encoder(&Default::default());
    let report = runner.encode_frame(id, &mut enc, &target.create_view(&Default::default()), time, Default::default()).unwrap();
    q.submit([enc.finish()]);
    report.commands
}

#[test]
fn packed_bake_matches_direct_at_every_mip() {
    let adapter = adapter();
    let feature = gpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
    if !adapter.features().contains(feature) { eprintln!("direct control requires adapter-specific formats"); return; }
    let (d, q) = device(&adapter, gpu::Features::empty());
    let (native_d, native_q) = device(&adapter, feature);
    let src = r"
space World
param density: f = 1.0
bake_tex Odd 13x7 r16float on load mips(true) (uv: v2) { uv.x * 0.5 + uv.y * 0.25 }
bake_tex Cube cube 9x9 r16float on load mips(true) (dir: d3@World) { let v = dir.xyz; v.x * 0.25 + v.z * 0.5 }
bake_tex Volume 9x7x5 r8unorm on dirty(density) mips(true) (uvw: v3) {
  clamp((uvw.x + uvw.y + uvw.z) * density - 1.0, 0.0, 1.0)
}";
    let (p, checked) = compile("packed-bake.loft", src, &[]);
    let bytes = loft_runner::bundle::encode(&lower(&p, &checked, (16, 16)));
    let mut portable = host(&d, &q);
    let mut direct = host(&native_d, &native_q).with_adapter(adapter, None);
    let id = portable.load_bundle_bytes(&bytes).unwrap();
    let control = direct.load_bundle_bytes(&bytes).unwrap();
    let target = image(&d, [16, 16, 1]);
    let control_target = image(&native_d, [16, 16, 1]);
    for t in [0., 1. / 120.] {
        if t > 0. {
            let param = portable.params(id).iter().find(|p| p.name == "density").unwrap().id;
            portable.set_param(id, param, &[1.5]).unwrap();
            direct.set_param(control, param, &[1.5]).unwrap();
        }
        assert_eq!(draw(&mut portable, id, &d, &q, &target, t),
            draw(&mut direct, control, &native_d, &native_q, &control_target, t));
        for name in ["Odd", "Cube", "Volume"] {
            let t = portable.image(id, name).unwrap();
            let c = direct.image(control, name).unwrap();
            assert_eq!((t.format(), t.size(), t.mip_level_count()), (c.format(), c.size(), c.mip_level_count()), "{name}");
            assert!(t.usage().contains(T::COPY_DST));
            assert!(c.usage().contains(T::STORAGE_BINDING));
            for mip in 0..t.mip_level_count() {
                let packed = read_texture(&d, &q, &t, mip).unwrap();
                let direct = read_texture(&native_d, &native_q, &c, mip).unwrap();
                if let Some(i) = packed.iter().zip(&direct).position(|(a, b)| a != b) {
                    panic!("{name} mip {mip} byte {i}: packed={} direct={}", packed[i], direct[i]);
                }
                assert_eq!(packed.len(), direct.len());
            }
        }
    }
}

#[test]
fn ungated_storage_texture_is_refused_before_pipeline_creation() {
    let (d, q) = device(&adapter(), gpu::Features::empty());
    let table = TableId(0);
    let bundle = Bundle {
        modules: vec![shader(0, r"
            @group(3) @binding(6) var output: texture_storage_2d<r16float, write>;
            @compute @workgroup_size(1) fn run() {
                textureStore(output, vec2i(0), vec4f(1.0));
            }
        ")],
        tables: vec![TableDecl { id: table, format: ElemFormat::U32, lifetime: TableLifetime::External,
            stride_bytes: 4, capacity: 0, initial: Box::new([]) }],
        passes: vec![PassDecl { id: PassId(0), wave: 0, queue: QueueClass::Graphics,
            bindings: Box::new([PassBinding { group: 3, slot: 6, resource: table, access: BindAccess::StorageTex }]),
            run: PassRun::Compute { module: ModuleId(0), entry: "run".into(), dispatch: DispatchExtent::Fixed([1; 3]) },
        }],
        ..Default::default()
    };
    assert_eq!(host(&d, &q).load_bundle(&bundle), Err(loft_runner::RunnerError::TextureAccess {
        format: "R16Float".into(), access: "WriteOnly".into(),
    }));
}

#[test]
fn prepared_sessions_share_encoder_and_query_set() {
    let adapter = adapter();
    if !adapter.features().contains(gpu::Features::TIMESTAMP_QUERY) { return; }
    let (d, q) = device(&adapter, gpu::Features::TIMESTAMP_QUERY);
    let mut first = host(&d, &q);
    let mut second = host(&d, &q);
    let (p, checked) = compile("stdlib/engines/capsules.loft", include_str!("../../../stdlib/engines/capsules.loft"), &[("count".into(), 1.)]);
    let bundle = lower(&p, &checked, (32, 32));
    let a = first.load_bundle(&bundle).unwrap();
    let b = second.load_bundle(&bundle).unwrap();
    let x = image(&d, [32, 32, 1]);
    let y = image(&d, [32, 32, 1]);
    let queries = d.create_query_set(&gpu::QuerySetDescriptor { label: None, ty: gpu::QueryType::Timestamp, count: 20 });
    let xv = x.create_view(&Default::default());
    let yv = y.create_view(&Default::default());
    first.prepare_frame(a, &xv).unwrap();
    second.prepare_frame(b, &yv).unwrap();
    let mut enc = d.create_command_encoder(&Default::default());
    let f = first.encode_frame(a, &mut enc, &xv, 0., FrameOptions {
        timestamps: Some(&queries), timestamp_start: 0, ..Default::default()
    }).unwrap();
    let g = second.encode_frame(b, &mut enc, &yv, 0., FrameOptions {
        timestamps: Some(&queries), timestamp_start: 10, ..Default::default()
    }).unwrap();
    assert_eq!((f.commands, g.commands, f.stages.len(), g.stages.len()), (5, 5, 5, 5));
    q.submit([enc.finish()]);
    assert_eq!(read_texture(&d, &q, &x, 0).unwrap(), read_texture(&d, &q, &y, 0).unwrap());
}

#[test]
#[ignore = "full-resolution lane evidence; run separately under the GPU lock"]
fn archive_frames() {
    let out = PathBuf::from(std::env::var_os("LOFT_D2C_OUTPUT").expect("output directory"));
    fs::create_dir_all(&out).unwrap();
    let g = native::gpu().unwrap();
    let (d, q) = device(&adapter(), gpu::Features::empty());
    let size = (1280, 800);
    let target = image(&d, [size.0, size.1, 1]);
    for family in ["capsules", "blobs", "hermit"] {
        let scene = format!("stdlib/engines/{family}.loft");
        let text = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(&scene)).unwrap();
        for count in [1, 20] {
            let (p, checked) = compile(&scene, &text, &[("count".into(), f64::from(count))]);
            let light = std::cell::Cell::new(None);
            let b = loft_passes::bundle::lower(&p, &checked, size, |sun, alternate| {
                light.set(alternate.map(|(lo, hi, side)| native::sun_camera(p.sun, lo, hi, side)));
                native::frame_bytes(p.camera, p.sun, size, 0., sun, light.get())
            }).unwrap();
            let frame = b.resources.frame.as_ref().unwrap();
            let bytes = loft_runner::bundle::encode(&b);
            assert_eq!(loft_runner::bundle::encode(&loft_runner::bundle::decode(&bytes).unwrap()), bytes);
            let mut r = host(&d, &q);
            let id = r.load_bundle_bytes(&bytes).unwrap();
            let loaded = Loaded::new(&g, &p, size, None).unwrap();
            let hdr = b.resources.textures.iter().find(|x| matches!(x.format, ImageFormat::Rgba16Float)).unwrap();
            for t in [0, 4, 9] {
                let prefix = format!("{family}-{count}_t{t}");
                let native_commands = loaded.frame(&g, t as f32);
                let uniform = |table| {
                    let mut bytes = native::frame_bytes(p.camera, p.sun, size, t as f32, table != frame.main, light.get());
                    bytes[188..192].copy_from_slice(&1_u32.to_le_bytes());
                    bytes
                };
                let mut enc = d.create_command_encoder(&Default::default());
                let options = FrameOptions { uniform: Some(&uniform), ..Default::default() };
                let archive_commands = r.encode_frame(id, &mut enc, &target.create_view(&Default::default()), f64::from(t), options).unwrap().commands;
                q.submit([enc.finish()]);
                let archive = read_texture(&d, &q, &target, 0).unwrap();
                let native = loaded.pixels(&g).unwrap();
                fs::write(out.join(format!("{prefix}.native.rgba")), &native).unwrap();
                fs::write(out.join(format!("{prefix}.archive.rgba")), &archive).unwrap();
                assert!(archive == native, "{prefix}: native and archive pixels differ with the same host camera");
                let hp = r.export_texture(id, hdr.table).unwrap();
                let words = read_texture(&d, &q, &hp, 0).unwrap();
                let center = 8 * (400 * 1280 + 640);
                eprintln!("{prefix} commands {native_commands}/{archive_commands}, center HDR {:02x?}, RGBA {:?}",
                    &words[center..center+8], &archive[center/2..center/2+4]);
            }
        }
    }
}

#[test]
fn sampled_study_ink_draws_from_the_portable_bake() {
    let scene = include_str!("../../../scenes/spectral_caustic_study.loft");
    let sampled = scene.replace("let stroke = study_ink(p.xz)",
        "let stroke = sample(StudyInk, 0.5 * (p.xz + [1.0, 1.0]))");
    let (program, compiled) = compile("scenes/spectral_caustic_study.loft", &sampled, &[]);
    let size = (64, 48);
    let bundle = lower(&program, &compiled, size);
    let gpu = native::gpu().unwrap();
    let (d, q) = (Arc::new(gpu.device.clone()), Arc::new(gpu.queue.clone()));
    let mut portable = host(&d, &q);
    let id = portable.load_bundle_bytes(&loft_runner::bundle::encode(&bundle)).unwrap();
    let target = image(&d, [size.0, size.1, 1]);
    draw(&mut portable, id, &d, &q, &target, 0.);
    assert_eq!(portable.image(id, "StudyInk").unwrap().mip_level_count(), 10);
    let native = Loaded::new(&gpu, &program, size, None).unwrap();
    native.frame(&gpu, 0.);
    assert_eq!(read_texture(&d, &q, &target, 0).unwrap(), native.pixels(&gpu).unwrap());
}
