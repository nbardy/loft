//! Cached raster pipelines and indirect draw bundles for the host's attachments.
use super::*;
use encode::Group;
use loft_std::{attachments::Canvas, schedule::Raster};
pub(super) use loft_std::attachments::Target;
use wgpu::*;

pub(super) struct Render {
    pub spec: Raster,
    pub shaders: Vec<ShaderModule>,
    pub layout: PipelineLayout,
    pub cached: Option<(Canvas, Vec<RenderPipeline>, u64, RenderBundle)>,
}

impl Render {
    pub fn prepare(&mut self, device: &Device, target: &Target<'_>, fixed: &BindGroup,
                   groups: &[Group], tables: &BTreeMap<TableId, Table>, epoch: u64) -> super::Result<()> {
        let canvas = self.spec.targets.canvas(target, |id| tables.get(&id)?.attachment())?;
        let mut newest = groups.iter().flat_map(|g| &g.slots).map(|(_,id)| tables[id].epoch).max().unwrap_or(0);
        for draw in &self.spec.draws {
            let args = &tables[&draw.decl.args];
            let buf = args.bytes()?;
            let size = match draw.index {
                Some((id, _)) => {
                    let index = &tables[&id];
                    if !index.bytes()?.usage().contains(BufferUsages::INDEX) { return Err(RunnerError::AbiMismatch); }
                    newest = newest.max(index.epoch);
                    20
                }
                None => 16,
            };
            if !buf.usage().contains(BufferUsages::INDIRECT) { return Err(RunnerError::AbiMismatch); }
            if u64::from(draw.decl.offset_bytes) + size > buf.size() { return Err(RunnerError::Capacity); }
            // Bundles capture argument and index handles even when the shader has no binding for either.
            newest = newest.max(args.epoch);
        }
        if self.cached.as_ref().is_some_and(|(c,_,e,_)| *c == canvas && *e >= newest) { return Ok(()); }
        let targets: Vec<_> = canvas.colors.iter().map(|&format| Some(format.into())).collect();
        let pipelines = match self.cached.take() {
            Some((old, pipelines, _, _)) if old == canvas => pipelines,
            _ => self.spec.draws.iter().zip(&self.shaders).map(|(draw, shader)| {
                device.create_render_pipeline(&RenderPipelineDescriptor {
                    label: None, layout: Some(&self.layout),
                    vertex: VertexState { module: shader, entry_point: Some(&draw.decl.vertex_entry),
                        buffers: &[], compilation_options: Default::default() },
                    primitive: PrimitiveState { cull_mode: self.spec.cull_front.then_some(Face::Front), ..Default::default() },
                    depth_stencil: canvas.depth_state(),
                    multisample: MultisampleState { count: canvas.samples, ..Default::default() },
                    fragment: (!draw.decl.fragment_entry.is_empty()).then_some(FragmentState {
                        module: shader, entry_point: Some(&draw.decl.fragment_entry), targets: &targets, compilation_options: Default::default()
                    }), multiview_mask: None, cache: None,
                })
            }).collect(),
        };
        let formats: Vec<_> = canvas.colors.iter().copied().map(Some).collect();
        let mut enc = device.create_render_bundle_encoder(&RenderBundleEncoderDescriptor {
            color_formats: &formats, depth_stencil: canvas.depth_layout(), sample_count: canvas.samples, ..Default::default()
        });
        enc.set_bind_group(0, fixed, &[]);
        for (i,g) in groups.iter().enumerate() { enc.set_bind_group(i as u32 + 1, g.bound(), &[]); }
        for (draw, pipeline) in self.spec.draws.iter().zip(&pipelines) {
            enc.set_pipeline(pipeline);
            let args = tables[&draw.decl.args].bytes()?;
            let offset = u64::from(draw.decl.offset_bytes);
            match draw.index {
                Some((id, format)) => {
                    enc.set_index_buffer(tables[&id].bytes()?.slice(..), format);
                    enc.draw_indexed_indirect(args, offset);
                }
                None => enc.draw_indirect(args, offset),
            }
        }
        let bundle = enc.finish(&Default::default());
        self.cached = Some((canvas, pipelines, epoch, bundle));
        Ok(())
    }

    pub fn pass(&self, target: &Target<'_>, tables: &BTreeMap<TableId, Table>, written: &mut std::collections::BTreeSet<TableId>) -> command::Pass {
        let (colors, depth) = self.spec.targets.views(target, |id| tables.get(&id)?.attachment(), written);
        let bundle = self.cached.as_ref().expect("prepared raster pass").3.clone();
        command::Pass::Render { colors, depth, bundle, draws: self.spec.draws.len() as u32 }
    }
}
