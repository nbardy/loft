//! Materialize the compiler's texture pass declarations for the native executor.
use super::*;
use loft_ir::{DispatchExtent, PassId, PassRun, TableId};
use loft_passes::bake_tex::{Bake, Clock as DeclClock, TexDim};
use super::mips::{Clock as BakeClock, Dimension, Mips, Pipelines};

pub(super) struct TextureBake {
    pub(super) name: String,
    pub(super) texture: wgpu::Texture,
    pub(super) clock: BakeClock,
    pub(super) mips: Option<Mips>,
    pipeline: wgpu::ComputePipeline,
    group: wgpu::BindGroup,
    empty: wgpu::BindGroup,
    work: [u32; 3],
}

impl TextureBake {
    pub(super) fn timestamp_count(&self) -> u32 {
        if self.mips.is_some() { 4 } else { 2 }
    }

    pub(super) fn prepare(g: &Gpu, bake: &Bake, modules: &[wgpu::ShaderModule], fixed: &wgpu::BindGroupLayout, cache: &mut Pipelines) -> Result<Self, String> {
        let format = match bake.format {
            naga::StorageFormat::R8Unorm => wgpu::TextureFormat::R8Unorm,
            naga::StorageFormat::R16Float => wgpu::TextureFormat::R16Float,
            naga::StorageFormat::R32Float => wgpu::TextureFormat::R32Float,
            naga::StorageFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
            naga::StorageFormat::Rgba16Float => wgpu::TextureFormat::Rgba16Float,
            _ => return Err(format!("{}: unsupported bake format", bake.name)),
        };
        if !g.bake_formats.contains(&format) {
            return Err(format!("{}: adapter does not support writing {format:?} storage textures", bake.name));
        }
        let dim = match bake.dim {
            TexDim::D2(..) => Dimension::D2,
            TexDim::D3(..) => Dimension::D3,
            TexDim::Cube(..) => Dimension::Cube,
        };
        let [width, height, depth_or_array_layers] = bake.size();
        let d = &g.device;
        let limits = d.limits();
        if bake.workgroup().into_iter().product::<u32>() > limits.max_compute_invocations_per_workgroup {
            return Err(format!("{}: bake workgroup exceeds the attached device invocation limit", bake.name));
        }
        let (dimension, fits) = if dim == Dimension::D3 {
            (wgpu::TextureDimension::D3, [width, height, depth_or_array_layers].into_iter().all(|n| n <= limits.max_texture_dimension_3d))
        } else {
            (wgpu::TextureDimension::D2, width.max(height) <= limits.max_texture_dimension_2d && depth_or_array_layers <= limits.max_texture_array_layers)
        };
        if !fits { return Err(format!("{}: bake texture exceeds device limits", bake.name)); }
        let texture = d.create_texture(&wgpu::TextureDescriptor {
            label: Some(&bake.name), size: wgpu::Extent3d { width, height, depth_or_array_layers },
            mip_level_count: bake.mip_count(), sample_count: 1, dimension, format,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(dim.storage_view()), mip_level_count: Some(1), ..Default::default()
        });
        let empty_layout = layout(d, "empty", &[]);
        let empty = bind(d, &empty_layout, &[]);
        let bgl = layout(d, "bake", &[(CS, wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly, format, view_dimension: dim.storage_view()
        })]);
        let group = bind(d, &bgl, &[wgpu::BindingResource::TextureView(&view)]);
        let table = TableId(bake.module as u32);
        let passes = bake.passes(table, PassId(0));
        let PassRun::Compute { module, entry, dispatch: DispatchExtent::Fixed(work) } = &passes[0].run else { unreachable!("bake lowers to a fixed dispatch") };
        let shader = modules.get(module.0 as usize).ok_or("missing bake shader")?;
        let pipeline = compute(d, &bake.name, &[fixed, &empty_layout, &empty_layout, &bgl], shader, entry, None);
        let mips = match passes.get(1).map(|p| &p.run) {
            Some(PassRun::GenerateMips { texture: id }) if *id == table => Some(cache.prepare(d, &texture, dim)?),
            None => None,
            _ => unreachable!("bake mips target the declared texture"),
        };
        let clock = match &bake.clock {
            DeclClock::Load => BakeClock::Load { done: false },
            DeclClock::Dirty(words) => BakeClock::Dirty { words: words.clone(), last: None },
        };
        Ok(Self { name: bake.name.clone(), texture, clock, mips, pipeline, group, empty, work: *work })
    }

    pub(super) fn encode(&self, g0: &wgpu::BindGroup, enc: &mut wgpu::CommandEncoder, timestamps: Option<(&wgpu::QuerySet, u32)>) -> u32 {
        {
            let timestamp_writes = timestamps.map(|(query_set, start)| wgpu::ComputePassTimestampWrites {
                query_set, beginning_of_pass_write_index: Some(start), end_of_pass_write_index: Some(start + 1),
            });
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some(&self.name), timestamp_writes });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, g0, &[]);
            pass.set_bind_group(1, &self.empty, &[]);
            pass.set_bind_group(2, &self.empty, &[]);
            pass.set_bind_group(3, &self.group, &[]);
            let [x, y, z] = self.work;
            pass.dispatch_workgroups(x, y, z);
        }
        1 + self.mips.as_ref().map_or(0, |mips| mips.encode(enc, timestamps.map(|(set, start)| (set, start + 2))))
    }
}
