//! Match each declared slot to the resource type used by this entry point.
use crate::error::*;
use loft_ir::*;
use naga::{AddressSpace, ImageClass, ImageDimension, StorageAccess, TypeInner};
use std::collections::BTreeMap;
use wgpu::{BindGroupLayoutEntry as Entry, BindingType as Binding};

pub fn reflect(
    ir: &naga::Module,
    usage: &naga::valid::FunctionInfo,
    pass: &PassDecl,
) -> Result<BTreeMap<(u32, u32), Entry>> {
    let mut entries = BTreeMap::new();
    for (handle, var) in ir.global_variables.iter() {
        let Some(binding) = var.binding.as_ref() else {
            continue;
        };
        if usage[handle].is_empty() {
            continue;
        }
        let address = (binding.group, binding.binding);
        let mut entry = match var.space {
            AddressSpace::Uniform => slot(binding.binding, wgpu::BufferBindingType::Uniform),
            AddressSpace::Storage { access } => slot(
                binding.binding,
                wgpu::BufferBindingType::Storage {
                    read_only: access == StorageAccess::LOAD,
                },
            ),
            AddressSpace::Handle => {
                let (ty, count) = match ir.types[var.ty].inner {
                    TypeInner::BindingArray {
                        base,
                        size: naga::ArraySize::Constant(count),
                    } => (base, Some(count)),
                    TypeInner::BindingArray { .. } => {
                        return Err(RunnerError::Unsupported("runtime binding array length"));
                    }
                    _ => (var.ty, None),
                };
                let ty = match ir.types[ty].inner {
                    TypeInner::AccelerationStructure { vertex_return } => Binding::AccelerationStructure { vertex_return },
                    TypeInner::Sampler { comparison } => Binding::Sampler(if comparison {
                        wgpu::SamplerBindingType::Comparison
                    } else {
                        wgpu::SamplerBindingType::Filtering
                    }),
                    TypeInner::Image { dim, arrayed, class } => {
                        let view_dimension = match (dim, arrayed) {
                            (ImageDimension::D1, false) => wgpu::TextureViewDimension::D1,
                            (ImageDimension::D2, false) => wgpu::TextureViewDimension::D2,
                            (ImageDimension::D2, true) => wgpu::TextureViewDimension::D2Array,
                            (ImageDimension::D3, false) => wgpu::TextureViewDimension::D3,
                            (ImageDimension::Cube, false) => wgpu::TextureViewDimension::Cube,
                            (ImageDimension::Cube, true) => wgpu::TextureViewDimension::CubeArray,
                            _ => return Err(RunnerError::AbiMismatch),
                        };
                        match class {
                            ImageClass::Sampled { kind, multi } => {
                                let sample_type = match kind {
                                    naga::ScalarKind::Float => wgpu::TextureSampleType::Float {
                                        filterable: usage.sampling_set.iter().any(|s| s.image == handle),
                                    },
                                    naga::ScalarKind::Sint => wgpu::TextureSampleType::Sint,
                                    naga::ScalarKind::Uint => wgpu::TextureSampleType::Uint,
                                    _ => return Err(RunnerError::AbiMismatch),
                                };
                                Binding::Texture {
                                    sample_type,
                                    view_dimension,
                                    multisampled: multi,
                                }
                            }
                            ImageClass::Depth { multi } => Binding::Texture {
                                sample_type: wgpu::TextureSampleType::Depth,
                                view_dimension,
                                multisampled: multi,
                            },
                            ImageClass::Storage { format, access } => Binding::StorageTexture {
                                access: if access.contains(StorageAccess::ATOMIC) {
                                    wgpu::StorageTextureAccess::Atomic
                                } else if access == StorageAccess::LOAD {
                                    wgpu::StorageTextureAccess::ReadOnly
                                } else if access == StorageAccess::STORE {
                                    wgpu::StorageTextureAccess::WriteOnly
                                } else {
                                    wgpu::StorageTextureAccess::ReadWrite
                                },
                                format: wgpu_naga_bridge::map_storage_format_from_naga(format),
                                view_dimension,
                            },
                            ImageClass::External => {
                                return Err(RunnerError::Unsupported("external video texture"));
                            }
                        }
                    }
                    _ => return Err(RunnerError::AbiMismatch),
                };
                Entry {
                    binding: binding.binding,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty,
                    count,
                }
            }
            _ => return Err(RunnerError::AbiMismatch),
        };
        use BindAccess::*;
        use wgpu::BufferBindingType::{Storage, Uniform};
        let access = match entry.ty {
            Binding::Buffer { ty: Uniform, .. } if address == (0, 0) => None,
            Binding::Buffer {
                ty: Storage { read_only: true },
                ..
            } if matches!(address, (0, 1 | 2)) => None,
            Binding::Buffer {
                ty: Storage { read_only: true },
                ..
            } => Some(ReadStorage),
            Binding::Buffer {
                ty: Storage { read_only: false },
                ..
            } => Some(ReadWriteStorage),
            Binding::Texture { .. } if entry.count.is_some() => Some(BindlessTexArray),
            Binding::Texture { .. } => Some(SampledTex),
            Binding::StorageTexture { .. } => Some(StorageTex),
            Binding::Sampler(_) => Some(Sampler),
            Binding::AccelerationStructure { .. } => Some(AccelerationStructure),
            _ => return Err(RunnerError::AbiMismatch),
        };
        if let Some(access) = access
            && !pass
                .bindings
                .iter()
                .any(|s| (u32::from(s.group), s.slot) == address && s.access == access)
        {
            return Err(RunnerError::AbiMismatch);
        }
        if binding.group != 0
            && let Binding::Buffer { min_binding_size, .. } = &mut entry.ty
        {
            *min_binding_size = wgpu::BufferSize::new(u64::from(ir.types[var.ty].inner.size(ir.to_ctx())));
        }
        entries.insert(address, entry);
    }
    Ok(entries)
}
pub fn slot(binding: u32, ty: wgpu::BufferBindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

pub fn merge_bindings(
    reflected: &mut BTreeMap<(u32, u32), wgpu::BindGroupLayoutEntry>,
    stage: BTreeMap<(u32, u32), wgpu::BindGroupLayoutEntry>,
    visibility: wgpu::ShaderStages,
) -> Result<()> {
    for (address, mut entry) in stage {
        entry.visibility = visibility;
        let merged = reflected.entry(address).or_insert(entry);
        if merged.ty != entry.ty || merged.count != entry.count {
            return Err(RunnerError::AbiMismatch);
        }
        merged.visibility |= visibility;
    }
    Ok(())
}

pub fn layout(device: &wgpu::Device, entries: &[Entry]) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries })
}

pub fn group(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, entries: &[wgpu::BindGroupEntry<'_>]) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout, entries })
}

pub(crate) fn validator(features: wgpu::Features) -> naga::valid::Validator {
    wgpu_naga_bridge::create_validator(
        features,
        wgpu::DownlevelFlags::all(),
        naga::valid::ValidationFlags::all(),
    )
}
