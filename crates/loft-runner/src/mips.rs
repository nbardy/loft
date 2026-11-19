use super::command::{Dispatch, Pass, Stamps, Work};
pub use loft_std::mips::{Clock, Dimension};
use std::collections::HashMap;
use wgpu::*;

#[derive(Default)]
pub struct Pipelines(HashMap<(Dimension, TextureFormat), ComputePipeline>);
pub struct Mips(Vec<Pass>);
impl Pipelines {
    pub fn prepare(&mut self, device: &Device, texture: &Texture, dim: Dimension) -> Result<Mips,String> {
        let format = texture.format();
        let pipeline = match self.0.entry((dim, format)) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                let text = loft_std::mips::source(dim, format)?;
                let ir = naga::front::wgsl::parse_str(&text).map_err(|e| e.emit_to_string(&text))?;
                let entries = loft_std::mips::bindings(dim,format);
                let group = device.create_bind_group_layout(&BindGroupLayoutDescriptor { label:None, entries:&entries });
                let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor { label:None, bind_group_layouts:&[Some(&group)], immediate_size:0 });
                let module = device.create_shader_module(ShaderModuleDescriptor { label:Some("mip reduction"), source:ShaderSource::Naga(std::borrow::Cow::Owned(ir)) });
                entry.insert(device.create_compute_pipeline(&ComputePipelineDescriptor { label:None, layout:Some(&layout), module:&module,
                    entry_point:Some("downsample"), compilation_options:Default::default(), cache:None }))
            }
        };
        let layout = pipeline.get_bind_group_layout(0);
        let size = [texture.width(),texture.height(),texture.depth_or_array_layers()];
        let views = (0..texture.mip_level_count()).map(|level| texture.create_view(&TextureViewDescriptor {
            dimension: Some(dim.storage_view()), base_mip_level:level, mip_level_count:Some(1), ..Default::default()
        })).collect::<Vec<_>>();
        let levels = views.windows(2).enumerate().map(|(level, pair)| {
            let entries = [0,1].map(|i| BindGroupEntry { binding:i as u32, resource:BindingResource::TextureView(&pair[i]) });
            let bg = device.create_bind_group(&BindGroupDescriptor { label:None, layout:&layout, entries:&entries });
            let extent = dim.extent(size,level as u32 + 1);
            let work = Work::Direct(std::array::from_fn(|i| extent[i].div_ceil(dim.group()[i])));
            Pass::Compute(vec![Dispatch { pipeline:pipeline.clone(), groups:vec![bg], work }])
        }).collect();
        Ok(Mips(levels))
    }
}
impl Mips {
    pub fn encode(&self, enc: &mut CommandEncoder, timestamps: Option<(&QuerySet,u32)>) -> u32 {
        for (i,p) in self.0.iter().enumerate() {
            let last = i+1 == self.0.len();
            let stamps = timestamps.filter(|_| i == 0 || last).map(|(set, start)| Stamps {
                set, start:(i == 0).then_some(start), end:last.then_some(start + 1)
            });
            p.record(enc, stamps);
        }
        self.0.len() as u32
    }
}
