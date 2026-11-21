use crate::error::*;
use loft_ir::*;
use wgpu::{BindingType, BufferUsages as B, TextureUsages as T, TextureViewDimension as V};
pub fn stale(stamp: Option<u64>, epochs: impl IntoIterator<Item = u64>) -> bool {
    stamp.is_none_or(|s| epochs.into_iter().any(|e| e > s))
}

pub fn next_epoch(epoch: u64) -> Result<u64> {
    epoch.checked_add(1).ok_or(RunnerError::Capacity)
}

pub fn check_buffer(size: u64, usage: B, min: u64, required: B) -> Result<()> {
    if size < min || !usage.contains(required) {
        return Err(RunnerError::AbiMismatch);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureShape {
    size: wgpu::Extent3d,
    dim: wgpu::TextureDimension,
    format: wgpu::TextureFormat,
    samples: u32,
    mips: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct TextureContract(pub TextureShape, pub T);

impl From<&wgpu::TextureDescriptor<'_>> for TextureContract {
    fn from(desc: &wgpu::TextureDescriptor<'_>) -> Self {
        Self(
            TextureShape {
                size: desc.size,
                dim: desc.dimension,
                format: desc.format,
                samples: desc.sample_count,
                mips: desc.mip_level_count,
            },
            desc.usage,
        )
    }
}

impl TextureShape {
    pub fn from_handle(tex: &wgpu::Texture) -> Self {
        Self {
            size: tex.size(),
            dim: tex.dimension(),
            format: tex.format(),
            samples: tex.sample_count(),
            mips: tex.mip_level_count(),
        }
    }

    fn check(self, binding: BindingType, usage: T, features: wgpu::Features) -> Result<()> {
        use wgpu::{TextureDimension as D, TextureSampleType as S};
        let (view, required) = match binding {
            BindingType::Texture {
                sample_type,
                view_dimension,
                multisampled,
            } => {
                let actual = self
                    .format
                    .sample_type(Some(wgpu::TextureAspect::All), Some(features))
                    .or_else(|| {
                        self.format
                            .sample_type(Some(wgpu::TextureAspect::DepthOnly), Some(features))
                    });
                if !(actual == Some(sample_type)
                    || matches!(
                        (sample_type, actual),
                        (S::Float { filterable: false }, Some(S::Float { .. }))
                    ))
                    || multisampled != (self.samples > 1)
                {
                    return Err(RunnerError::AbiMismatch);
                }
                (view_dimension, T::TEXTURE_BINDING)
            }
            BindingType::StorageTexture {
                format, view_dimension, ..
            } => {
                if format != self.format || self.samples != 1 {
                    return Err(RunnerError::AbiMismatch);
                }
                (view_dimension, T::STORAGE_BINDING)
            }
            _ => return Err(RunnerError::AbiMismatch),
        };
        let layers = self.size.depth_or_array_layers;
        let fits = match (view, self.dim) {
            (V::D1, D::D1) | (V::D2, D::D2) => layers == 1,
            (V::D2Array, D::D2) | (V::D3, D::D3) => layers > 0,
            (V::Cube, D::D2) => self.size.width == self.size.height && layers == 6,
            (V::CubeArray, D::D2) => self.size.width == self.size.height && layers > 0 && layers.is_multiple_of(6),
            _ => false,
        };
        if !fits || !usage.contains(required) {
            return Err(RunnerError::AbiMismatch);
        }
        Ok(())
    }
}

pub fn texture_view(tex: &wgpu::Texture, binding: BindingType) -> Result<wgpu::TextureView> {
    let (dimension, usage, mips, depth) = match binding {
        BindingType::Texture {
            view_dimension,
            sample_type,
            ..
        } => (
            view_dimension,
            T::TEXTURE_BINDING,
            None,
            sample_type == wgpu::TextureSampleType::Depth,
        ),
        BindingType::StorageTexture { view_dimension, .. } => (view_dimension, T::STORAGE_BINDING, Some(1), false),
        _ => return Err(RunnerError::AbiMismatch),
    };
    Ok(tex.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(dimension),
        usage: Some(usage),
        mip_level_count: mips,
        aspect: if depth && tex.format().has_stencil_aspect() {
            wgpu::TextureAspect::DepthOnly
        } else {
            wgpu::TextureAspect::All
        },
        ..Default::default()
    }))
}

pub fn storage_access(
    features: wgpu::Features, adapter: Option<&wgpu::Adapter>,
    format: wgpu::TextureFormat, access: wgpu::StorageTextureAccess,
) -> Result<()> {
    use wgpu::{StorageTextureAccess as A, TextureFormatFeatureFlags as F};
    let required = match access {
        A::WriteOnly => F::STORAGE_WRITE_ONLY,
        A::ReadOnly => F::STORAGE_READ_ONLY,
        A::ReadWrite => F::STORAGE_READ_WRITE,
        A::Atomic => F::STORAGE_ATOMIC,
    };
    let mut granted = format.guaranteed_format_features(features);
    if features.contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES)
        && let Some(adapter) = adapter
    {
        granted = adapter.get_texture_format_features(format);
    }
    if !granted.allowed_usages.contains(T::STORAGE_BINDING) || !granted.flags.contains(required)
        || (access == A::Atomic && !features.contains(wgpu::Features::TEXTURE_ATOMIC)) {
        return Err(RunnerError::TextureAccess { format: format!("{format:?}"), access: format!("{access:?}") });
    }
    Ok(())
}

