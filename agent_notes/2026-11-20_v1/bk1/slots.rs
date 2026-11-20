//! Compare WGSL runtime lengths to a generated MSL signature under the pinned Metal HAL.
use wgpu::*;
fn buffer(slot: u32, ty: BufferBindingType) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding: slot,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}
fn main() {
    let api = Instance::new(InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(api.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&DeviceDescriptor {
        required_features: Features::PASSTHROUGH_SHADERS,
        ..Default::default()
    }))
    .unwrap();
    let read_only = BufferBindingType::Storage { read_only: true };
    let read_write = BufferBindingType::Storage { read_only: false };
    let tex = BindGroupLayoutEntry {
        binding: 9,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Uint,
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    };
    let entries = [
        vec![
            buffer(2, read_only),
            buffer(0, BufferBindingType::Uniform),
            buffer(1, read_only),
        ],
        vec![buffer(7, read_only)],
        vec![buffer(5, read_write)],
        vec![tex],
    ];
    let mut attributes = std::collections::BTreeMap::new();
    let (mut buffers, mut textures) = (0, 0);
    for (g, group) in entries.iter().enumerate() {
        let mut sorted = group.clone();
        sorted.sort_by_key(|e| e.binding);
        for e in sorted {
            let attr = match e.ty {
                BindingType::Buffer { .. } => {
                    buffers += 1;
                    format!("buffer({})", buffers - 1)
                }
                BindingType::Texture { .. } => {
                    textures += 1;
                    format!("texture({})", textures - 1)
                }
                _ => unreachable!(),
            };
            attributes.insert((g, e.binding), attr);
        }
    }
    let msl = format!(
        "#include <metal_stdlib>\nusing namespace metal;\nkernel void stamp(device const uint& bytes [[{}]], device uint* answer [[{}]], texture2d<uint, access::read> img [[{}]]) {{ answer[0] = img.get_width() + bytes/4u; }}",
        attributes[&(1, 7)],
        attributes[&(2, 5)],
        attributes[&(3, 9)]
    );
    println!(
        "{}\nreserved sizes buffer({buffers})\n{msl}",
        adapter.get_info().name
    );
    // The signature and explicit compute layout describe the same resources and workgroup.
    #[allow(unsafe_code)]
    let metal_module = unsafe {
        device.create_shader_module_passthrough(ShaderModuleDescriptorPassthrough {
            entry_points: vec![PassthroughShaderEntryPoint {
                name: "stamp".into(),
                workgroup_size: (1, 1, 1),
            }]
            .into(),
            msl: Some(msl.into()),
            ..Default::default()
        })
    };
    let wgsl_module = device.create_shader_module(ShaderModuleDescriptor {
        label: None,
        source: ShaderSource::Wgsl(
            r"
            @group(2) @binding(5) var<storage, read_write> answer: array<u32>;
            @group(3) @binding(9) var img: texture_2d<u32>;
            @compute @workgroup_size(1) fn stamp() {
                answer[0] = textureDimensions(img).x + arrayLength(&answer);
            }
        "
            .into(),
        ),
    });
    let layouts = entries.each_ref().map(|group| {
        device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: None,
            entries: group,
        })
    });
    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &layouts.each_ref().map(Some),
        immediate_size: 0,
    });
    let image = device.create_texture(&TextureDescriptor {
        label: None,
        size: Extent3d {
            width: 40,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::R32Uint,
        usage: TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = image.create_view(&Default::default());
    let alloc = |size, usage| {
        device.create_buffer(&BufferDescriptor {
            label: None,
            size,
            usage,
            mapped_at_creation: false,
        })
    };
    let unused = alloc(256, BufferUsages::UNIFORM | BufferUsages::STORAGE);
    let length = alloc(4, BufferUsages::STORAGE | BufferUsages::COPY_DST);
    let answer = alloc(
        512,
        BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
    );
    let bind = |g: usize, entries: &[BindGroupEntry]| {
        device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &layouts[g],
            entries,
        })
    };
    let fixed = bind(
        0,
        &[0, 1, 2].map(|binding| BindGroupEntry {
            binding,
            resource: unused.as_entire_binding(),
        }),
    );
    let lengths = bind(
        1,
        &[BindGroupEntry {
            binding: 7,
            resource: length.as_entire_binding(),
        }],
    );
    let img = bind(
        3,
        &[BindGroupEntry {
            binding: 9,
            resource: BindingResource::TextureView(&view),
        }],
    );
    for (name, module) in [("WGSL", wgsl_module), ("MSL", metal_module)] {
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("stamp"),
            compilation_options: Default::default(),
            cache: None,
        });
        for bytes in [8u32, 4u32] {
            queue.write_buffer(&answer, 256, &0u32.to_le_bytes());
            queue.write_buffer(&length, 0, &bytes.to_le_bytes());
            let buf = bind(
                2,
                &[BindGroupEntry {
                    binding: 5,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &answer,
                        offset: 256,
                        size: BufferSize::new(bytes.into()),
                    }),
                }],
            );
            let read = alloc(4, BufferUsages::COPY_DST | BufferUsages::MAP_READ);
            let mut enc = device.create_command_encoder(&Default::default());
            {
                let mut pass = enc.begin_compute_pass(&Default::default());
                pass.set_pipeline(&pipeline);
                for (i, g) in [&fixed, &lengths, &buf, &img].into_iter().enumerate() {
                    pass.set_bind_group(i as u32, g, &[]);
                }
                pass.dispatch_workgroups(1, 1, 1);
            }
            enc.copy_buffer_to_buffer(&answer, 256, &read, 0, Some(4));
            queue.submit([enc.finish()]);
            read.map_async(MapMode::Read, .., |r| r.unwrap());
            device.poll(PollType::wait_indefinitely()).unwrap();
            let word =
                u32::from_le_bytes((&*read.get_mapped_range(..).unwrap()).try_into().unwrap());
            println!("{name}: offset=256 bytes={bytes} word={word}");
            assert_eq!(word, 40 + bytes / 4);
        }
    }
}
