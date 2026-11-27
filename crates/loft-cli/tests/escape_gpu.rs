use loft_ir::{DepthConvention, GpuFeatures, HostCaps};
use loft_runner::Runner;
use std::{fs, path::Path, sync::Arc};

fn sources(scene: &str) -> loft_passes::source::Graph {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    loft_passes::source::Graph::file(&root.join(scene), "").unwrap()
}

#[test]
fn source_compute_bundle_runs_with_gpu_readback() {
    let src = sources("crates/loft-passes/tests/escapes/compute.loft");
    let (_, _, compiled) =
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
    assert!(runner.image(id, "Probe").is_some());
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
            21,
            33
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