pub fn check_texture<'a>(
    bindings: impl Iterator<Item = &'a wgpu::BindGroupLayoutEntry>,
    shape: TextureShape,
    usage: T,
    features: wgpu::Features,
) -> Result<()> {
    for binding in bindings {
        if binding.count.is_some() {
            return Err(RunnerError::Unsupported("texture binding arrays"));
        }
        shape.check(binding.ty, usage, features)?;
    }
    Ok(())
}

pub fn buffer_usage(bundle: &Bundle, table: TableId) -> B {
    let mut usage = B::COPY_SRC | B::COPY_DST;
    for pass in crate::bundle::passes(&bundle.passes) {
        if pass.bindings.iter().any(|b| b.resource == table) { usage |= B::STORAGE; }
        match &pass.run {
            PassRun::Compute { dispatch: DispatchExtent::Indirect { buf, .. }, .. }
            | PassRun::NativeCompute { dispatch: DispatchExtent::Indirect { buf, .. }, .. }
            | PassRun::HostCallback { dispatch: DispatchExtent::Indirect { buf, .. }, .. } if *buf == table => usage |= B::INDIRECT,
            PassRun::RenderBundle { draws, .. } => for draw in draws {
                if draw.args == table { usage |= B::INDIRECT; }
                if draw.vertex == Some(table) { usage |= B::VERTEX; }
                if draw.index == Some(table) { usage |= B::INDEX; }
            },
            _ => {}
        }
    }
    usage
}

pub fn logical_size(t: &TableDecl) -> u64 {
    (u64::from(t.capacity) * u64::from(t.stride_bytes)).max(4)
}

pub fn buffer_range(
    capacity: u64, usage: B, logical: u64, offset: u64, size: u64,
    slot: &wgpu::BindGroupLayoutEntry, limits: &wgpu::Limits,
) -> Result<wgpu::BufferSize> {
    let BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { .. }, min_binding_size, .. } = slot.ty
        else { return Err(RunnerError::AbiMismatch); };
    if !usage.contains(B::STORAGE) || !offset.is_multiple_of(u64::from(limits.min_storage_buffer_offset_alignment))
        || !size.is_multiple_of(4) || size < min_binding_size.map_or(4, wgpu::BufferSize::get)
        || size > limits.max_storage_buffer_binding_size
        || offset.checked_add(size).is_none_or(|end| end > logical.min(capacity))
    {
        return Err(RunnerError::AbiMismatch);
    }
    wgpu::BufferSize::new(size).ok_or(RunnerError::AbiMismatch)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l23a_interop_checks_and_affected_groups_only() {
        assert!(stale(None, [0]));
        assert!(stale(Some(3), [2, 4]));
        assert!(!stale(Some(3), [1, 3]));
        assert!(!stale(Some(3), []));
        assert!(check_buffer(64, B::STORAGE, 64, B::STORAGE).is_ok());
        assert_eq!(
            check_buffer(60, B::STORAGE, 64, B::STORAGE),
            Err(RunnerError::AbiMismatch)
        );
        assert_eq!(
            check_buffer(64, B::COPY_DST, 64, B::STORAGE),
            Err(RunnerError::AbiMismatch)
        );
        let shape = TextureShape {
            size: wgpu::Extent3d {
                width: 16,
                height: 16,
                depth_or_array_layers: 1,
            },
            dim: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            samples: 1,
            mips: 1,
        };
        let binding = BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format: shape.format,
            view_dimension: V::D2,
        };
        let check = |shape: TextureShape, usage| shape.check(binding, usage, wgpu::Features::empty());
        assert!(check(shape, T::STORAGE_BINDING).is_ok());
        assert_eq!(check(shape, T::TEXTURE_BINDING), Err(RunnerError::AbiMismatch));
        assert_eq!(
            check(TextureShape { samples: 4, ..shape }, T::STORAGE_BINDING),
            Err(RunnerError::AbiMismatch)
        );
        assert_eq!(
            check(
                TextureShape {
                    format: wgpu::TextureFormat::R8Unorm,
                    ..shape
                },
                T::STORAGE_BINDING
            ),
            Err(RunnerError::AbiMismatch)
        );
    }
}

/// A recorded draw retains its vertex and index handles even when all bind groups are current.
pub fn render_bundle_stale(draws: &[IndirectDraw], cached_epoch: u64, epoch: impl Fn(TableId) -> u64) -> bool {
    draws
        .iter()
        .flat_map(|draw| [draw.vertex, draw.index])
        .flatten()
        .any(|id| epoch(id) > cached_epoch)
}

