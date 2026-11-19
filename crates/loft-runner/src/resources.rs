use super::*;
use wgpu::{Device, Sampler, TextureView, TextureUsages as Usage};

#[derive(Default)]
pub(super) struct Images {
    pub views: BTreeMap<TableId, (TextureView, TextureDecl)>,
    pub samplers: BTreeMap<TableId, Sampler>,
}

impl Images {
    pub fn new(d: &Device, b: &Bundle) -> Result<Self> {
        let mut images = Self::default();
        let external = |id| b.tables.iter().any(|t| t.id == id && t.lifetime == TableLifetime::External);
        for t in &b.resources.textures {
            let [width, height, layers] = t.extent;
            let format = match t.format {
                ImageFormat::Rg32Uint => wgpu::TextureFormat::Rg32Uint,
                ImageFormat::Rgba16Float => wgpu::TextureFormat::Rgba16Float,
                ImageFormat::Depth32Float => wgpu::TextureFormat::Depth32Float,
            };
            if !external(t.table) || images.views.contains_key(&t.table) || width == 0 || height == 0
                || width.max(height) > d.limits().max_texture_dimension_2d || layers != 1
                || !matches!(t.samples, 1 | 4) || (t.samples == 4 && matches!(t.format, ImageFormat::Rgba16Float))
                || t.clear.is_some_and(|c| c.iter().any(|v| !v.is_finite()))
            {
                return Err(RunnerError::AbiMismatch);
            }
            let mut usage = Usage::TEXTURE_BINDING;
            usage |= if matches!(t.format, ImageFormat::Rgba16Float) {
                Usage::STORAGE_BINDING | Usage::RENDER_ATTACHMENT
            } else {
                Usage::RENDER_ATTACHMENT
            };
            let texture = d.create_texture(&wgpu::TextureDescriptor {
                label: None, size: wgpu::Extent3d { width, height, depth_or_array_layers: layers },
                mip_level_count: 1, sample_count: u32::from(t.samples),
                dimension: wgpu::TextureDimension::D2, format, usage, view_formats: &[],
            });
            images.views.insert(t.table, (texture.create_view(&Default::default()), *t));
        }
        for &id in &b.resources.comparison_samplers {
            if !external(id) || images.views.contains_key(&id) || images.samplers.contains_key(&id) {
                return Err(RunnerError::AbiMismatch);
            }
            images.samplers.insert(id, d.create_sampler(&wgpu::SamplerDescriptor {
                mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear,
                compare: Some(wgpu::CompareFunction::LessEqual), ..Default::default()
            }));
        }
        Ok(images)
    }
}
