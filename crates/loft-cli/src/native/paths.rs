//! Runtime bindings for the authored path present and optional photon pass.
use super::*;
use loft_passes::emit::paths::{Integrator, PathTrace};

fn append_bvh(nodes: &mut Vec<u32>, leaves: &mut Vec<u32>, tree: &loft_passes::bvh::Bvh2) -> Result<u32, String> {
    let node_base = u32::try_from(nodes.len() / 8).map_err(|_| "World BVH node address overflow")?;
    let leaf_base = u32::try_from(leaves.len() / 2).map_err(|_| "World BVH leaf address overflow")?;
    let root = if tree.nodes().is_empty() { 0 } else { node_base.checked_add(1).ok_or("World root overflow")? };
    for n in tree.nodes() {
        nodes.extend(n.lo.map(f32::to_bits));
        let base = if n.count == 0 { node_base } else { leaf_base };
        nodes.push(n.left_or_first.checked_add(base).ok_or("World BVH offset overflow")?);
        nodes.extend(n.hi.map(f32::to_bits));
        nodes.push(n.count);
    }
    leaves.extend(tree.leaves().iter().flat_map(|r| [r.kind_id, r.row_id]));
    Ok(root)
}

pub(super) struct PathGpu {
    pub(super) group: wgpu::BindGroup,
    pub(super) photons: Option<(wgpu::Buffer, command::Pass)>,
}

impl PathGpu {
    pub(super) fn layout(d: &wgpu::Device, path: &PathTrace, shader: &naga::Module) -> Result<wgpu::BindGroupLayout, String> {
        let spectral = matches!(path.integrator, Integrator::Spectral);
        let mut entries = vec![(wgpu::ShaderStages::empty(), storage(true)); 5];
        if spectral { entries.push((wgpu::ShaderStages::empty(), storage(false))); }
        if shader.global_variables.iter().any(|(_, v)| v.name.as_deref() == Some("world_data")) {
            return Err("serialized World shader uses a separate descriptor buffer; rebuild from source".into());
        }
        for entry in loft_passes::telemetry::storage_entries(shader)? {
            let stage = match (entry.entry.as_str(), entry.stage) {
                ("fs_present", naga::ShaderStage::Fragment) => FS,
                ("photons", naga::ShaderStage::Compute) if spectral => CS,
                _ => continue,
            };
            for (_, slot) in entry.bindings.into_iter().filter(|&(group, _)| group == 2) {
                entries.get_mut(slot as usize).ok_or("unexpected World storage binding")?.0 |= stage;
            }
        }
        for stage in [FS, CS] {
            // Group zero's inputs and parameters are visible in both stages.
            let count = 2 + entries.iter().filter(|(stages, _)| stages.contains(stage)).count();
            if count > d.limits().max_storage_buffers_per_shader_stage as usize {
                return Err(format!("World {stage:?} needs {count} storage bindings"));
            }
        }
        Ok(layout(d, "path world", &entries))
    }

    pub(super) fn prepare(g: &Gpu, p: &Program, fixed: &wgpu::BindGroupLayout, present: &wgpu::BindGroupLayout,
                      path_layout: &wgpu::BindGroupLayout, shader: &wgpu::ShaderModule, g0: &wgpu::BindGroup, g1: &wgpu::BindGroup) -> Result<Self, String> {
        use wgpu::{BufferUsages as U, util::DeviceExt};
        let path = p.path.as_ref().expect("path layout needs a path");
        let d = &g.device;
        let init = |bytes: &[u8]| d.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytes, usage: U::STORAGE });
        let mut words = vec![0u32; 1 + 2 * p.kinds.len()];
        let mut nodes = Vec::new();
        let mut leaves = Vec::new();
        let word = |n: usize| u32::try_from(n).map_err(|_| "World arena exceeds its word address range");
        let media_count = word(path.media.len())?;
        let mut world_data = vec![word(path.worlds.len())?, media_count];
        for world in &path.worlds {
            let instances = p.instances.iter().filter(|r| world.kinds.contains(&r.kind)).copied().collect::<Vec<_>>();
            let tree = loft_passes::bvh::Bvh2::build(&instances).map_err(|e| format!("invalid world BVH: {e:?}"))?;
            let root = append_bvh(&mut nodes, &mut leaves, &tree)?;
            world_data.extend([root, if world.media { media_count } else { 0 }]);
        }
        world_data.extend(path.media.as_flattened().as_flattened().iter().map(|v| v.to_bits()));
        for (i, kind) in p.kinds.iter().enumerate() {
            words[1 + i * 2] = word(words.len())?;
            words[2 + i * 2] = kind.stride;
            words.extend(kind.rows.iter().map(|v| v.to_bits()));
        }
        words[0] = word(words.len())?;
        words.extend(world_data);
        let count = word(words.len())?;
        if u64::from(count) * 4 > u64::from(d.limits().max_storage_buffer_binding_size) {
            return Err("World word arena exceeds the device storage binding limit".into());
        }
        let packed = init(&bytes_of(&words));
        if nodes.is_empty() { nodes.resize(8, 0); }
        let nodes = init(&bytes_of(&nodes));
        if leaves.is_empty() { leaves.resize(2, 0); }
        let leaves = init(&bytes_of(&leaves));
        let luts = init(&f32_bytes(if p.luts.is_empty() { &[0.0; 4] } else { &p.luts }));
        let lights = init(&f32_bytes(path.lamps.as_flattened().as_flattened()));
        let mut resources = vec![packed.as_entire_binding(), nodes.as_entire_binding(), luts.as_entire_binding(), lights.as_entire_binding(), leaves.as_entire_binding()];
        let bins = matches!(path.integrator, Integrator::Spectral).then(|| {
            buffer(d, "fixed photon bins", 16 * 128 * 128, U::STORAGE | U::COPY_DST)
        });
        if let Some(bins) = &bins { resources.push(bins.as_entire_binding()); }
        let group = bind(d, path_layout, &resources);
        let photons = bins.map(|bins| {
            let kernel = compute(d, "caustic photons", &[fixed, present, path_layout], shader, "photons", None);
            (bins, command::Pass::Compute(vec![Dispatch { pipeline: kernel, groups: vec![g0.clone(), g1.clone(), group.clone()], work: Work::Direct([1024, 1, 1]) }]))
        });
        Ok(Self { group, photons })
    }

    pub(super) fn has_photons(&self) -> bool { self.photons.is_some() }

}
