use super::*;
use crate::execution::{Clock, Dimension, Execution};
use loft_std::{bundle::passes, interop::{TextureBinding, TextureContract, check_texture}};
use std::collections::BTreeSet;
use wgpu::{BindingType, TextureDimension, TextureUsages as T, TextureViewDimension as V};

pub(super) struct Initializer {
    pub name: String,
    pub clock: mips::Clock,
    pub passes: Vec<PassId>,
}
#[derive(Default)]
pub(super) struct Initializers {
    pub clocks: Vec<Initializer>,
    pub images: BTreeMap<String, TableId>,
    pub dimensions: BTreeMap<TableId, mips::Dimension>,
}

impl Initializers {
    pub fn prepare(
        exec: Execution, decl: &Bundle, tables: &mut BTreeMap<TableId, Table>,
        compute: &encode::ComputeCache, device: &wgpu::Device,
    ) -> Result<Self> {
        let mut seen = BTreeSet::new();
        let mut out = Self::default();
        for init in exec.initializers {
            if init.passes.is_empty() || init.passes.iter().any(|id| !seen.insert(*id)
                || !decl.passes.iter().any(|p| p.id == *id
                    && matches!(p.run, PassRun::Compute { .. } | PassRun::GenerateMips { .. }))) {
                return Err(RunnerError::AbiMismatch);
            }
            let clock = match init.clock {
                Clock::Load => mips::Clock::Load { done: false },
                Clock::Dirty(words) => {
                    if words.iter().any(|&word| word >= compute.params.bytes().len() / 4) {
                        return Err(RunnerError::AbiMismatch);
                    }
                    mips::Clock::Dirty { words, last: None }
                }
            };
            if let Some(image) = init.image {
                let table = tables.get_mut(&image.table).ok_or(RunnerError::UnknownResource)?;
                if !matches!(table.resource, Resource::Texture(TextureBinding::Vacant))
                    || out.images.insert(init.name.clone(), image.table).is_some() {
                    return Err(RunnerError::AbiMismatch);
                }
                let slots = compute.resource_bindings(image.table).collect::<Vec<_>>();
                let format = slots.iter().find_map(|slot| match slot.ty {
                    BindingType::StorageTexture { format, .. } => Some(format),
                    _ => None,
                }).ok_or(RunnerError::AbiMismatch)?;
                let [width, height, depth_or_array_layers] = image.extent;
                let limits = device.limits();
                let (dimension, limit, layers_ok) = match image.dimension {
                    Dimension::Plane => (TextureDimension::D2, limits.max_texture_dimension_2d, depth_or_array_layers == 1),
                    Dimension::Volume => (TextureDimension::D3, limits.max_texture_dimension_3d, depth_or_array_layers <= limits.max_texture_dimension_3d),
                    Dimension::Cube => (TextureDimension::D2, limits.max_texture_dimension_2d, depth_or_array_layers == 6 && width == height),
                };
                let dimension_for_mips = match image.dimension {
                    Dimension::Plane => mips::Dimension::D2,
                    Dimension::Volume => mips::Dimension::D3,
                    Dimension::Cube => mips::Dimension::Cube,
                };
                out.dimensions.insert(image.table, dimension_for_mips);
                let max_side = if dimension == TextureDimension::D3 { width.max(height).max(depth_or_array_layers) } else { width.max(height) };
                if image.extent.contains(&0) || max_side > limit || !layers_ok
                    || image.levels == 0 || image.levels > u32::BITS - max_side.leading_zeros() {
                    return Err(RunnerError::Capacity);
                }
                let has_mips = passes(&decl.passes).any(|p| init.passes.contains(&p.id)
                    && matches!(p.run, PassRun::GenerateMips { texture } if texture == image.table));
                if has_mips != (image.levels > 1) { return Err(RunnerError::AbiMismatch); }
                if matches!(image.dimension, Dimension::Cube) && !slots.iter().any(|slot|
                    matches!(slot.ty, BindingType::StorageTexture { view_dimension: V::D2Array, .. })) {
                    return Err(RunnerError::AbiMismatch);
                }
                let desc = wgpu::TextureDescriptor {
                    label: Some(&init.name), size: wgpu::Extent3d { width, height, depth_or_array_layers },
                    mip_level_count: image.levels, sample_count: 1, dimension, format,
                    usage: T::STORAGE_BINDING | T::TEXTURE_BINDING | T::COPY_SRC, view_formats: &[],
                };
                let contract = TextureContract::from(&desc);
                check_texture(slots.into_iter(), contract.0, contract.1, device.features())?;
                table.resource = Resource::Texture(TextureBinding::Bound(contract, Arc::new(device.create_texture(&desc))));
            }
            out.clocks.push(Initializer { name: init.name, clock, passes: init.passes });
        }
        Ok(out)
    }
}
