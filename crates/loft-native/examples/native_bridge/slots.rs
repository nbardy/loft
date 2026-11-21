use loft_ir::{GpuFeatures, HostCaps};
use loft_native::Gpu;
use loft_runner::Runner;
use std::{path::Path, sync::Arc};

pub fn scene(name: &str) -> loft_passes::source::Graph {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/native_bridge")
        .join(name);
    loft_passes::source::Graph::file(&path, "").unwrap()
}

pub fn attach(g: &Gpu, caps: HostCaps) -> Runner {
    Runner::attach(Arc::new(g.device.clone()), Arc::new(g.queue.clone()), caps)
}

pub fn texture(
    g: &Gpu,
    size: [u32; 2],
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    g.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

pub fn check(g: &Gpu) {
    let src = scene("slots.loft");
    let (program, _, compiled) =
        loft_passes::compile(&src, &[]).unwrap_or_else(|err| panic!("{}", err.render(&src)));
    let linked =
        loft_passes::bundle::lower(&program, &compiled, (4, 4), |_, _| vec![0; 256]).unwrap();
    let bundle = loft_runner::bundle::encode(&linked);
    loft_runner::bundle::validate_wgsl(&loft_runner::bundle::decode(&bundle).unwrap()).unwrap();
    let target = texture(
        g,
        [1; 2],
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
    )
    .create_view(&Default::default());
    assert!(
        g.host_caps()
            .features
            .contains(GpuFeatures::PASSTHROUGH_SHADERS)
    );
    // The fixed fixture uses exactly two input words, two output words and one in-bounds texel.
    #[allow(unsafe_code)]
    let native = unsafe { attach(g, g.host_caps()).with_trusted_msl() };
    for (sentinel, mut runner) in [attach(g, HostCaps::WEB_BASELINE), native]
        .into_iter()
        .enumerate()
    {
        let id = runner.load_bundle_bytes(&bundle).unwrap();
        let mut enc = g.device.create_command_encoder(&Default::default());
        runner
            .encode_into(id, &mut enc, &target, None, 0.0, 1.0 / 120.0)
            .unwrap();
        g.queue.submit([enc.finish()]);
        let output = runner
            .export_buffer(id, compiled.compute.tables["Output"])
            .unwrap();
        let bytes = loft_runner::read_buffer(&g.device, &g.queue, &output).unwrap();
        assert_eq!(u32::from_le_bytes(bytes[..4].try_into().unwrap()), 29);
        assert_eq!(
            u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
            sentinel as u32
        );
    }
    println!("MSL bfloat and sparse buffer/texture slot witness: portable=[29, 0], Metal=[29, 1]");
}
