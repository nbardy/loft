//! Build the static procedural bounds used by native World traversal.
use crate::error::{Result, RunnerError};
use loft_ir::{TableDecl, TableLifetime};
use wgpu::{self as gpu, util::DeviceExt};

pub fn limits(base: gpu::Limits, adapter: gpu::Limits) -> gpu::Limits {
    // The experimental token alone leaves both per-stage limits at zero.
    base.using_acceleration_structure_values(adapter)
}

pub fn has_aabb_capacity(device: &gpu::Device, count: usize) -> bool {
    let limits = device.limits();
    device.features().contains(gpu::Features::EXPERIMENTAL_RAY_QUERY)
        && count != 0
        && count <= limits.max_blas_primitive_count as usize
        && limits.max_blas_geometry_count > 0
        && limits.max_tlas_instance_count > 0
        && limits.max_acceleration_structures_per_shader_stage > 0
        && (count as u64) * gpu::AABB_GEOMETRY_MIN_STRIDE <= limits.max_buffer_size
}

pub fn build_aabbs(device: &gpu::Device, queue: &gpu::Queue, table: &TableDecl) -> Result<gpu::Tlas> {
    if table.lifetime != TableLifetime::Static || u64::from(table.stride_bytes) != gpu::AABB_GEOMETRY_MIN_STRIDE
        || table.initial.len() as u64 != u64::from(table.capacity) * gpu::AABB_GEOMETRY_MIN_STRIDE
    {
        return Err(RunnerError::AbiMismatch);
    }
    if !has_aabb_capacity(device, table.capacity as usize) {
        return Err(RunnerError::Unsupported("ray-query acceleration-structure capacity"));
    }
    for row in table.initial.as_chunks::<24>().0 {
        let bounds: [f32; 6] = std::array::from_fn(|i| f32::from_le_bytes(row[i * 4..][..4].try_into().unwrap()));
        if bounds.iter().any(|x| !x.is_finite()) || (0..3).any(|i| bounds[i] > bounds[i + 3]) {
            return Err(RunnerError::AbiMismatch);
        }
    }
    let bounds = device.create_buffer_init(&gpu::util::BufferInitDescriptor {
        label: None, contents: &table.initial, usage: gpu::BufferUsages::BLAS_INPUT,
    });
    let size = gpu::BlasAABBGeometrySizeDescriptor {
        primitive_count: table.capacity, flags: gpu::AccelerationStructureGeometryFlags::empty(),
    };
    let flags = gpu::AccelerationStructureFlags::PREFER_FAST_TRACE;
    let update_mode = gpu::AccelerationStructureUpdateMode::Build;
    let blas = device.create_blas(&gpu::CreateBlasDescriptor { label: None, flags, update_mode },
        gpu::BlasGeometrySizeDescriptors::AABBs { descriptors: vec![size.clone()] });
    let mut tlas = device.create_tlas(&gpu::CreateTlasDescriptor { label: None, max_instances: 1, flags, update_mode });
    tlas[0] = Some(gpu::TlasInstance::new(&blas,
        [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0.], 0, 0xff));
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.build_acceleration_structures([&gpu::BlasBuildEntry {
        blas: &blas,
        geometry: gpu::BlasGeometries::AabbGeometries(vec![gpu::BlasAabbGeometry {
            size: &size, stride: gpu::AABB_GEOMETRY_MIN_STRIDE, aabb_buffer: &bounds, primitive_offset: 0,
        }]),
    }], [&tlas]);
    queue.submit([encoder.finish()]);
    Ok(tlas)
}
