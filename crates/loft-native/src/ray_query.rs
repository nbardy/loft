//! Device negotiation. Requests the hardware ray query that native World traversal uses when the adapter grants it.
use wgpu as gpu;

/// Whether the adapter grants the ray-query feature with room for one AABB geometry and one instance.
fn grants_ray_query(features: gpu::Features, limits: &gpu::Limits) -> bool {
    features.contains(gpu::Features::EXPERIMENTAL_RAY_QUERY)
        && limits.max_blas_geometry_count >= 1
        && limits.max_blas_primitive_count >= 1
        && limits.max_tlas_instance_count >= 1
        && limits.max_acceleration_structures_per_shader_stage != 0
        && limits.max_buffers_and_acceleration_structures_per_shader_stage != 0
}

use super::Gpu;

pub(super) fn request_auto() -> Result<Gpu, String> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .map_err(|e| format!("no adapter: {e}"))?;
    let adapter_limits = adapter.limits();
    let adapter_features = adapter.features();
    // PIPELINE_CACHE exists on Vulkan only: wgpu 30's Metal backend stubs `create_pipeline_cache` (it never uses
    // MTLBinaryArchive), so on Metal the only cache across processes is the OS's own Metal shader cache.
    let mut requested = wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::PIPELINE_CACHE | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
    if grants_ray_query(adapter_features, &adapter_limits) {
        requested |= wgpu::Features::EXPERIMENTAL_RAY_QUERY;
    }
    // SHADER_F16 is requested whenever the adapter has it, so `Precision::resolve` can tell a device that lacks it
    // (recorded as `Resolved::HalfStorage`) from one that has it.
    requested |= wgpu::Features::SHADER_F16;
    let features = adapter_features & requested;
    // Opt in only when the adapter grants the unstable native traversal feature.
    let experimental_features = if features.contains(wgpu::Features::EXPERIMENTAL_RAY_QUERY) {
        #[allow(unsafe_code)]
        unsafe { wgpu::ExperimentalFeatures::enabled() }
    } else {
        Default::default()
    };
    let mut limits = wgpu::Limits {
        max_compute_invocations_per_workgroup: adapter_limits.max_compute_invocations_per_workgroup.min(384),
        ..Default::default()
    };
    if features.contains(wgpu::Features::EXPERIMENTAL_RAY_QUERY) {
        limits = loft_runner::ray_query::limits(limits, adapter_limits);
    }
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("loft-runner"),
        required_features: features,
        experimental_features,
        required_limits: limits,
        ..Default::default()
    }))
    .map_err(|e| format!("no device: {e}"))?;
    let info = adapter.get_info();
    Ok(Gpu {
        raw_adapter: adapter,
        device,
        queue,
        device_signature: format!("{} ({:?}) {} {}", info.name, info.backend, info.driver, info.driver_info),
        adapter: format!("{} ({:?})", info.name, info.backend),
        timestamps: features.contains(wgpu::Features::TIMESTAMP_QUERY),
        cache_key: features.contains(wgpu::Features::PIPELINE_CACHE).then(|| wgpu::util::pipeline_cache_key(&info)).flatten(),
    })
}
