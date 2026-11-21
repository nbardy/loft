use crate::error::*;
use loft_ir::PickResult;
use loft_std::pick::{decode, shader};
use wgpu::util::DeviceExt;
pub struct Source {
    vis: wgpu::Texture,
    depth: wgpu::Texture,
    inv_vp: [[f32; 4]; 4],
    pipeline: wgpu::ComputePipeline,
}

impl Source {
    /// Publish the textures and column-major inverse view-projection from the frame to pick.
    pub fn publish(
        source: &mut Option<Self>, device: &wgpu::Device,
        visibility: &wgpu::Texture,
        depth: &wgpu::Texture,
        inv_vp: [[f32; 4]; 4],
    ) -> Result<()> {
        use wgpu::{TextureDimension::D2, TextureFormat as F, TextureUsages as U};
        if visibility.format() != F::Rgba16Uint
            || !depth.format().has_depth_aspect()
            || visibility.dimension() != D2
            || depth.dimension() != D2
            || visibility.size() != depth.size()
            || depth.depth_or_array_layers() != 1
            || visibility.sample_count() != depth.sample_count()
            || !visibility.usage().contains(U::TEXTURE_BINDING)
            || !depth.usage().contains(U::TEXTURE_BINDING)
            || inv_vp.iter().flatten().any(|v| !v.is_finite())
        {
            return Err(RunnerError::AbiMismatch);
        }
        let pipeline = source
            .as_ref()
            .filter(|s| s.depth.sample_count() == depth.sample_count())
            .map(|s| s.pipeline.clone())
            .unwrap_or_else(|| {
                let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("sample zero pick"),
                    source: wgpu::ShaderSource::Wgsl(shader(depth.sample_count() > 1).into()),
                });
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("sample zero pick"),
                    layout: None,
                    module: &module,
                    entry_point: Some("read_pixel"),
                    compilation_options: Default::default(),
                    cache: None,
                })
            });
        *source = Some(Source {
            vis: visibility.clone(),
            depth: depth.clone(),
            inv_vp,
            pipeline,
        });
        Ok(())
    }

    pub fn pick_async(
        &self, device: &wgpu::Device, queue: &wgpu::Queue,
        px: [u32; 2],
        dispatch: impl FnOnce(&mut wgpu::CommandEncoder, wgpu::ComputePipeline, wgpu::BindGroup),
        cb: Box<dyn FnOnce(Result<PickResult>) + Send>,
    ) -> Result<()> {
        let source = self;
        let size = [source.vis.width(), source.vis.height()];
        if px[0] >= size[0] || px[1] >= size[1] {
            return Err(RunnerError::Capacity);
        }
        use wgpu::BufferUsages as U;
        let coord = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: px.map(u32::to_le_bytes).as_flattened(),
            usage: U::UNIFORM,
        });
        let output = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: 12, usage: U::STORAGE | U::COPY_SRC, mapped_at_creation: false });
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("single pixel staging"),
            size: u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT),
            usage: U::MAP_READ | U::COPY_DST,
            mapped_at_creation: false,
        });
        let vis = source.vis.create_view(&Default::default());
        let depth = source.depth.create_view(&wgpu::TextureViewDescriptor {
            aspect: wgpu::TextureAspect::DepthOnly,
            ..Default::default()
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &source.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&vis),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&depth),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: coord.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: output.as_entire_binding(),
                },
            ],
        });
        let mut enc = device.create_command_encoder(&Default::default());
        dispatch(&mut enc, source.pipeline.clone(), bind);
        enc.copy_buffer_to_buffer(&output, 0, &staging, 0, Some(12));
        let pixels = staging.clone();
        let inv_vp = source.inv_vp;
        enc.map_buffer_on_submit(&staging, wgpu::MapMode::Read, ..12, move |mapped| {
            let result = if mapped.is_ok() {
                let bytes = pixels.get_mapped_range(..12);
                let result = bytes.as_ref().ok().map(|b| {
                    let w = |i| u32::from_le_bytes(b[i..i + 4].try_into().unwrap());
                    decode([w(0), w(4), w(8)], px, size, inv_vp)
                });
                drop(bytes);
                pixels.unmap();
                result
            } else {
                None
            };
            cb(result.ok_or(RunnerError::Readback));
        });
        queue.submit([enc.finish()]);
        Ok(())
    }

}
