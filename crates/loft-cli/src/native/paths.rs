//! Runtime bindings for the authored path present and optional photon pass.
use super::*;
use loft_passes::emit::paths::{Integrator, PathTrace};

pub(super) struct PathGpu {
    pub(super) group: wgpu::BindGroup,
    photons: Option<(wgpu::ComputePipeline, wgpu::Buffer)>,
}

impl PathGpu {
    pub(super) fn layout(d: &wgpu::Device, path: &PathTrace) -> wgpu::BindGroupLayout {
        let stages = CS | FS;
        let mut entries = vec![(stages, storage(true)); 5];
        if matches!(path.integrator, Integrator::Spectral) { entries.push((stages, storage(false))); }
        layout(d, "path world", &entries)
    }

    pub(super) fn prepare(g: &Gpu, p: &Program, fixed: &wgpu::BindGroupLayout, present: &wgpu::BindGroupLayout,
                      path_layout: &wgpu::BindGroupLayout, shader: &wgpu::ShaderModule) -> Result<Self, String> {
        use wgpu::{BufferUsages as U, util::DeviceExt};
        let path = p.path.as_ref().expect("path layout needs a path");
        let d = &g.device;
        let init = |bytes: &[u8]| d.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytes, usage: U::STORAGE });
        let tree = loft_passes::bvh::Bvh2::build(&p.instances)
            .map_err(|e| format!("invalid world BVH: {e:?}"))?;
        // Frozen leaf IDs remain (kind,row). A per-kind base translates them into the packed row arena.
        let mut words = vec![0u32; 1 + 2 * p.kinds.len()];
        words[0] = tree.nodes().len() as u32;
        for (i, kind) in p.kinds.iter().enumerate() {
            words[1 + i * 2] = words.len() as u32;
            words[2 + i * 2] = kind.stride;
            words.extend(kind.rows.iter().map(|v| v.to_bits()));
        }
        let packed = init(&bytes_of(&words));
        let mut nodes = Vec::new();
        for n in tree.nodes() {
            nodes.extend(n.lo.map(f32::to_bits));
            nodes.push(n.left_or_first);
            nodes.extend(n.hi.map(f32::to_bits));
            nodes.push(n.count);
        }
        if nodes.is_empty() { nodes.resize(8, 0); }
        let nodes = init(&bytes_of(&nodes));
        let mut leaves: Vec<_> = tree.leaves().iter().flat_map(|r| [r.kind_id, r.row_id]).collect();
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
            (kernel, bins)
        });
        Ok(Self { group, photons })
    }

    pub(super) fn has_photons(&self) -> bool { self.photons.is_some() }

    pub(super) fn encode_photons(&self, enc: &mut wgpu::CommandEncoder, fixed: &wgpu::BindGroup,
        present: &wgpu::BindGroup, timestamp_writes: Option<wgpu::ComputePassTimestampWrites<'_>>) -> u32 {
        let Some((kernel, bins)) = &self.photons else { return 0; };
        enc.clear_buffer(bins, 0, None);
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("caustic photons"), timestamp_writes });
        pass.set_pipeline(kernel);
        pass.set_bind_group(0, fixed, &[]);
        pass.set_bind_group(1, present, &[]);
        pass.set_bind_group(2, &self.group, &[]);
        pass.dispatch_workgroups(1024, 1, 1);
        1
    }
}
