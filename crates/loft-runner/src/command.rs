//! A prepared device pass. All handles are retained until the pass is recorded.
use wgpu::*;
use std::ops::Range;

pub use loft_std::frame::{Stamps, Work};

pub struct Dispatch {
    pub pipeline: ComputePipeline,
    pub groups: Vec<BindGroup>,
    pub work: Work,
}

pub use loft_std::attachments::{Color, Depth};

pub enum DrawSource<'a> {
    Direct { vertices: Range<u32>, instances: Range<u32> },
    Indirect { args: &'a Buffer, offset: u64 },
    Indexed { args: &'a Buffer, offset: u64, indices: &'a Buffer, format: IndexFormat },
}

pub struct Draw<'a> {
    pub pipeline: &'a RenderPipeline,
    pub groups: Vec<&'a BindGroup>,
    pub source: DrawSource<'a>,
}

#[derive(Clone)]
pub struct Raster {
    bundle: RenderBundle,
    draws: u32,
}

impl Raster {
    pub fn prepare<'a>(
        device: &Device, desc: &RenderBundleEncoderDescriptor<'_>, fixed: &[&BindGroup],
        draws: impl IntoIterator<Item = Draw<'a>>,
    ) -> Self {
        let mut enc = device.create_render_bundle_encoder(desc);
        for (i, group) in fixed.iter().enumerate() {
            enc.set_bind_group(i as u32, *group, &[]);
        }
        let mut count = 0;
        for draw in draws {
            enc.set_pipeline(draw.pipeline);
            for (i, group) in draw.groups.iter().enumerate() {
                enc.set_bind_group((fixed.len() + i) as u32, *group, &[]);
            }
            match draw.source {
                DrawSource::Direct { vertices, instances } => enc.draw(vertices, instances),
                DrawSource::Indirect { args, offset } => enc.draw_indirect(args, offset),
                DrawSource::Indexed { args, offset, indices, format } => {
                    enc.set_index_buffer(indices.slice(..), format);
                    enc.draw_indexed_indirect(args, offset);
                }
            }
            count += 1;
        }
        let bundle = enc.finish(&RenderBundleDescriptor { label: desc.label });
        Self { bundle, draws: count }
    }

    pub fn attach(&self, colors: Vec<Color>, depth: Option<Depth>) -> Pass {
        Pass::Render { colors, depth, raster: self.clone() }
    }
}

pub enum Pass {
    Compute(Vec<Dispatch>),
    Render { colors: Vec<Color>, depth: Option<Depth>, raster: Raster },
}

impl Pass {
    pub fn record(&self, encoder: &mut CommandEncoder, stamps: Option<Stamps<'_>>) -> u32 {
        match self {
            Self::Compute(kernels) => record_compute(encoder, kernels, stamps),
            Self::Render { colors, depth, raster } => {
                record_render(encoder, colors, depth.as_ref(), &raster.bundle, stamps);
                raster.draws
            }
        }
    }
}

fn record_compute(enc: &mut CommandEncoder, kernels: &[Dispatch], stamps: Option<Stamps<'_>>) -> u32 {
    let timestamp_writes = stamps.map(|s| ComputePassTimestampWrites {
        query_set: s.set, beginning_of_pass_write_index: s.start, end_of_pass_write_index: s.end,
    });
    let mut pass = enc.begin_compute_pass(&ComputePassDescriptor { label: None, timestamp_writes });
    for kernel in kernels {
        pass.set_pipeline(&kernel.pipeline);
        for (i, group) in kernel.groups.iter().enumerate() {
            pass.set_bind_group(i as u32, group, &[]);
        }
        match &kernel.work {
            Work::Direct([x, y, z]) => pass.dispatch_workgroups(*x, *y, *z),
            Work::Indirect(buffer, offset) => pass.dispatch_workgroups_indirect(buffer, *offset),
        }
    }
    kernels.len() as u32
}

fn record_render(enc: &mut CommandEncoder, colors: &[Color], depth: Option<&Depth>,
                 bundle: &RenderBundle, stamps: Option<Stamps<'_>>) {
    let colors: Vec<_> = colors.iter().map(|c| Some(RenderPassColorAttachment {
        view: &c.view, ops: c.ops, resolve_target: None, depth_slice: None,
    })).collect();
    let depth_stencil_attachment = depth.map(|d| RenderPassDepthStencilAttachment {
        view: &d.view, depth_ops: d.ops, stencil_ops: None,
    });
    let timestamp_writes = stamps.map(|s| RenderPassTimestampWrites {
        query_set: s.set, beginning_of_pass_write_index: s.start, end_of_pass_write_index: s.end,
    });
    let mut pass = enc.begin_render_pass(&RenderPassDescriptor {
        color_attachments: &colors, depth_stencil_attachment, timestamp_writes, ..Default::default()
    });
    pass.execute_bundles([bundle]);
}
