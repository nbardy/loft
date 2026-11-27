use super::*;
use crate::execution::{Clock, Dimension, Execution, Image, ImageStorage};
use loft_wgpu::{bundle::passes, interop::{TextureBinding, TextureContract, storage_access}};
use std::collections::BTreeSet;
use wgpu::{StorageTextureAccess, TextureDimension, TextureFormat as F, TextureUsages as T};

pub(super) struct Initializer {
    pub name: String,
    pub clock: mips::Clock,
    pub passes: Vec<PassId>,
}

impl Instance {
    pub(super) fn new(device: &wgpu::Device, queue: &wgpu::Queue, decl: &Bundle, adapter: Option<&wgpu::Adapter>, pipeline_cache: Option<&wgpu::PipelineCache>, target: Option<&embed::Target<'_>>, clock: Option<fn() -> std::time::Duration>) -> Result<Self> {
        let exec = loft_wgpu::schedule::descriptor(decl)?;
        let mut decl = decl.clone();
        // Initializers allocate IDs from the ordinary tables. Keep the descriptor out of that space
        // and pass the decoded execution to both allocation and linking.
        decl.tables.retain(|t| t.id != execution::DESCRIPTOR);
        let mut tables = storage::allocations_with_execution(&decl, &device.limits(), &exec)?.into_iter().map(|(t,active,alloc)| {
            let resource = storage::create(device, queue, t, alloc, interop::buffer_usage(&decl, t.id),
                decl.resources.comparison_samplers.contains(&t.id))?;
            Ok((t.id, Table { decl:t.clone(), resource, active, epoch:0 }))
        }).collect::<Result<BTreeMap<_,_>>>()?;
        let checkpoints = if let Some(s) = &decl.state {
            let table = tables.get(&s.table).ok_or(RunnerError::UnknownResource)?;
            let size = table.bytes()?.size();
            (0..s.checkpoint_slots).map(|_| allocate(device, size, wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST)).collect()
        } else {
            Vec::new()
        };
        let initializers = Initializers::prepare(&exec, &mut decl, &mut tables, device, adapter)?;
        let compute = encode::ComputeCache::prepare(device, &decl, &exec, adapter, pipeline_cache, target.map(|target| (target, &tables)), clock)?;
        initializers.check_params(compute.params.bytes().len() / 4)?;
        Ok(Self { tables, decl, execution: exec, compute, initializers, checkpoints, resource_epoch:0,
            uploads: wgpu::util::StagingBelt::new(device.clone(), 4096),
            history:Default::default(), outbox:None, pick_source:None })
    }
}

#[derive(Default)]
pub(super) struct Initializers {
    pub clocks: Vec<Initializer>,
    pub images: BTreeMap<String, TableId>,
    pub dimensions: BTreeMap<TableId, mips::Dimension>,
    pub packed: BTreeMap<PassId, mips::Packed>,
}

