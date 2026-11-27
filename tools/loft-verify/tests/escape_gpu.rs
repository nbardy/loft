use loft_ir::{DepthConvention, GpuFeatures, HostCaps};
use loft_passes::emit::Aov;
use loft_runner::{Runner, RunnerError};
use loft_verify::cpu_twin::step_bundle_cpu;
use std::{collections::BTreeMap, path::Path, sync::Arc};

fn sources(scene: &str) -> loft_passes::source::Graph {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    loft_passes::source::Graph::file(&root.join(scene), "").unwrap()
}

#[test]
fn opted_out_trace_renders_beauty_and_normals() {
    let src = sources("crates/loft-passes/tests/escapes/geometry.loft");
    let g = loft_cli::native::gpu().unwrap();
    for aov in [Aov::Beauty, Aov::Normal] {
        let (program, report, _) = loft_passes::compile_aov(&src, &[], aov)
            .unwrap_or_else(|d| panic!("{}", d.render(&src)));
        let loaded = loft_cli::native::Loaded::with_aov(&g, &program, (96, 64), None, aov).unwrap();
        loaded.frame(&g, 0.0);
        let pixels = loaded.pixels(&g).unwrap();
        let nonempty = pixels
            .chunks_exact(4)
            .filter(|p| p[..3] != [0, 0, 0])
            .count();
        assert!(nonempty > 100);
        if aov == Aov::Normal {
            assert!(nonempty < 96 * 64);
        }
        eprintln!(
            "{}: {aov:?}, {nonempty} colored pixels, proof {:?}",
            g.adapter, report.kinds[0].proof
        );
    }
}

#[test]
fn particle_scatter_statistical_twin() {
    let src = sources("scenes/particle_scatter.loft");
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let feature = wgpu::Features::SHADER_FLOAT32_ATOMIC & adapter.features();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: feature,
        ..Default::default()
    })).unwrap();
    let device = Arc::new(device);
    let queue = Arc::new(queue);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    }).create_view(&Default::default());
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 128,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut choices = vec![GpuFeatures::empty()];
    if !feature.is_empty() { choices.push(GpuFeatures::FLOAT32_ATOMICS); }
    else { eprintln!("{}: float32 atomics unavailable", adapter.get_info().name); }
    for features in choices {
        let mut runner = Runner::attach(device.clone(), queue.clone(), HostCaps {
            depth_convention: DepthConvention::StandardZeroToOne,
            msaa_samples: 1,
            features,
        });
        let (mut worst_cell, mut worst_global) = (0.0_f64, 0.0_f64);
        let mut squared_error = 0.0;
        for seed in 0_u32..32 {
            let span = if seed % 2 == 0 { 16 } else { 2 };
            let reverse = seed & 2 != 0;
            let args = [
                ("seed".into(), f64::from(seed)),
                ("bins".into(), f64::from(span)),
                ("reverse".into(), f64::from(u8::from(reverse))),
            ];
            let (_, _, compiled) = loft_passes::compile_for_device(&src, &args, Aov::Beauty, features)
                .unwrap_or_else(|d| panic!("{}", d.render(&src)));
            let table = compiled.compute.tables["Grid"];
            let id = runner.load_bundle(&compiled.compute.bundle).unwrap();
            let mut encoder = device.create_command_encoder(&Default::default());
            runner.encode_into(id, &mut encoder, &target, None, 0., 0.).unwrap();
            encoder.copy_buffer_to_buffer(&runner.export_buffer(id, table).unwrap(), 0, &readback, 0, 128);
            queue.submit([encoder.finish()]);
            let slice = readback.slice(..);
            let (tx, rx) = std::sync::mpsc::channel();
            slice.map_async(wgpu::MapMode::Read, move |result| tx.send(result).unwrap());
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            rx.recv().unwrap().unwrap();
            let mapped = slice.get_mapped_range().unwrap();
            let gpu = mapped.chunks_exact(4).map(|bytes| f64::from(f32::from_le_bytes(bytes.try_into().unwrap())));
            let (cpu, absolute_momentum) = scatter_reference(seed, span);
            let mut difference = [0.0; 2];
            for (i, (gpu, cpu)) in gpu.zip(cpu.into_iter().flatten()).enumerate() {
                assert!(gpu.is_finite());
                let error = gpu - cpu;
                worst_cell = worst_cell.max(error.abs());
                squared_error += error * error;
                difference[i % 2] += error;
            }
            worst_global = worst_global.max((difference[0] / 256.).abs())
                .max((difference[1] / absolute_momentum).abs());
            drop(mapped);
            readback.unmap();
        }
        let rms = (squared_error / (32. * 16. * 2.)).sqrt();
        eprintln!("{} {features:?}: 32 seeds, max={worst_cell:.9}, RMS={rms:.9}, conservation={worst_global:.9}",
            adapter.get_info().name);
        assert!(worst_cell <= 0.0001 && rms <= 0.00003 && worst_global <= 0.00001);
    }
}

