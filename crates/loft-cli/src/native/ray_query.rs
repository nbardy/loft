//! Native traversal for static triangle assets. The caller records and submits on its own device.
use wgpu::{self as gpu, util::DeviceExt};

/// The adapter and device must both grant this capacity for the required route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Requirements {
    pub geometries: u32,
    pub primitives: u32,
    pub instances: u32,
}

impl Requirements {
    const ONE: Self = Self { geometries: 1, primitives: 1, instances: 1 };

    fn limits(limits: &gpu::Limits) -> Self {
        Self {
            geometries: limits.max_blas_geometry_count,
            primitives: limits.max_blas_primitive_count,
            instances: limits.max_tlas_instance_count,
        }
    }

    fn validate(self, features: gpu::Features, limits: &gpu::Limits) -> Result<(), RayError> {
        if !features.contains(gpu::Features::EXPERIMENTAL_RAY_QUERY) {
            return Err(RayError::Feature);
        }
        let available = Self::limits(limits);
        if self.geometries > available.geometries
            || self.primitives > available.primitives
            || self.instances > available.instances
        {
            return Err(RayError::Capacity { required: self, available });
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum RayError {
    Feature,
    Capacity { required: Requirements, available: Requirements },
    Asset(&'static str),
    Adapter(String),
    Device(String),
}

impl std::fmt::Display for RayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Feature => f.write_str("required hardware ray-query feature is unavailable"),
            Self::Capacity { required, available } => write!(f, "required ray capacity {required:?}, granted {available:?}"),
            Self::Asset(message) => f.write_str(message),
            Self::Adapter(message) => write!(f, "no adapter: {message}"),
            Self::Device(message) => write!(f, "no device: {message}"),
        }
    }
}

impl std::error::Error for RayError {}

#[derive(Clone, Copy)]
pub struct Instance {
    /// Row-major 3x4 affine transform in the same order as a TLAS descriptor.
    pub transform: [f32; 12],
    pub row: u32,
}

pub fn build(
    device: &gpu::Device,
    encoder: &mut gpu::CommandEncoder,
    triangles: &[[[f32; 3]; 3]],
    instances: &[Instance],
) -> Result<gpu::Tlas, RayError> {
    let primitive_count = u32::try_from(triangles.len()).map_err(|_| RayError::Asset("too many triangles"))?;
    let instance_count = u32::try_from(instances.len()).map_err(|_| RayError::Asset("too many instances"))?;
    let vertex_count = primitive_count.checked_mul(3)
        .filter(|&n| n != 0).ok_or(RayError::Asset("invalid triangle count"))?;
    if instances.is_empty() || instances.iter().any(|instance| {
        instance.row >= (1 << 24) || !instance.transform.iter().all(|x| x.is_finite())
    }) {
        return Err(RayError::Asset("ray instances require finite transforms and 24-bit row IDs"));
    }
    let limits = device.limits();
    Requirements { geometries: 1, primitives: primitive_count, instances: instance_count }
        .validate(device.features(), &limits)?;
    let bytes = triangles.as_flattened().as_flattened().iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>();
    if bytes.len() as u64 > limits.max_buffer_size {
        return Err(RayError::Asset("triangle data exceeds device limits"));
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
        label: None, max_instances: instance_count, flags: gpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
        update_mode: gpu::AccelerationStructureUpdateMode::Build,
    });
    for (index, instance) in instances.iter().enumerate() {
        tlas[index] = Some(gpu::TlasInstance::new(&blas, instance.transform, instance.row, 0xff));
    }
    encoder.build_acceleration_structures([&gpu::BlasBuildEntry {
        blas: &blas,
        geometry: gpu::BlasGeometries::TriangleGeometries(vec![gpu::BlasTriangleGeometry {
            size: &size, vertex_buffer: &vertices, first_vertex: 0, vertex_stride: 12,
            index_buffer: None, first_index: None, transform_buffer: None, transform_buffer_offset: None,
        }]),
    }], [&tlas]);
    Ok(tlas)
}

/// Shader-side triangle result, with the authored row and primitive identity.
pub const WGSL: &str = r#"
enable wgpu_ray_query;
struct MeshRayHit { t: f32, row: u32, triangle: u32 }
fn mesh_ray_query(tree: acceleration_structure, origin: vec3f, dir: vec3f, near: f32, far: f32, any: bool) -> MeshRayHit {
    var query: ray_query;
    let flags = select(0u, RAY_FLAG_TERMINATE_ON_FIRST_HIT, any);
    rayQueryInitialize(&query, tree, RayDesc(flags, 0xffu, near, far, origin, dir));
    while rayQueryProceed(&query) {}
    let hit = rayQueryGetCommittedIntersection(&query);
    if hit.kind == 0u { return MeshRayHit(-1.0, 0xffffffffu, 0xffffffffu); }
    return MeshRayHit(hit.t, hit.instance_custom_data, hit.primitive_index);
}
"#;

use super::Gpu;

pub(super) fn request_gpu(ray_query: bool) -> Result<Gpu, String> {
    attach(ray_query.then_some(Requirements::ONE)).map_err(|err| err.to_string())
}

pub fn request(required: Requirements) -> Result<Gpu, RayError> {
    attach(Some(required))
}

fn attach(requirements: Option<Requirements>) -> Result<Gpu, RayError> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .map_err(|e| RayError::Adapter(e.to_string()))?;
    let adapter_limits = adapter.limits();
    let adapter_features = adapter.features();
    if let Some(required) = requirements {
        required.validate(adapter_features, &adapter_limits)?;
    }
    // PIPELINE_CACHE exists on Vulkan only: wgpu 30's Metal backend stubs `create_pipeline_cache` (it never uses
    // MTLBinaryArchive), so on Metal the only cache across processes is the OS's own Metal shader cache.
    let mut requested = wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::PIPELINE_CACHE | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
    if requirements.is_some() { requested |= wgpu::Features::EXPERIMENTAL_RAY_QUERY; }
    let features = adapter_features & requested;
    // Opt in only for callers that request the unstable native traversal feature.
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
    // Feature opt-in alone leaves every acceleration-structure limit at zero.
    if requirements.is_some() {
        limits.max_blas_geometry_count = adapter_limits.max_blas_geometry_count;
        limits.max_blas_primitive_count = adapter_limits.max_blas_primitive_count;
        limits.max_tlas_instance_count = adapter_limits.max_tlas_instance_count;
    }
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("loft-runner"),
        required_features: features,
        experimental_features,
        required_limits: limits,
        ..Default::default()
    }))
    .map_err(|e| RayError::Device(e.to_string()))?;
    if let Some(required) = requirements {
        required.validate(device.features(), &device.limits())?;
    }
    let info = adapter.get_info();
    Ok(Gpu {
        device,
        queue,
        device_signature: format!("{} ({:?}) {} {}", info.name, info.backend, info.driver, info.driver_info),
        adapter: format!("{} ({:?})", info.name, info.backend),
        timestamps: features.contains(wgpu::Features::TIMESTAMP_QUERY),
        cache_key: features.contains(wgpu::Features::PIPELINE_CACHE).then(|| wgpu::util::pipeline_cache_key(&info)).flatten(),
    })
}