impl Initializers {
    pub fn prepare(
        exec: &Execution, decl: &mut Bundle, tables: &mut BTreeMap<TableId, Table>,
        device: &wgpu::Device, adapter: Option<&wgpu::Adapter>,
    ) -> Result<Self> {
        let mut seen = BTreeSet::new();
        let mut out = Self::default();
        for init in &exec.initializers {
            if init.passes.is_empty() || init.passes.iter().any(|id| !seen.insert(*id)
                || !decl.passes.iter().any(|p| p.id == *id
                    && matches!(p.run, PassRun::Compute { .. } | PassRun::GenerateMips { .. }))) {
                return Err(RunnerError::AbiMismatch);
            }
            let clock = match &init.clock {
                Clock::Load => mips::Clock::Load { done: false },
                Clock::Dirty(words) => mips::Clock::Dirty { words: words.clone(), last: None },
            };
            if let Some(image) = &init.image {
                let table = tables.get_mut(&image.table).ok_or(RunnerError::UnknownResource)?;
                if !matches!(table.resource, Resource::Texture(TextureBinding::Vacant))
                    || out.images.insert(init.name.clone(), image.table).is_some() {
                    return Err(RunnerError::AbiMismatch);
                }
                let mut desc = image_desc(image, &device.limits())?;
                let has_mips = passes(&decl.passes).any(|p| init.passes.contains(&p.id)
                    && matches!(p.run, PassRun::GenerateMips { texture } if texture == image.table));
                if has_mips != (image.levels > 1) { return Err(RunnerError::AbiMismatch); }
                let id = init.passes[0];
                let pass = decl.passes.iter_mut().find(|p| p.id == id).ok_or(RunnerError::AbiMismatch)?;
                let (module, slot, name, actual, workgroup) = bake_shader(&decl.modules, pass, image.table)?;
                if matches!(image.format, ImageStorage::LegacyReflect) { desc.format = actual; }
                if desc.format != actual { return Err(RunnerError::AbiMismatch); }
                let dim = match image.dimension {
                    Dimension::Plane => mips::Dimension::D2, Dimension::Volume => mips::Dimension::D3, Dimension::Cube => mips::Dimension::Cube,
                };
                out.dimensions.insert(image.table, dim);
                let packed = storage_access(device.features(), adapter, desc.format, StorageTextureAccess::WriteOnly).is_err();
                desc.label = Some(&init.name);
                if packed { desc.usage = T::TEXTURE_BINDING | T::COPY_DST | T::COPY_SRC; }
                let contract = TextureContract::from(&desc);
                let tex = Arc::new(device.create_texture(&desc));
                let group = dim.group(device.limits().max_compute_workgroup_size_z, device.limits().max_compute_invocations_per_workgroup);
                let split_cube = dim == mips::Dimension::Cube && workgroup == [8, 8, 6] && group != workgroup;
                if packed || split_cube {
                    let mut source = std::str::from_utf8(&decl.modules[module].bytes).map_err(|_| RunnerError::AbiMismatch)?.to_owned();
                    if split_cube {
                        source = source.replace("@workgroup_size(8, 8, 6)", &format!("@workgroup_size(8, 8, {})", group[2]));
                        let PassRun::Compute { dispatch: DispatchExtent::Fixed(size), .. } = &mut pass.run else {
                            return Err(RunnerError::AbiMismatch);
                        };
                        size[2] = image.extent[2].div_ceil(group[2]);
                    }
                    if packed {
                        let pack = mips::Packed::new(device, &tex, dim, 0)?;
                        // The fallback keeps the original coordinates and dispatch. Only the terminal store changes.
                        source = pack.shader(&source, &name, 2, next_slot(pass, 2)?)?;
                        let pack_table = TableId(tables.keys().map(|id| id.0).max().unwrap_or(0).checked_add(1).ok_or(RunnerError::Capacity)?);
                        let binding = PassBinding { group: 2, slot: next_slot(pass, 2)?, resource: pack_table, access: BindAccess::ReadWriteStorage };
                        let mut bindings = pass.bindings.to_vec();
                        bindings[slot] = binding;
                        pass.bindings = bindings.into_boxed_slice();
                        let decl_table = TableDecl { id: pack_table, stride_bytes: 4,
                            capacity: (pack.buffer.size() / 4).try_into().map_err(|_| RunnerError::Capacity)?,
                            initial: Box::new([]), format: ElemFormat::U32, lifetime: TableLifetime::Transient };
                        tables.insert(pack_table, Table { decl: decl_table.clone(), resource: Resource::Buffer(Some(pack.buffer.clone())), active: 0, epoch: 0 });
                        decl.tables.push(decl_table);
                        out.packed.insert(id, pack);
                    }
                    let mut blob = decl.modules[module].clone();
                    blob.id = ModuleId(decl.modules.iter().map(|m| m.id.0).max().unwrap_or(0).checked_add(1).ok_or(RunnerError::Capacity)?);
                    blob.bytes = source.into_bytes().into_boxed_slice();
                    blob.hash = *blake3::hash(&blob.bytes).as_bytes();
                    let PassRun::Compute { module, .. } = &mut pass.run else { return Err(RunnerError::AbiMismatch); };
                    *module = blob.id;
                    decl.modules.push(blob);
                }
                tables.get_mut(&image.table).ok_or(RunnerError::UnknownResource)?.resource =
                    Resource::Texture(TextureBinding::Bound(contract, tex));
            }
            out.clocks.push(Initializer { name: init.name.clone(), clock, passes: init.passes.clone() });
        }
        Ok(out)
    }