use std::sync::Arc;
pub enum Resource {
    Buffer(Option<Arc<wgpu::Buffer>>),
    Texture(TextureBinding),
    Attachment(wgpu::TextureView, TextureDecl),
    Sampler(wgpu::Sampler),
    AccelerationStructure(wgpu::Tlas),
    HostAttachment,
}
pub enum TextureBinding {
    Vacant,
    Declared(TextureContract),
    Bound(TextureContract, Arc<wgpu::Texture>),
}
impl Resource {
    pub fn texture(&self) -> Option<Arc<wgpu::Texture>> {
        match self {
            Self::Texture(TextureBinding::Bound(_, t)) => Some(t.clone()),
            Self::Attachment(v, _) => Some(Arc::new(v.texture().clone())),
            Self::Buffer(_) | Self::Sampler(_) | Self::AccelerationStructure(_) | Self::HostAttachment | Self::Texture(_) => None,
        }
    }
    pub fn buffer(&self) -> Option<&Arc<wgpu::Buffer>> {
        match self {
            Self::Buffer(b) => b.as_ref(),
            Self::Texture(_) | Self::Attachment(..) | Self::Sampler(_) | Self::AccelerationStructure(_) | Self::HostAttachment => None,
        }
    }
    pub fn bytes(&self) -> Result<&wgpu::Buffer> {
        self.buffer().map(Arc::as_ref).ok_or(RunnerError::AbiMismatch)
    }
    pub fn attachment(&self) -> Option<(&wgpu::TextureView, TextureDecl)> {
        match self {
            Self::Attachment(v, t) => Some((v, *t)),
            Self::Buffer(_) | Self::Texture(_) | Self::Sampler(_) | Self::AccelerationStructure(_) | Self::HostAttachment => None,
        }
    }
    pub fn declare(&mut self, desc: &wgpu::TextureDescriptor<'_>) -> Result<()> {
        let expected = TextureContract::from(desc);
        match self {
            Self::Texture(binding @ TextureBinding::Vacant) => {
                *binding = TextureBinding::Declared(expected);
                Ok(())
            }
            Self::Texture(TextureBinding::Declared(old) | TextureBinding::Bound(old, _)) if *old == expected => {
                Ok(())
            }
            _ => Err(RunnerError::AbiMismatch),
        }
    }
    pub fn adopt(&mut self, tex: Arc<wgpu::Texture>) -> Result<()> {
        let Self::Texture(TextureBinding::Declared(c) | TextureBinding::Bound(c, _)) = self else {
            return Err(RunnerError::AbiMismatch);
        };
        if TextureShape::from_handle(&tex) != c.0 || !tex.usage().contains(c.1) {
            return Err(RunnerError::AbiMismatch);
        }
        *self = Self::Texture(TextureBinding::Bound(*c, tex));
        Ok(())
    }
}
pub enum Bound<'a> {
    Buffer(wgpu::BufferBinding<'a>),
    Image(wgpu::TextureView),
    Sampler(&'a wgpu::Sampler),
    AccelerationStructure(&'a wgpu::Tlas),
}
impl<'a> Bound<'a> {
    pub fn binding(&self) -> wgpu::BindingResource<'_> {
        match self {
            Self::Buffer(binding) => wgpu::BindingResource::Buffer(binding.clone()),
            Self::Image(view) => wgpu::BindingResource::TextureView(view),
            Self::Sampler(sampler) => wgpu::BindingResource::Sampler(sampler),
            Self::AccelerationStructure(tlas) => tlas.as_binding(),
        }
    }
}

impl Resource {
    pub fn bind(&self, slot: &wgpu::BindGroupLayoutEntry, logical: u64, range: (u64, u64), limits: &wgpu::Limits) -> Result<Bound<'_>> {
        if slot.count.is_some() {
            return Err(RunnerError::Unsupported("texture binding arrays"));
        }
        match (self, slot.ty) {
            (Self::Buffer(Some(buf)), BindingType::Buffer { .. }) => {
                let (offset, size) = range;
                let size = buffer_range(buf.size(), buf.usage(), logical, offset, size, slot, limits)?;
                Ok(Bound::Buffer(wgpu::BufferBinding { buffer: buf, offset, size: Some(size) }))
            }
            (
                Self::Texture(TextureBinding::Bound(_, tex)),
                BindingType::Texture { .. } | BindingType::StorageTexture { .. },
            ) => Ok(Bound::Image(texture_view(tex, slot.ty)?)),
            (Self::Attachment(view, _), BindingType::Texture { .. } | BindingType::StorageTexture { .. }) => {
                Ok(Bound::Image(view.clone()))
            }
            (Self::Sampler(sampler), BindingType::Sampler(_)) => {
                Ok(Bound::Sampler(sampler))
            }
            (Self::AccelerationStructure(tlas), BindingType::AccelerationStructure { vertex_return: false }) => {
                Ok(Bound::AccelerationStructure(tlas))
            }
            _ => Err(RunnerError::UnknownResource),
        }
    }
}
