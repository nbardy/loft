use loft_ir::{DepthConvention, GpuFeatures, HostCaps};
use loft_passes::emit::Aov;
use loft_runner::Runner;
use std::{fs, path::Path, sync::Arc};

fn sources(scene: &str) -> loft_passes::source::Graph {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    loft_passes::source::Graph::file(&root.join(scene), "").unwrap()
}

#[test]
fn source_compute_bundle_runs_with_gpu_readback() {
    let src = sources("crates/loft-passes/tests/escapes/compute.loft");
    let (program, _, compiled) =
        loft_passes::compile(&src, &[]).unwrap_or_else(|d| panic!("{}", d.render(&src)));
    let compute = compiled.compute;
    let artifact = tempfile::NamedTempFile::new().unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_loft"))
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .args([
            "build",
            "crates/loft-passes/tests/escapes/compute.loft",
            "-o",
        ])
        .arg(artifact.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = fs::read(artifact.path()).unwrap();
    let bundle = loft_runner::bundle::decode(&bytes).unwrap();
    loft_runner::bundle::validate_wgsl(&bundle).unwrap();
    let g = loft_cli::native::gpu().unwrap();
    let mut runner = Runner::attach(
        Arc::new(g.device.clone()),
        Arc::new(g.queue.clone()),
        HostCaps {
            depth_convention: DepthConvention::StandardZeroToOne,
            msaa_samples: 1,
            features: GpuFeatures::empty(),
        },
    );
    let id = runner.load_bundle_bytes(&bytes).unwrap();
    let loaded = loft_cli::native::Loaded::new(&g, &program, (32, 32), None).unwrap();
    let texture = loaded.baked_texture("Probe").unwrap();
    let probe = compute.tables["Probe"];
    runner
        .declare_external_texture(
            id,
            probe,
            &wgpu::TextureDescriptor {
                label: Some("Probe"),
                size: texture.size(),
                mip_level_count: texture.mip_level_count(),
                sample_count: texture.sample_count(),
                dimension: texture.dimension(),
                format: texture.format(),
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        )
        .unwrap();
    runner
        .bind_external_texture(id, probe, Arc::new(texture))
        .unwrap();
    let target = g.device.create_texture(&wgpu::TextureDescriptor {
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
    });
    let read = g.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 20,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = g.device.create_command_encoder(&Default::default());
    runner
        .encode_into(
            id,
            &mut enc,
            &target.create_view(&Default::default()),
            None,
            0.0,
            0.0,
        )
        .unwrap();
    for (name, offset, n) in [
        ("Fixed", 0, 4),
        ("Float", 4, 4),
        ("Texel", 8, 4),
        ("Output", 12, 8),
    ] {
        let buffer = runner.export_buffer(id, compute.tables[name]).unwrap();
        enc.copy_buffer_to_buffer(&buffer, 0, &read, offset, n);
    }
    g.queue.submit([enc.finish()]);
    let slice = read.slice(..);
    let (send, recv) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        send.send(result).unwrap()
    });
    g.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    recv.recv().unwrap().unwrap();
    let view = slice.get_mapped_range().unwrap();
    let words = view
        .chunks_exact(4)
        .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
        .collect::<Vec<_>>();
    let expected = [1, 2, 3, 4, 5, 6, 7, 8].iter().sum::<u32>();
    assert_eq!(
        words,
        [
            expected,
            (expected as f32 * 0.5).to_bits(),
            0.375f32.to_bits(),
            42,
            0
        ]
    );
    eprintln!(
        "{}: i32={} f32={} sample={} WGSL={:?} bundle={} bytes",
        g.adapter,
        words[0],
        f32::from_bits(words[1]),
        f32::from_bits(words[2]),
        &words[3..],
        bytes.len()
    );
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
