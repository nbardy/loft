//! Checked allocations and byte ranges for the table ABI.
use crate::{
    bundle::{Resources, Role},
    error::*,
};
use loft_ir::*;
use std::collections::BTreeSet;
use wgpu::TextureUsages as T;

pub enum Allocation {
    Buffer(u64),
    ExternalBuffer,
    Texture,
    Attachment(TextureDecl),
    Sampler,
    AccelerationStructure,
    HostAttachment,
}

pub fn allocations<'a>(b: &'a Bundle, limits: &wgpu::Limits) -> Result<Vec<(&'a TableDecl, u32, Allocation)>> {
    let roles = Resources::new(b)?;
    let mut seen = BTreeSet::new();
    let mut buffers = BTreeSet::new();
    let mut out = Vec::new();
    for t in &b.tables {
        let role = roles.role(t.id);
        if !seen.insert(t.id)
            || t.stride_bytes == 0
            || !t.stride_bytes.is_multiple_of(4)
            || !t.initial.len().is_multiple_of(t.stride_bytes as usize)
            || matches!(t.format, ElemFormat::Struct(i) if b.struct_layouts.get(i as usize).is_none_or(|s| s.stride_bytes != t.stride_bytes))
            || matches!(t.format, ElemFormat::F64 if t.stride_bytes < 8)
        {
            return Err(RunnerError::AbiMismatch);
        }
        let active =
            u32::try_from(t.initial.len() / t.stride_bytes as usize).map_err(|_| RunnerError::Capacity)?;
        if active > t.capacity
            || active != 0 && (t.lifetime == TableLifetime::External || !matches!(role, Role::Buffer | Role::AccelerationStructure))
        {
            return Err(RunnerError::AbiMismatch);
        }
        if matches!(role, Role::Buffer) {
            buffers.insert(t.id);
        }
        let alloc = match role {
            Role::Buffer if t.lifetime == TableLifetime::External => Allocation::ExternalBuffer,
            Role::Buffer => Allocation::Buffer(bounds(t, t.capacity, limits)?),
            Role::Texture => Allocation::Texture,
            Role::Attachment(desc) => Allocation::Attachment(desc),
            Role::Sampler => Allocation::Sampler,
            Role::AccelerationStructure => Allocation::AccelerationStructure,
            Role::HostAttachment => Allocation::HostAttachment,
        };
        out.push((t, active, alloc));
    }
    if b.rows.values().any(|id| !buffers.contains(id)) {
        return Err(RunnerError::UnknownResource);
    }
    Ok(out)
}

pub fn bounds(t: &TableDecl, n: u32, limits: &wgpu::Limits) -> Result<u64> {
    let size = u64::from(n) * u64::from(t.stride_bytes);
    if size > limits.max_buffer_size
        || matches!(t.lifetime, TableLifetime::DynamicArena { max_rows } if n > max_rows)
    {
        return Err(RunnerError::Capacity);
    }
    Ok(size)
}

pub fn rows(t: &TableDecl, start: u32, bytes: &[u8]) -> Result<(u64, u32)> {
    if !bytes.len().is_multiple_of(t.stride_bytes as usize) {
        return Err(RunnerError::AbiMismatch);
    }
    let count = u32::try_from(bytes.len() / t.stride_bytes as usize).map_err(|_| RunnerError::Capacity)?;
    let end = start
        .checked_add(count)
        .filter(|&n| n <= t.capacity)
        .ok_or(RunnerError::Capacity)?;
    Ok((u64::from(start) * u64::from(t.stride_bytes), end))
}

pub fn attachment(t: TextureDecl, limits: &wgpu::Limits) -> Result<wgpu::TextureDescriptor<'static>> {
    let [width, height, layers] = t.extent;
    let format = match t.format {
        ImageFormat::Rg32Uint => wgpu::TextureFormat::Rg32Uint,
        ImageFormat::Rgba16Float => wgpu::TextureFormat::Rgba16Float,
        ImageFormat::Depth32Float => wgpu::TextureFormat::Depth32Float,
    };
    if width == 0
        || height == 0
        || width.max(height) > limits.max_texture_dimension_2d
        || layers != 1
        || !matches!(t.samples, 1 | 4)
        || (t.samples == 4 && matches!(t.format, ImageFormat::Rgba16Float))
        || t.clear.is_some_and(|clear| clear.iter().any(|v| !v.is_finite()))
    {
        return Err(RunnerError::AbiMismatch);
    }
    let mut usage = T::TEXTURE_BINDING | T::RENDER_ATTACHMENT;
    if t.samples == 1 { usage |= T::COPY_SRC; }
    if matches!(t.format, ImageFormat::Rgba16Float) {
        usage |= T::STORAGE_BINDING;
    }
    Ok(wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: layers,
        },
        mip_level_count: 1,
        sample_count: u32::from(t.samples),
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

pub fn compatible_state<'a>(
    old: Option<&StateStoreDecl>,
    new: Option<&'a mut StateStoreDecl>,
) -> Option<(TableId, &'a mut StateStoreDecl)> {
    let (old, new) = old.zip(new)?;
    if old.layout_hash != new.layout_hash {
        return None;
    }
    new.checkpoint_slots = old.checkpoint_slots;
    Some((old.table, new))
}

pub fn preserve_capacity(next: &mut TableDecl, current: &TableDecl) -> Result<()> {
    if next.stride_bytes != current.stride_bytes
        || matches!(next.lifetime, TableLifetime::DynamicArena { max_rows } if max_rows < current.capacity)
    {
        return Err(RunnerError::AbiMismatch);
    }
    next.capacity = current.capacity;
    Ok(())
}

use crate::{allocate, interop::{Resource, TextureBinding}};

pub fn create(d: &wgpu::Device, q: &wgpu::Queue, t: &TableDecl, alloc: Allocation, usage: wgpu::BufferUsages, comparison: bool) -> Result<Resource> {
    Ok(match alloc {
        Allocation::Buffer(size) => {
            let buf = allocate(d, size, usage);
            if !t.initial.is_empty() { q.write_buffer(&buf,0,&t.initial); }
            Resource::Buffer(Some(buf))
        }
        Allocation::ExternalBuffer => Resource::Buffer(None),
        Allocation::Texture => Resource::Texture(TextureBinding::Vacant),
        Allocation::Attachment(desc) => {
            let tex = d.create_texture(&attachment(desc,&d.limits())?);
            Resource::Attachment(tex.create_view(&Default::default()),desc)
        }
        Allocation::Sampler => Resource::Sampler(d.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            compare: comparison.then_some(wgpu::CompareFunction::LessEqual), ..Default::default()
        })),
        Allocation::AccelerationStructure => Resource::AccelerationStructure(crate::ray_query::build_aabbs(d, q, t)?),
        Allocation::HostAttachment => Resource::HostAttachment,
    })
}
