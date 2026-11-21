//! Cached raster pipelines and indirect draw bundles for the host's attachments.
use super::*;
use encode::Group;
use loft_wgpu::{attachments::Canvas, schedule::Raster};
pub(super) use loft_wgpu::command::RasterRecipe;
pub(super) use loft_wgpu::attachments::Target;
use wgpu::*;

pub(super) struct Render {
    pub spec: Raster,
    pub shaders: Vec<ShaderModule>,
    pub layout: PipelineLayout,
    pub additive: bool,
    pub override_pipeline: Option<RenderPipeline>,
    pub cached: Option<(Canvas, Vec<RenderPipeline>, u64, command::Raster)>,
}

impl Render {
    pub fn prepare(&mut self, device: &Device, target: &Target<'_>, fixed: &BindGroup,
                   groups: &[Group], tables: &BTreeMap<TableId, Table>, epoch: u64, pool: &[(RasterRecipe, RenderPipeline)]) -> super::Result<()> {
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
            if offset + size > interop::logical_size(&tables[&draw.decl.args].decl).min(args.size()) { return Err(RunnerError::Capacity); }
            Ok(source)
        }).collect::<super::Result<Vec<_>>>()?;
        let pipelines = match self.cached.take() {
            Some((old, pipelines, _, _)) if old == canvas => pipelines,
            _ => self.spec.draws.iter().zip(&self.shaders).map(|(draw, _)| {
                if let Some(pipeline) = &self.override_pipeline { return pipeline.clone(); }
                let recipe = self.draw_recipe(&canvas, draw);
                pool.iter().find(|(r, _)| r == &recipe).expect("prepared raster recipe").1.clone()
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

impl Render {
    fn draw_recipe(&self, canvas: &Canvas, draw: &loft_wgpu::schedule::Draw) -> RasterRecipe {
        RasterRecipe::new(canvas, &self.layout, draw, self.spec.cull_front, self.additive)
    }

    pub fn misses<'a>(&self, target: &Target<'_>, attachment: impl Fn(TableId) -> Option<(&'a TextureView, TextureDecl)>,
        recipes: &mut Vec<(RasterRecipe, ShaderModule)>) -> super::Result<()> {
        let canvas = self.spec.targets.canvas(target, attachment)?;
        if self.override_pipeline.is_some() || self.cached.as_ref().is_some_and(|(old, _, _, _)| *old == canvas) { return Ok(()); }
        for (draw, shader) in self.spec.draws.iter().zip(&self.shaders) {
            let recipe = self.draw_recipe(&canvas, draw);
            if !recipes.iter().any(|(old, _)| *old == recipe) { recipes.push((recipe, shader.clone())); }
        }
        Ok(())
    }

    pub fn recipe(&self) -> Option<(RasterRecipe, RenderPipeline)> {
        let (canvas, pipes, _, _) = self.cached.as_ref()?;
        let [draw] = self.spec.draws.as_slice() else { return None };
        Some((self.draw_recipe(canvas, draw), pipes[0].clone()))
    }
}
