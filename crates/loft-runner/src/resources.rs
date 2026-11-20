use super::*;
use loft_std::{interop::TextureBinding, storage::{Allocation, attachment}};

pub(super) fn create(d: &wgpu::Device, q: &wgpu::Queue, t: &TableDecl, alloc: Allocation, usage: wgpu::BufferUsages) -> Result<Resource> {
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
            compare: Some(wgpu::CompareFunction::LessEqual), ..Default::default()
        })),
        Allocation::HostAttachment => Resource::HostAttachment,
    })
}
