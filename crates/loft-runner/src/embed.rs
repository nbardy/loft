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
    pub cached: Option<(Canvas, Vec<RenderPipeline>, u64, command::Raster)>,
}

impl Render {
    pub fn prepare(&mut self, device: &Device, target: &Target<'_>, fixed: &BindGroup,
                   groups: &[Group], tables: &BTreeMap<TableId, Table>, epoch: u64) -> super::Result<()> {
        let canvas = self.spec.targets.canvas(target, |id| tables.get(&id)?.attachment())?;
        let mut newest = groups.iter().flat_map(|g| &g.slots).map(|(_,id)| tables[id].epoch).max().unwrap_or(0);
        for draw in &self.spec.draws {
            // Bundles capture argument and index handles even when the shader has no binding for either.
            newest = newest.max(tables[&draw.decl.args].epoch);
            if let Some((id, _)) = draw.index { newest = newest.max(tables[&id].epoch); }
        }
        // Buffer usage and capacity are fixed for a resource epoch.
        if self.cached.as_ref().is_some_and(|(c,_,e,_)| *c == canvas && *e >= newest) { return Ok(()); }
        let sources = self.spec.draws.iter().map(|draw| {
            let args = tables[&draw.decl.args].bytes()?;
            let offset = u64::from(draw.decl.offset_bytes);
            let (source, size) = match draw.index {
                Some((id, format)) => {
                    let indices = tables[&id].bytes()?;
                    if !indices.usage().contains(BufferUsages::INDEX) { return Err(RunnerError::AbiMismatch); }
                    (command::DrawSource::Indexed { args, offset, indices, format }, 20)
                }
                None => (command::DrawSource::Indirect { args, offset }, 16),
            };
            if !args.usage().contains(BufferUsages::INDIRECT) { return Err(RunnerError::AbiMismatch); }
            if offset + size > args.size() { return Err(RunnerError::Capacity); }
            Ok(source)
        }).collect::<super::Result<Vec<_>>>()?;
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
        let fixed = std::iter::once(fixed).chain(groups.iter().map(Group::bound)).collect::<Vec<_>>();
        let draws = sources.into_iter().zip(&pipelines).map(|(source, pipeline)| command::Draw {
            pipeline, groups: vec![], source,
        });
        let raster = command::Raster::prepare(device, &RenderBundleEncoderDescriptor {
            color_formats: &formats, depth_stencil: canvas.depth_layout(), sample_count: canvas.samples, ..Default::default()
        }, &fixed, draws);
        self.cached = Some((canvas, pipelines, epoch, raster));
        Ok(())
    }

    pub fn pass(&self, target: &Target<'_>, tables: &BTreeMap<TableId, Table>, written: &mut std::collections::BTreeSet<TableId>) -> command::Pass {
        let (colors, depth) = self.spec.targets.views(target, |id| tables.get(&id)?.attachment(), written);
        self.cached.as_ref().expect("prepared raster pass").3.attach(colors, depth)
    }
}
