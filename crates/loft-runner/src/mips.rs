use super::*;
use wgpu::*;

pub(super) struct Chain {
    epoch: u64,
    pipeline: ComputePipeline,
    levels: Vec<(BindGroup, u32, u32)>,
}

impl Chain {
    pub fn prepare(device: &Device, table: &Table, cached: &mut Option<Self>) -> super::Result<()> {
        if cached
            .as_ref()
            .is_some_and(|chain| chain.epoch == table.epoch)
        {
            return Ok(());
        }
        let Resource::Texture {
            handle: Some(texture),
            ..
        } = &table.resource
        else {
            return Err(RunnerError::UnknownResource);
        };
        if texture.dimension() != TextureDimension::D2
            || texture.depth_or_array_layers() != 1
            || texture.mip_level_count() < 2
            || texture.format() != TextureFormat::Rgba16Float
            || !texture
                .usage()
                .contains(TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING)
        {
            return Err(RunnerError::Unsupported(
                "mips require a 2D rgba16float texture with sampled and storage usages",
            ));
        }
        let module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("mip reduction"),
            source: ShaderSource::Wgsl(loft_std::bundle::MIP_2D.into()),
        });
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: None,
            layout: None,
            module: &module,
            entry_point: Some("reduce"),
            compilation_options: Default::default(),
            cache: None,
        });
        let layout = pipeline.get_bind_group_layout(0);
        let view = |level| {
            texture.create_view(&TextureViewDescriptor {
                base_mip_level: level,
                mip_level_count: Some(1),
                ..Default::default()
            })
        };
        let levels = (1..texture.mip_level_count())
            .map(|level| {
                let entries = [view(level - 1), view(level)];
                let group = device.create_bind_group(&BindGroupDescriptor {
                    label: None,
                    layout: &layout,
                    entries: &[
                        BindGroupEntry {
                            binding: 0,
                            resource: BindingResource::TextureView(&entries[0]),
                        },
                        BindGroupEntry {
                            binding: 1,
                            resource: BindingResource::TextureView(&entries[1]),
                        },
                    ],
                });
                (
                    group,
                    (texture.width() >> level).max(1).div_ceil(8),
                    (texture.height() >> level).max(1).div_ceil(8),
                )
            })
            .collect();
        *cached = Some(Self {
            epoch: table.epoch,
            pipeline,
            levels,
        });
        Ok(())
    }

    pub fn record(&self, encoder: &mut CommandEncoder) {
        for (group, x, y) in &self.levels {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.dispatch_workgroups(*x, *y, 1);
        }
    }
}
