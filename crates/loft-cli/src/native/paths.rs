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
        let mut entries = vec![(stages, storage(true)); 4];
        if matches!(path.integrator, Integrator::Spectral) { entries.push((stages, storage(false))); }
        layout(d, "path world", &entries)
    }

    pub(super) fn prepare(g: &Gpu, p: &Program, fixed: &wgpu::BindGroupLayout, present: &wgpu::BindGroupLayout,
                      path_layout: &wgpu::BindGroupLayout, shader: &wgpu::ShaderModule) -> Self {
        use wgpu::{BufferUsages as U, util::DeviceExt};
        let path = p.path.as_ref().expect("path layout needs a path");
        let d = &g.device;
        let init = |bytes: &[u8]| d.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytes, usage: U::STORAGE });
        let mut words = Vec::new();
        let mut base = Vec::new();
        for kind in &p.kinds {
            base.push(words.len() as u32);
            words.extend_from_slice(&kind.rows);
        }
        if words.is_empty() { words.extend([0.0; 4]); }
        let packed = init(&f32_bytes(&words));
        let mut rows = Vec::new();
        for row in &p.instances {
            let i = row.kind as usize;
            rows.extend(row.lo.map(f32::to_bits));
            rows.push(row.kind);
            rows.extend(row.hi.map(f32::to_bits));
            rows.push(base[i] + row.row * p.kinds[i].stride);
        }
        if rows.is_empty() {
            // The storage array requires one stride. An empty sentinel is behind every ray.
            rows.extend([0.0_f32.to_bits(); 8]);
            rows[3] = u32::MAX;
        }
        let rows = init(&bytes_of(&rows));
        let luts = init(&f32_bytes(if p.luts.is_empty() { &[0.0; 4] } else { &p.luts }));
        let lights = init(&f32_bytes(path.lamps.as_flattened().as_flattened()));
        let mut resources = vec![packed.as_entire_binding(), rows.as_entire_binding(), luts.as_entire_binding(), lights.as_entire_binding()];
        let bins = matches!(path.integrator, Integrator::Spectral).then(|| {
            buffer(d, "fixed photon bins", 16 * 128 * 128, U::STORAGE | U::COPY_DST)
        });
        if let Some(bins) = &bins { resources.push(bins.as_entire_binding()); }
        let group = bind(d, path_layout, &resources);
        let photons = bins.map(|bins| {
            let kernel = compute(d, "caustic photons", &[fixed, present, path_layout], shader, "photons", None);
            (kernel, bins)
        });
        Self { group, photons }
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
