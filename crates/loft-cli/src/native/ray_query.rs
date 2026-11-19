//! Native traversal for static triangle assets. The caller records and submits on its own device.
use wgpu::{self as gpu, util::DeviceExt};

pub fn build(
    device: &gpu::Device, encoder: &mut gpu::CommandEncoder, triangles: &[[[f32; 3]; 3]],
) -> Result<gpu::Tlas, &'static str> {
    if !device.features().contains(gpu::Features::EXPERIMENTAL_RAY_QUERY) {
        return Err("device has no hardware ray queries");
    }
    let vertex_count = u32::try_from(triangles.len()).ok().and_then(|n| n.checked_mul(3))
        .filter(|&n| n != 0).ok_or("invalid triangle count")?;
    let bytes = triangles.as_flattened().as_flattened().iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>();
    if bytes.len() as u64 > device.limits().max_buffer_size {
        return Err("triangle data exceeds device limits");
    }
    let vertices = device.create_buffer_init(&gpu::util::BufferInitDescriptor {
        label: None, contents: &bytes, usage: gpu::BufferUsages::BLAS_INPUT,
    });
    let size = gpu::BlasTriangleGeometrySizeDescriptor {
        vertex_format: gpu::VertexFormat::Float32x3, vertex_count,
        index_format: None, index_count: None, flags: gpu::AccelerationStructureGeometryFlags::OPAQUE,
    };
    let blas = device.create_blas(&gpu::CreateBlasDescriptor {
        label: None, flags: gpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
        update_mode: gpu::AccelerationStructureUpdateMode::Build,
    }, gpu::BlasGeometrySizeDescriptors::Triangles { descriptors: vec![size.clone()] });
    let mut tlas = device.create_tlas(&gpu::CreateTlasDescriptor {
        label: None, max_instances: 1, flags: gpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
        update_mode: gpu::AccelerationStructureUpdateMode::Build,
    });
    tlas[0] = Some(gpu::TlasInstance::new(&blas,
        [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0.], 0, 0xff));
    encoder.build_acceleration_structures([&gpu::BlasBuildEntry {
        blas: &blas,
        geometry: gpu::BlasGeometries::TriangleGeometries(vec![gpu::BlasTriangleGeometry {
            size: &size, vertex_buffer: &vertices, first_vertex: 0, vertex_stride: 12,
            index_buffer: None, first_index: None, transform_buffer: None, transform_buffer_offset: None,
        }]),
    }], [&tlas]);
    Ok(tlas)
}

/// A composable shader query. Returns -1 on miss and distance in units of the supplied direction on hit.
pub const WGSL: &str = r#"
enable wgpu_ray_query;
fn mesh_ray_query(tree: acceleration_structure, origin: vec3f, dir: vec3f, far: f32) -> f32 {
    var query: ray_query;
    rayQueryInitialize(&query, tree, RayDesc(0u, 0xffu, 0.0, far, origin, dir));
    while rayQueryProceed(&query) {}
    let hit = rayQueryGetCommittedIntersection(&query);
    return select(hit.t, -1.0, hit.kind == 0u);
}
"#;

use super::Gpu;

pub(super) fn request_gpu(ray_query: bool) -> Result<Gpu, String> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .map_err(|e| format!("no adapter: {e}"))?;
    // PIPELINE_CACHE exists on Vulkan only: wgpu 30's Metal backend stubs `create_pipeline_cache` (it never uses
    // MTLBinaryArchive), so on Metal the only cache across processes is the OS's own Metal shader cache.
    let mut requested = wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::PIPELINE_CACHE | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
    if ray_query { requested |= wgpu::Features::EXPERIMENTAL_RAY_QUERY; }
    let features = adapter.features() & requested;
    // Opt in only for callers that request the unstable native traversal feature.
    let experimental_features = if features.contains(wgpu::Features::EXPERIMENTAL_RAY_QUERY) {
        #[allow(unsafe_code)]
        unsafe { wgpu::ExperimentalFeatures::enabled() }
    } else {
        Default::default()
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("loft-runner"),
        required_features: features,
        experimental_features,
        // The cube kernel covers six 8x8 faces. wgpu otherwise grants only the default 256 lanes.
        required_limits: wgpu::Limits {
            max_compute_invocations_per_workgroup: adapter.limits().max_compute_invocations_per_workgroup.min(384),
            ..Default::default()
        },
        ..Default::default()
    }))
    .map_err(|e| format!("no device: {e}"))?;
    let info = adapter.get_info();
    use wgpu::TextureFormat as F;
    let bake_formats = [F::R8Unorm, F::R16Float, F::R32Float, F::Rgba8Unorm, F::Rgba16Float].into_iter()
        .filter(|f| {
            let ff = if features.contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES) {
                adapter.get_texture_format_features(*f)
            } else {
                f.guaranteed_format_features(features)
            };
            ff.allowed_usages.contains(wgpu::TextureUsages::STORAGE_BINDING)
                && ff.flags.contains(wgpu::TextureFormatFeatureFlags::STORAGE_WRITE_ONLY)
        }).collect();
    Ok(Gpu {
        bake_formats,
        device,
        queue,
        device_signature: format!("{} ({:?}) {} {}", info.name, info.backend, info.driver, info.driver_info),
        adapter: format!("{} ({:?})", info.name, info.backend),
        timestamps: features.contains(wgpu::Features::TIMESTAMP_QUERY),
        cache_key: features.contains(wgpu::Features::PIPELINE_CACHE).then(|| wgpu::util::pipeline_cache_key(&info)).flatten(),
    })
}
