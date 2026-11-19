//! Local host acceptance probe. Only the host initializes, submits, and reads attachments.
use loft_ir::*;
use loft_runner::Runner;
use std::{error::Error, sync::Arc};
use wgpu::*;

fn bundle() -> Bundle {
    let source = r#"
        @vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
            let p = array<vec2f,3>(vec2f(-1,-1), vec2f(3,-1), vec2f(-1,3));
            return vec4f(p[i],0.5,1);
        }
        struct Out { @location(0) color: vec4f, @builtin(frag_depth) z: f32 }
        @fragment fn fs(@builtin(position) p: vec4f) -> Out {
            return Out(vec4f(1,0,0,1), select(0.25,0.75,p.x>1.0));
        }"#;
    Bundle {
        modules: vec![ModuleBlob {
            id: ModuleId(0),
            target: ShaderTarget::Wgsl,
            hash: *blake3::hash(source.as_bytes()).as_bytes(),
            bytes: source.as_bytes().into(),
        }],
        tables: vec![TableDecl {
            id: TableId(0),
            format: ElemFormat::U32,
            lifetime: TableLifetime::Static,
            stride_bytes: 4,
            capacity: 4,
            initial: [3u32, 1, 0, 0]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect(),
        }],
        passes: vec![PassDecl {
            id: PassId(0),
            queue: QueueClass::Graphics,
            wave: 0,
            bindings: Box::default(),
            run: PassRun::RenderBundle {
                draws: Box::new([IndirectDraw {
                    module: ModuleId(0),
                    vertex_entry: "vs".into(),
                    fragment_entry: "fs".into(),
                    args: TableId(0),
                    offset_bytes: 0,
                    vertex: None,
                    index: None,
                }]),
                color_targets: Box::new([TargetDesc { table: TableId(1) }]),
                depth: Some(DepthDesc {
                    table: TableId(2),
                    write: false,
                }),
                msaa: 1,
            },
        }],
        ..Default::default()
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let api = Instance::new(InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(api.request_adapter(&Default::default()))?;
    println!(
        "adapter {:?}, hardware ray query {}",
        adapter.get_info(),
        adapter
            .features()
            .contains(Features::EXPERIMENTAL_RAY_QUERY)
    );
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default()))?;
    let (d, q) = (Arc::new(device), Arc::new(queue));
    let size = Extent3d {
        width: 2,
        height: 1,
        depth_or_array_layers: 1,
    };
    let attachment = |format| {
        d.create_texture(&TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    };
    let color = attachment(TextureFormat::Rgba8Unorm);
    let cv = color.create_view(&Default::default());
    let dv = attachment(TextureFormat::Depth32Float).create_view(&Default::default());
    let red = [255, 0, 0, 255];
    let blue = [0, 0, 255, 255];
    for (depth_convention, expected) in [
        (DepthConvention::StandardZeroToOne, [red, blue]),
        (DepthConvention::ReverseOneToZero, [blue, red]),
    ] {
        let mut runner = Runner::attach(
            d.clone(),
            q.clone(),
            HostCaps {
                depth_convention,
                msaa_samples: 1,
                features: GpuFeatures::empty(),
            },
        );
        let id = runner
            .load_bundle(&bundle())
            .map_err(|e| format!("fixture load: {e:?}"))?;
        let mut enc = d.create_command_encoder(&Default::default());
        drop(enc.begin_render_pass(&RenderPassDescriptor {
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &cv,
                resolve_target: None,
                depth_slice: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::BLUE),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &dv,
                depth_ops: Some(Operations {
                    load: LoadOp::Clear(0.5),
                    store: StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        }));
        runner
            .encode_into(id, &mut enc, &cv, Some(&dv), 0.0, 0.0)
            .map_err(|e| format!("host encode: {e:?}"))?;
        let read = d.create_buffer(&BufferDescriptor {
            label: None,
            size: 256,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        enc.copy_texture_to_buffer(
            color.as_image_copy(),
            TexelCopyBufferInfo {
                buffer: &read,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: None,
                },
            },
            size,
        );
        q.submit([enc.finish()]);
        read.map_async(MapMode::Read, .., |s| s.unwrap());
        d.poll(PollType::wait_indefinitely())?;
        let pixels = read.get_mapped_range(..)?;
        assert_eq!(&pixels[..8], expected.as_flattened());
        println!("{depth_convention:?} {:?}", &pixels[..8]);
    }
    if let Some(path) = std::env::args_os().nth(1) {
        let bytes = std::fs::read(path)?;
        let mut runner = Runner::attach(
            d.clone(),
            q.clone(),
            HostCaps {
                depth_convention: DepthConvention::StandardZeroToOne,
                msaa_samples: 1,
                features: GpuFeatures::empty(),
            },
        );
        let id = runner
            .load_bundle_bytes(&bytes)
            .map_err(|e| format!("device load: {e:?}"))?;
        println!("loaded {id:?}");
        let decl = loft_runner::bundle::decode(&bytes)?;
        if let Some(texture) = decl.resources.textures.first() {
            let target = d.create_texture(&TextureDescriptor {
                label: None, size: Extent3d {
                    width: texture.extent[0], height: texture.extent[1], depth_or_array_layers: 1,
                },
                mip_level_count: 1, sample_count: 1, dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8UnormSrgb, usage: TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = target.create_view(&Default::default());
            let mut enc = d.create_command_encoder(&Default::default());
            for time in [0.0, 1.0 / 120.0] {
                runner.encode_into(id, &mut enc, &view, None, time, 1.0 / 120.0)
                    .map_err(|e| format!("device encode: {e:?}"))?;
            }
            q.submit([enc.finish()]);
            d.poll(PollType::wait_indefinitely())?;
            println!("executed two frames at {}x{}", texture.extent[0], texture.extent[1]);
        }

    }
    Ok(())
}
