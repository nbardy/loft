//! Run stamp.wgsl through the bundle codec and runner, then compare host and runner allocations.
use loft_ir::*;
use std::sync::Arc;
use wgpu::{BufferUsages as U, TextureUsages as T};

fn main() {
    let source = include_str!("stamp.wgsl");
    let table = TableId(11);
    let module = ModuleId(3);
    let bundle = Bundle {
        modules: vec![ModuleBlob {
            id: module,
            target: ShaderTarget::Wgsl,
            hash: *blake3::hash(source.as_bytes()).as_bytes(),
            bytes: source.as_bytes().into(),
        }],
        tables: vec![TableDecl {
            id: table,
            format: ElemFormat::U32,
            lifetime: TableLifetime::Static,
            stride_bytes: 4,
            capacity: 2,
            initial: vec![0; 8].into(),
        }],
        passes: vec![PassDecl {
            id: PassId(1),
            wave: 0,
            queue: QueueClass::AsyncCompute,
            bindings: vec![PassBinding {
                group: 2,
                slot: 5,
                resource: table,
                access: BindAccess::ReadWriteStorage,
            }]
            .into(),
            run: PassRun::Compute {
                module,
                entry: "stamp".into(),
                dispatch: DispatchExtent::Fixed([1, 1, 1]),
            },
        }],
        ..Default::default()
    };
    let gpu = loft_cli::native::gpu().unwrap();
    let device = Arc::new(gpu.device);
    let queue = Arc::new(gpu.queue);
    let mut runner = loft_runner::Runner::attach(
        device.clone(),
        queue.clone(),
        HostCaps {
            depth_convention: DepthConvention::StandardZeroToOne,
            msaa_samples: 1,
            features: GpuFeatures::empty(),
        },
    );
    let bytes = loft_runner::bundle::encode(&bundle);
    let view = device
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
            usage: T::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default());
    for host_buffer in [true, false] {
        // A host rebind persists for the life of the instance. Start each arm from the archive.
        let instance = runner.load_bundle_bytes(&bytes).unwrap();
        if host_buffer {
            runner
                .bind_external_buffer(
                    instance,
                    table,
                    Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
                        label: None,
                        size: 8,
                        usage: U::STORAGE | U::COPY_SRC,
                        mapped_at_creation: false,
                    })),
                )
                .unwrap();
        }
        let output = runner.export_buffer(instance, table).unwrap();
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 8,
            usage: U::COPY_DST | U::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&Default::default());
        runner
            .encode_into(instance, &mut enc, &view, None, 0., 0.)
            .unwrap();
        enc.copy_buffer_to_buffer(&output, 0, &staging, 0, Some(8));
        queue.submit([enc.finish()]);
        staging.map_async(wgpu::MapMode::Read, .., |r| r.unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let result: Vec<_> = staging
            .get_mapped_range(..)
            .unwrap()
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        println!(
            "{}: LOFTB001 {} bytes, stamp(1,1,1), host={host_buffer}, API size={}, table 11 = {:?}",
            gpu.adapter,
            bytes.len(),
            output.size(),
            result
        );
        assert_eq!(
            result,
            [42, 0],
            "arrayLength must use the eight-byte binding"
        );
    }
}