fn scatter_reference(seed: u32, span: u32) -> ([[f64; 2]; 16], f64) {
    let mut grid = [[0.; 2]; 16];
    let mut absolute_momentum = 0.;
    for id in 0_u32..256 {
        let word = id.wrapping_mul(747796405).wrapping_add(seed.wrapping_mul(2891336453));
        let (cell, fraction) = ((word >> 20) % span, f64::from((word >> 16) & 15) / 16.);
        let momentum = f64::from(((word >> 24) % 127) as i32 - 63) / 63.;
        absolute_momentum += momentum.abs();
        grid[cell as usize][0] += 1. - fraction;
        grid[cell as usize][1] += momentum * (1. - fraction);
        let next = ((cell + 1) % span) as usize;
        grid[next][0] += fraction;
        grid[next][1] += momentum * fraction;
    }
    (grid, absolute_momentum)
}

#[test]
fn nondet_law_matches_a_statistical_twin_and_refuses_cpu_stepping() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = loft_passes::source::Graph::text(&root, Path::new("scenes/drift_probe.loft"), r"
        use stdlib::materials::bsdf::{ggx}
        use stdlib::sdf::solid::{sphere}
        space World
        fn wave(x: f) -> f = wgsl (x: f) -> f { return sin(x); }
        kind Wisp @World (x: f) {
            trace(p) = sphere(p - [0, 0, 0]@World, 0.5)
            material(p,n) = ggx(rgb(0.4), 0.8, 0)
        }
        build {
            emit Wisp(x: -0.6)
            emit Wisp(x: -0.1)
            emit Wisp(x: 0.2)
            emit Wisp(x: 0.8)
        }
        law drift(w: Wisp, dt) nondet writes w.{x} {
            w.x = wave(w.x + dt + time * 0.01)
        }
        schedule Main { run_laws(drift) }
    ");
    let (program, _, _) = loft_passes::compile(&src, &[])
        .unwrap_or_else(|diag| panic!("{}", diag.render(&src)));
    let g = loft_cli::native::gpu().unwrap();
    let motion = loft_passes::motion::motion_bundle(&program, &program.dispatches[0], 0);
    assert!(matches!(
        step_bundle_cpu(
            &motion,
            loft_ir::PassId(0),
            &mut BTreeMap::new(),
            loft_ir::InstanceId(0),
            0..4,
            loft_passes::motion::advance(0, 1, 0).unwrap(),
        ),
        Err(RunnerError::Unsupported("NonDet ticks require GPU execution"))
    ));

    let initial = [-0.6_f64, -0.1, 0.2, 0.8];
    let mut max_error = 0.0_f64;
    for ticks in [1, 31, 128] {
        let sampled = loft_cli::native::sample_motion(&g, &program, ticks as f32 / 120.).unwrap();
        let [(field, values)] = sampled[0].fields.as_slice() else {
            panic!("expected one motion field");
        };
        assert_eq!(field, "x");
        assert_eq!(values.len(), initial.len());
        for (&gpu, x) in values.iter().zip(initial) {
            let expected = (1..=ticks).fold(x, |state, tick| {
                (state + 1. / 120. + f64::from(tick) / 120. * 0.01).sin()
            });
            assert!(gpu.is_finite());
            max_error = max_error.max((f64::from(gpu) - expected).abs());
        }
    }
    eprintln!("{}: NonDet law, 12 observations, max={max_error:.9}", g.adapter);
    assert!(max_error <= 0.00003);
}
