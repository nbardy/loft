//! Classified resource handles. Host attachment roles are resolved from the render declarations.
use super::*;
use wgpu::{BindingType, TextureUsages as T};

pub(super) enum Resource {
    Buffer(Option<Arc<wgpu::Buffer>>),
    Texture {
        handle: Option<Arc<wgpu::Texture>>,
        contract: Option<interop::TextureContract>,
    },
    Attachment(wgpu::TextureView, TextureDecl),
    Sampler(wgpu::Sampler),
    HostAttachment,
}

pub(super) fn texture(d: &wgpu::Device, t: TextureDecl) -> Result<Resource> {
    let [width, height, layers] = t.extent;
    let format = match t.format {
        ImageFormat::Rg32Uint => wgpu::TextureFormat::Rg32Uint,
        ImageFormat::Rgba16Float => wgpu::TextureFormat::Rgba16Float,
        ImageFormat::Depth32Float => wgpu::TextureFormat::Depth32Float,
    };
    if width == 0
        || height == 0
        || width.max(height) > d.limits().max_texture_dimension_2d
        || layers != 1
        || !matches!(t.samples, 1 | 4)
        || (t.samples == 4 && matches!(t.format, ImageFormat::Rgba16Float))
        || t.clear
            .is_some_and(|clear| clear.iter().any(|v| !v.is_finite()))
    {
        return Err(RunnerError::AbiMismatch);
    }
    let mut usage = T::TEXTURE_BINDING | T::RENDER_ATTACHMENT;
    if matches!(t.format, ImageFormat::Rgba16Float) {
        usage |= T::STORAGE_BINDING;
    }
    let texture = d.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: layers,
        },
        mip_level_count: 1,
        sample_count: u32::from(t.samples),
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    });
    Ok(Resource::Attachment(
        texture.create_view(&Default::default()),
        t,
    ))
}

pub(super) fn sampler(d: &wgpu::Device) -> Resource {
    Resource::Sampler(d.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        compare: Some(wgpu::CompareFunction::LessEqual),
        ..Default::default()
    }))
}

pub(super) enum Bound<'a> {
    Buffer(&'a wgpu::Buffer),
    Image(wgpu::TextureView),
    Sampler(&'a wgpu::Sampler),
}
impl<'a> Bound<'a> {
    pub fn binding(&self) -> wgpu::BindingResource<'_> {
        match self {
            Self::Buffer(buffer) => buffer.as_entire_binding(),
            Self::Image(view) => wgpu::BindingResource::TextureView(view),
            Self::Sampler(sampler) => wgpu::BindingResource::Sampler(sampler),
        }
    }
}

impl Resource {
    pub fn bind(&self, slot: &wgpu::BindGroupLayoutEntry) -> Result<Bound<'_>> {
        if slot.count.is_some() {
            return Err(RunnerError::Unsupported("texture binding arrays"));
        }
        match (self, slot.ty) {
            (Self::Buffer(Some(buf)), BindingType::Buffer { .. }) => Ok(Bound::Buffer(buf)),
            (
                Self::Texture {
                    handle: Some(tex), ..
                },
                BindingType::Texture { .. } | BindingType::StorageTexture { .. },
            ) => Ok(Bound::Image(interop::texture_view(tex, slot.ty)?)),
            (
                Self::Attachment(view, _),
                BindingType::Texture { .. } | BindingType::StorageTexture { .. },
            ) => Ok(Bound::Image(view.clone())),
            (
                Self::Sampler(sampler),
                BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
            ) => Ok(Bound::Sampler(sampler)),
            _ => Err(RunnerError::UnknownResource),
        }
    }
}
