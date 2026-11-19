//! Match each declared slot to the resource type used by this entry point.
use super::*;
use naga::{AddressSpace, ImageClass, ImageDimension, StorageAccess, TypeInner};
use wgpu::{BindGroupLayoutEntry as Entry, BindingType as Binding};

pub(super) fn reflect(
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
        let entry = match var.space {
            AddressSpace::Uniform => {
                super::encode::slot(binding.binding, wgpu::BufferBindingType::Uniform)
            }
            AddressSpace::Storage { access } => super::encode::slot(
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
                    TypeInner::Sampler { comparison } => Binding::Sampler(if comparison {
                        wgpu::SamplerBindingType::Comparison
                    } else {
                        wgpu::SamplerBindingType::Filtering
                    }),
                    TypeInner::Image {
                        dim,
                        arrayed,
                        class,
                    } => {
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
                                        filterable: usage
                                            .sampling_set
                                            .iter()
                                            .any(|s| s.image == handle),
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
        entries.insert(address, entry);
    }
    Ok(entries)
}
