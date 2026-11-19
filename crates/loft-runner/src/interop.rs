//! Views of host-owned resources on the attached device.
use super::*;
use wgpu::{BindingType, BufferUsages as B, TextureUsages as T, TextureViewDimension as V};

pub(super) fn stale(stamp: Option<u64>, epochs: impl IntoIterator<Item = u64>) -> bool {
    stamp.is_none_or(|s| epochs.into_iter().any(|e| e > s))
}

fn next_epoch(epoch: u64) -> Result<u64> {
    epoch.checked_add(1).ok_or(RunnerError::Capacity)
}

fn check_buffer(size: u64, usage: B, min: u64, required: B) -> Result<()> {
    if size < min || !usage.contains(required) {
        return Err(RunnerError::AbiMismatch);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct TextureShape {
    size: wgpu::Extent3d,
    dim: wgpu::TextureDimension,
    format: wgpu::TextureFormat,
    samples: u32,
    mips: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct TextureContract(TextureShape, T);

impl From<&wgpu::TextureDescriptor<'_>> for TextureContract {
    fn from(desc: &wgpu::TextureDescriptor<'_>) -> Self {
        Self(TextureShape {
            size: desc.size,
            dim: desc.dimension,
            format: desc.format,
            samples: desc.sample_count,
            mips: desc.mip_level_count,
        }, desc.usage)
    }
}

impl TextureShape {
    fn from_handle(tex: &wgpu::Texture) -> Self {
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
                format,
                view_dimension,
                ..
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
            (V::CubeArray, D::D2) => {
                self.size.width == self.size.height && layers > 0 && layers.is_multiple_of(6)
            }
            _ => false,
        };
        if !fits || !usage.contains(required) {
            return Err(RunnerError::AbiMismatch);
        }
        Ok(())
    }
}

pub(super) fn texture_view(tex: &wgpu::Texture, binding: BindingType) -> Result<wgpu::TextureView> {
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
        BindingType::StorageTexture { view_dimension, .. } => {
            (view_dimension, T::STORAGE_BINDING, Some(1), false)
        }
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

fn check_texture(
    compute: &encode::ComputeCache,
    table: TableId,
    shape: TextureShape,
    usage: T,
    features: wgpu::Features,
) -> Result<()> {
    for binding in compute.resource_bindings(table) {
        if binding.count.is_some() {
            return Err(RunnerError::Unsupported("texture binding arrays"));
        }
        shape.check(binding.ty, usage, features)?;
    }
    Ok(())
}

impl Runner {
    pub fn bind_external_buffer(
        &mut self,
        inst: InstanceId,
        table: TableId,
        buf: Arc<wgpu::Buffer>,
    ) -> Result<()> {
        let limits = self.device.limits();
        let inst = self.instance(inst)?;
        let t = inst
            .tables
            .get_mut(&table)
            .ok_or(RunnerError::UnknownResource)?;
        if t.texture_only {
            return Err(RunnerError::AbiMismatch);
        }
        let mut required = B::STORAGE;
        if matches!(t.decl.lifetime, TableLifetime::DynamicArena { .. }) {
            required |= B::COPY_SRC | B::COPY_DST;
        }
        if inst.decl.state.as_ref().is_some_and(|s| s.table == table) {
            required |= B::COPY_SRC | B::COPY_DST;
        }
        for p in &inst.decl.passes {
            match &p.run {
                PassRun::Compute {
                    dispatch: DispatchExtent::Indirect { buf, .. },
                    ..
                } if *buf == table => required |= B::INDIRECT,
                PassRun::RenderBundle { draws, .. } => {
                    for draw in draws {
                        if draw.args == table {
                            required |= B::INDIRECT;
                        }
                        if draw.vertex == Some(table) {
                            required |= B::VERTEX;
                        }
                        if draw.index == Some(table) {
                            required |= B::INDEX;
                        }
                    }
                }
                _ => {}
            }
        }
        let min = (u64::from(t.decl.capacity) * u64::from(t.decl.stride_bytes)).max(4);
        let mut storage = false;
        let mut binding_min = 0;
        for bind in inst.compute.resource_bindings(table) {
            let BindingType::Buffer {
                min_binding_size, ..
            } = bind.ty
            else {
                return Err(RunnerError::AbiMismatch);
            };
            storage = true;
            binding_min = binding_min.max(min_binding_size.map_or(0, std::num::NonZeroU64::get));
        }
        if storage
            && (buf.size() > limits.max_storage_buffer_binding_size
                || !buf.size().is_multiple_of(4))
        {
            return Err(RunnerError::AbiMismatch);
        }
        check_buffer(buf.size(), buf.usage(), min.max(binding_min), required)?;
        let epoch = next_epoch(inst.resource_epoch)?;
        t.buffer = Some(buf);
        if t.decl.lifetime == TableLifetime::External {
            t.active = t.decl.capacity;
        }
        t.epoch = epoch;
        inst.resource_epoch = epoch;
        Ok(())
    }

    /// Set the asset contract before the first bind. The descriptor comes from
    /// the host's asset declaration because TableDecl carries storage layout only.
    pub fn declare_external_texture(
        &mut self, id: InstanceId, table: TableId, desc: &wgpu::TextureDescriptor<'_>,
    ) -> Result<()> {
        let features = self.device.features();
        let inst = self.instance(id)?;
        let t = inst.tables.get_mut(&table).ok_or(RunnerError::UnknownResource)?;
        let contract = TextureContract::from(desc);
        if !t.texture_only || t.decl.lifetime != TableLifetime::External
            || t.texture_contract.is_some_and(|existing| existing != contract)
        {
            return Err(RunnerError::AbiMismatch);
        }
        check_texture(&inst.compute, table, contract.0, contract.1, features)?;
        t.texture_contract = Some(contract);
        Ok(())
    }

    /// Allocate backing for a texture table using a host-supplied descriptor.
    pub fn allocate_texture(
        &mut self, id: InstanceId, table: TableId, desc: &wgpu::TextureDescriptor<'_>,
    ) -> Result<()> {
        let device = self.device.clone();
        let inst = self.instance(id)?;
        let t = inst.tables.get_mut(&table).ok_or(RunnerError::UnknownResource)?;
        let contract = TextureContract::from(desc);
        if t.decl.lifetime == TableLifetime::External || !t.texture_only || t.texture.is_some() {
            return Err(RunnerError::AbiMismatch);
        }
        check_texture(&inst.compute, table, contract.0, contract.1, device.features())?;
        let epoch = next_epoch(inst.resource_epoch)?;
        t.texture = Some(Arc::new(device.create_texture(desc)));
        t.texture_contract = Some(contract);
        t.epoch = epoch;
        inst.resource_epoch = epoch;
        Ok(())
    }

    pub fn bind_external_texture(
        &mut self, id: InstanceId, table: TableId, tex: Arc<wgpu::Texture>,
    ) -> Result<()> {
        let features = self.device.features();
        let inst = self.instance(id)?;
        let t = inst.tables.get_mut(&table).ok_or(RunnerError::UnknownResource)?;
        let contract = t.texture_contract.ok_or(RunnerError::AbiMismatch)?;
        let shape = TextureShape::from_handle(&tex);
        if shape != contract.0 || !tex.usage().contains(contract.1) {
            return Err(RunnerError::AbiMismatch);
        }
        check_texture(&inst.compute, table, shape, tex.usage(), features)?;
        let epoch = next_epoch(inst.resource_epoch)?;
        t.texture = Some(tex);
        t.epoch = epoch;
        inst.resource_epoch = epoch;
        Ok(())
    }

    pub fn export_buffer(&self, inst: InstanceId, table: TableId) -> Option<Arc<wgpu::Buffer>> {
        self.instances
            .get(inst.0 as usize)?
            .tables
            .get(&table)?
            .buffer
            .clone()
    }

    pub fn export_texture(&self, inst: InstanceId, table: TableId) -> Option<Arc<wgpu::Texture>> {
        self.instances
            .get(inst.0 as usize)?
            .tables
            .get(&table)?
            .texture
            .clone()
    }
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
        let check =
            |shape: TextureShape, usage| shape.check(binding, usage, wgpu::Features::empty());
        assert!(check(shape, T::STORAGE_BINDING).is_ok());
        assert_eq!(
            check(shape, T::TEXTURE_BINDING),
            Err(RunnerError::AbiMismatch)
        );
        assert_eq!(
            check(
                TextureShape {
                    samples: 4,
                    ..shape
                },
                T::STORAGE_BINDING
            ),
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