    pub fn check_params(&self, words: usize) -> Result<()> {
        if self.clocks.iter().any(|i| matches!(&i.clock, mips::Clock::Dirty { words: watch, .. }
            if watch.iter().any(|&word| word >= words))) { return Err(RunnerError::AbiMismatch); }
        Ok(())
    }
}

fn next_slot(pass: &PassDecl, group: u8) -> Result<u32> {
    pass.bindings.iter().filter(|b| b.group == group).map(|b| b.slot).max()
        .map_or(Ok(0), |slot| slot.checked_add(1).ok_or(RunnerError::Capacity))
}

fn bake_shader(modules: &[ModuleBlob], pass: &PassDecl, table: TableId) -> Result<(usize, usize, String, F, [u32; 3])> {
    let PassRun::Compute { module, entry, .. } = &pass.run else { return Err(RunnerError::AbiMismatch); };
    let index = modules.iter().position(|m| m.id == *module).ok_or(RunnerError::UnknownResource)?;
    let (slot, binding) = pass.bindings.iter().enumerate().find(|(_, b)| b.resource == table && b.access == BindAccess::StorageTex)
        .ok_or(RunnerError::AbiMismatch)?;
    let source = std::str::from_utf8(&modules[index].bytes).map_err(|_| RunnerError::AbiMismatch)?;
    let ir = naga::front::wgsl::parse_str(source).map_err(|e| RunnerError::InvalidShader(e.to_string()))?;
    let workgroup = ir.entry_points.iter().find(|ep| ep.name == *entry && ep.stage == naga::ShaderStage::Compute)
        .ok_or(RunnerError::AbiMismatch)?.workgroup_size;
    for (_, variable) in ir.global_variables.iter().filter(|(_, g)| g.binding ==
        Some(naga::ResourceBinding { group: u32::from(binding.group), binding: binding.slot })) {
        if let naga::TypeInner::Image { class: naga::ImageClass::Storage { format, access }, .. } = ir.types[variable.ty].inner
            && access == naga::StorageAccess::STORE {
            let format = match format {
                naga::StorageFormat::R8Unorm => F::R8Unorm, naga::StorageFormat::R16Float => F::R16Float,
                naga::StorageFormat::R32Float => F::R32Float, naga::StorageFormat::Rgba8Unorm => F::Rgba8Unorm,
                naga::StorageFormat::Rgba16Float => F::Rgba16Float,
                _ => return Err(RunnerError::Unsupported("bake storage format")),
            };
            return Ok((index, slot, variable.name.clone().ok_or(RunnerError::AbiMismatch)?, format, workgroup));
        }
    }
    Err(RunnerError::AbiMismatch)
}

fn image_desc<'a>(image: &Image, limits: &wgpu::Limits) -> Result<wgpu::TextureDescriptor<'a>> {
    let [width, height, depth_or_array_layers] = image.extent;
    let (dimension, limit, layers_ok, max_side) = match image.dimension {
        Dimension::Plane => (TextureDimension::D2, limits.max_texture_dimension_2d, depth_or_array_layers == 1, width.max(height)),
        Dimension::Volume => (TextureDimension::D3, limits.max_texture_dimension_3d, true, width.max(height).max(depth_or_array_layers)),
        Dimension::Cube => (TextureDimension::D2, limits.max_texture_dimension_2d,
            depth_or_array_layers == 6 && limits.max_texture_array_layers >= 6 && width == height, width),
    };
    if image.extent.contains(&0) || max_side > limit || !layers_ok
        || image.levels == 0 || image.levels > u32::BITS - max_side.leading_zeros() {
        return Err(RunnerError::Capacity);
    }
    Ok(wgpu::TextureDescriptor {
        label: None, size: wgpu::Extent3d { width, height, depth_or_array_layers },
        mip_level_count: image.levels, sample_count: 1, dimension,
        format: match image.format {
            ImageStorage::LegacyReflect | ImageStorage::R8Unorm => F::R8Unorm,
            ImageStorage::R16Float => F::R16Float, ImageStorage::R32Float => F::R32Float,
            ImageStorage::Rgba8Unorm => F::Rgba8Unorm, ImageStorage::Rgba16Float => F::Rgba16Float,
        },
        usage: T::STORAGE_BINDING | T::TEXTURE_BINDING | T::COPY_SRC, view_formats: &[],
    })
}
