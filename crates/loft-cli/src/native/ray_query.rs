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
