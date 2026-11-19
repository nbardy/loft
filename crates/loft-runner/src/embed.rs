//! Render bundles targeting attachments owned by the host.
use super::*;
use encode::{Group, Shader};
use wgpu::{
    CompareFunction, LoadOp, Operations, RenderBundleDepthStencil, ShaderStages, StoreOp,
    TextureFormat, TextureView,
};

pub(super) struct Target<'a> {
    pub color: &'a TextureView,
    pub depth: Option<&'a TextureView>,
    pub caps: HostCaps,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Canvas {
    colors: Vec<TextureFormat>,
    depth: Option<TextureFormat>,
    samples: u32,
    comparison: CompareFunction,
    depth_write: bool,
}

impl Canvas {
    fn depth_state(&self) -> Option<wgpu::DepthStencilState> {
        self.depth.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: Some(self.depth_write),
            depth_compare: Some(self.comparison),
            stencil: Default::default(),
            bias: Default::default(),
        })
    }

    fn depth_layout(&self) -> Option<RenderBundleDepthStencil> {
        self.depth.map(|format| RenderBundleDepthStencil {
            format,
            depth_read_only: !self.depth_write,
            stencil_read_only: true,
        })
    }


    #[cfg(test)]
    fn color_ops(&self) -> Option<Operations<wgpu::Color>> {
        (!self.colors.is_empty()).then(preserve)
    }
    #[cfg(test)]
    fn depth_ops(&self) -> Option<Operations<f32>> {
        self.depth_write.then(preserve)
    }

}

#[cfg(test)]
fn preserve<T>() -> Operations<T> {
    Operations { load: LoadOp::Load, store: StoreOp::Store }
}

fn comparison(convention: DepthConvention) -> CompareFunction {
    match convention {
        DepthConvention::StandardZeroToOne => CompareFunction::Less,
        DepthConvention::ReverseOneToZero => CompareFunction::Greater,
    }
}

#[cfg(test)]
fn check_targets(passes: &[PassDecl]) -> Result<()> {
    let mut ids = std::collections::BTreeSet::new();
    for pass in passes {
        if let PassRun::RenderBundle { color_targets, depth, .. } = &pass.run {
            ids.extend(color_targets.iter().map(|t| t.table));
            ids.extend(depth.iter().map(|t| t.table));
        }
    }
    let bundle = Bundle { tables: ids.into_iter().map(|id| TableDecl {
        id, format: ElemFormat::U32, lifetime: TableLifetime::External, capacity: 0,
        stride_bytes: 4, initial: Box::default(),
    }).collect(), passes: passes.to_vec(), ..Bundle::default() };
    loft_std::bundle::Resources::new(&bundle)?;
    Ok(())
}

struct Draw {
    decl: IndirectDraw,
    shader: wgpu::ShaderModule,
    index_format: Option<wgpu::IndexFormat>,
}

pub(super) struct Render {
    draws: Vec<Draw>,
    layout: wgpu::PipelineLayout,
    colors: Box<[TargetDesc]>,
    depth: Option<DepthDesc>,
    samples: u8,
    cull_front: bool,
    cached: Option<(Canvas, Vec<wgpu::RenderPipeline>, u64, wgpu::RenderBundle)>,
}

impl Render {
    pub(super) fn new(
        draws: &[IndirectDraw],
        modules: &BTreeMap<ModuleId, Shader>,
        tables: &BTreeMap<TableId, Table>,
        colors: &[TargetDesc],
        depth: Option<&DepthDesc>,
        samples: u8,
        pass: &PassDecl,
        cull_front: bool,
    ) -> Result<(
        impl FnOnce(wgpu::PipelineLayout) -> Self,
        BTreeMap<(u32, u32), wgpu::BindGroupLayoutEntry>,
    )> {
        if (colors.is_empty() && depth.is_none()) || !matches!(samples, 1 | 4) {
            return Err(RunnerError::AbiMismatch);
        }
        let mut reflected = BTreeMap::new();
        let draws = draws
            .iter()
            .map(|draw| {
                if draw.vertex.is_some() {
                    return Err(RunnerError::Unsupported(
                        "vertex attributes require a layout; use storage vertex pulling",
                    ));
                }
                if !tables.contains_key(&draw.args) {
                    return Err(RunnerError::UnknownResource);
                }
                if !draw.offset_bytes.is_multiple_of(4) {
                    return Err(RunnerError::AbiMismatch);
                }
                let index_format = draw
                    .index
                    .map(|id| {
                        let index = tables.get(&id).ok_or(RunnerError::UnknownResource)?;
                        match (index.decl.format, index.decl.stride_bytes) {
                            (ElemFormat::U16, 4) => Ok(wgpu::IndexFormat::Uint16), // Two packed indices per word.
                            (ElemFormat::U32, 4) => Ok(wgpu::IndexFormat::Uint32),
                            _ => Err(RunnerError::AbiMismatch),
                        }
                    })
                    .transpose()?;
                let shader = modules
                    .get(&draw.module)
                    .ok_or(RunnerError::UnknownResource)?;
                let vertex = shader.reflect(naga::ShaderStage::Vertex, &draw.vertex_entry, pass)?;
                merge_bindings(&mut reflected, vertex, ShaderStages::VERTEX)?;
                if !colors.is_empty() || !draw.fragment_entry.is_empty() {
                    let fragment =
                        shader.reflect(naga::ShaderStage::Fragment, &draw.fragment_entry, pass)?;
                    merge_bindings(&mut reflected, fragment, ShaderStages::FRAGMENT)?;
                }
                Ok(Draw {
                    decl: draw.clone(),
                    shader: shader.handle.clone(),
                    index_format,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let (colors, depth) = (colors.into(), depth.cloned());
        Ok((
            move |layout| Self {
                draws,
                layout,
                colors,
                depth,
                samples,
                cull_front,
                cached: None,
            },
            reflected,
        ))
    }

    fn canvas(&self, target: &Target<'_>, tables: &BTreeMap<TableId, Table>) -> Result<Canvas> {
        let samples = u32::from(self.samples);
        let attachment = |view: &TextureView, depth: bool| {
            let texture = view.texture();
            let format = texture.format();
            if texture.sample_count() != samples
                || texture.dimension() != wgpu::TextureDimension::D2
                || !texture
                    .usage()
                    .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
                || if depth {
                    !format.has_depth_aspect()
                } else {
                    format.is_depth_stencil_format()
                }
            {
                return Err(RunnerError::AbiMismatch);
            }
            Ok(format)
        };
        let colors = self.colors.iter().map(|t| {
            let view = tables.get(&t.table).and_then(Table::attachment).map_or(target.color, |(view, _)| view);
            attachment(view, false)
        }).collect::<Result<Vec<_>>>()?;
        let depth_view = self.depth.as_ref().map(|t| {
            tables.get(&t.table).and_then(Table::attachment).map(|(view, _)| view).or(target.depth)
                .ok_or(RunnerError::AbiMismatch)
        }).transpose()?;
        let compare = if self.depth.as_ref().is_some_and(|t| tables.get(&t.table).and_then(Table::attachment).is_some()) {
            CompareFunction::Less
        } else {
            comparison(target.caps.depth_convention)
        };
        if self.colors.iter().any(|t| tables.get(&t.table).and_then(Table::attachment).is_none())
            && self.samples != target.caps.msaa_samples {
            return Err(RunnerError::AbiMismatch);
        }
        Ok(Canvas { colors, depth: depth_view.map(|view| attachment(view, true)).transpose()?,
            samples, comparison: compare, depth_write: self.depth.as_ref().is_some_and(|t| t.write) })
    }

    pub(super) fn prepare(
        &mut self,
        d: &wgpu::Device,
        target: &Target<'_>,
        fixed: &wgpu::BindGroup,
        groups: &[Group],
        tables: &BTreeMap<TableId, Table>,
        epoch: u64,
    ) -> Result<()> {
        let canvas = self.canvas(target, tables)?;
        for draw in &self.draws {
            let args = tables[&draw.decl.args].bytes()?;
            let bytes = if let Some(id) = draw.decl.index {
                if !tables[&id]
                    .bytes()?
                    .usage()
                    .contains(wgpu::BufferUsages::INDEX)
                {
                    return Err(RunnerError::AbiMismatch);
                }
                20
            } else {
                16
            };
            if !args.usage().contains(wgpu::BufferUsages::INDIRECT) {
                return Err(RunnerError::AbiMismatch);
            }
            if u64::from(draw.decl.offset_bytes) + bytes > args.size() {
                return Err(RunnerError::Capacity);
            }
        }
        if let Some((previous, _, recorded, _)) = &self.cached
            && *previous == canvas
            && !groups.iter().flat_map(|g| &g.slots).any(|(_, id)| tables[id].epoch > *recorded)
            // An indirect draw also captures its argument buffer, even if no shader binds that table.
            && !self.draws.iter().any(|draw| {
                [Some(draw.decl.args), draw.decl.index].into_iter().flatten()
                    .any(|id| tables[&id].epoch > *recorded)
            })
        {
            return Ok(());
        }
        let (formats, colors): (Vec<_>, Vec<_>) = canvas
            .colors
            .iter().copied()
            .map(|format| {
                (
                    Some(format),
                    Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                )
            })
            .unzip();
        let pipelines = match self.cached.take() {
            Some((previous, pipelines, _, _)) if previous == canvas => pipelines,
            _ => self
                .draws
                .iter()
                .map(|draw| {
                    d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: None,
                        layout: Some(&self.layout),
                        vertex: wgpu::VertexState {
                            module: &draw.shader,
                            entry_point: Some(&draw.decl.vertex_entry),
                            buffers: &[],
                            compilation_options: Default::default(),
                        },
                        primitive: wgpu::PrimitiveState {
                            cull_mode: self.cull_front.then_some(wgpu::Face::Front),
                            ..Default::default()
                        },
                        depth_stencil: canvas.depth_state(),
                        multisample: wgpu::MultisampleState {
                            count: canvas.samples,
                            ..Default::default()
                        },
                        fragment: (!draw.decl.fragment_entry.is_empty()).then_some(
                            wgpu::FragmentState {
                                module: &draw.shader,
                                entry_point: Some(&draw.decl.fragment_entry),
                                targets: &colors,
                                compilation_options: Default::default(),
                            },
                        ),
                        multiview_mask: None,
                        cache: None,
                    })
                })
                .collect(),
        };
        let mut encoder = d.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
            color_formats: &formats,
            depth_stencil: canvas.depth_layout(),
            sample_count: canvas.samples,
            ..Default::default()
        });
        for (draw, pipeline) in self.draws.iter().zip(&pipelines) {
            encoder.set_pipeline(pipeline);
            encoder.set_bind_group(0, fixed, &[]);
            for group in groups {
                encoder.set_bind_group(u32::from(group.index), group.bound(), &[]);
            }
            let args = tables[&draw.decl.args].bytes()?;
            let offset = u64::from(draw.decl.offset_bytes);
            match draw.decl.index.zip(draw.index_format) {
                Some((id, format)) => {
                    encoder.set_index_buffer(tables[&id].bytes()?.slice(..), format);
                    encoder.draw_indexed_indirect(args, offset);
                }
                None => encoder.draw_indirect(args, offset),
            }
        }
        let recorded = encoder.finish(&Default::default());
        self.cached = Some((canvas, pipelines, epoch, recorded));
        Ok(())
    }

    pub(super) fn encode(
        &self, enc: &mut wgpu::CommandEncoder, target: &Target<'_>, tables: &BTreeMap<TableId, Table>,
        written: &mut std::collections::BTreeSet<TableId>,
    ) {
        let (_, _, _, bundle) = self.cached.as_ref().expect("render prepared");
        // Internal targets are cleared on their first writer each frame. Caller targets always load.
        let color: Vec<_> = self.colors.iter().map(|t| {
            let (view, clear) = tables.get(&t.table).and_then(Table::attachment)
                .map_or((target.color, None), |(view, decl)| (view, decl.clear));
            let load = match clear.filter(|_| written.insert(t.table)) {
                Some([r, g, b, a]) => LoadOp::Clear(wgpu::Color {
                    r, g, b, a,
                }),
                None => LoadOp::Load,
            };
            Some(wgpu::RenderPassColorAttachment {
                view, depth_slice: None, resolve_target: None,
                ops: Operations { load, store: StoreOp::Store },
            })
        }).collect();
        let depth = self.depth.as_ref().map(|t| {
            let (view, clear) = match tables.get(&t.table).and_then(Table::attachment) {
                Some((view, decl)) => (view, decl.clear),
                None => (target.depth.expect("validated host depth"), None),
            };
            let depth_ops = t.write.then(|| Operations {
                load: clear.filter(|_| written.insert(t.table)).map_or(LoadOp::Load, |c| LoadOp::Clear(c[0] as f32)),
                store: StoreOp::Store,
            });
            wgpu::RenderPassDepthStencilAttachment { view, depth_ops, stencil_ops: None }
        });
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: color.as_slice(),
            depth_stencil_attachment: depth,
            ..Default::default()
        });
        pass.execute_bundles([bundle]);
    }
}

fn merge_bindings(
    reflected: &mut BTreeMap<(u32, u32), wgpu::BindGroupLayoutEntry>,
    stage: BTreeMap<(u32, u32), wgpu::BindGroupLayoutEntry>,
    visibility: ShaderStages,
) -> Result<()> {
    for (address, mut entry) in stage {
        entry.visibility = visibility;
        let merged = reflected.entry(address).or_insert(entry);
        if merged.ty != entry.ty || merged.count != entry.count {
            return Err(RunnerError::AbiMismatch);
        }
        merged.visibility |= visibility;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode::{FrameCommand, frame_commands};

    fn pass(id: u32, run: PassRun) -> PassDecl {
        PassDecl {
            id: PassId(id),
            wave: 0,
            queue: QueueClass::Graphics,
            bindings: Box::default(),
            run,
        }
    }

    fn render(color: u32, depth: u32) -> PassRun {
        PassRun::RenderBundle {
            draws: Box::default(),
            color_targets: [TargetDesc {
                table: TableId(color),
            }]
            .into(),
            depth: Some(DepthDesc {
                table: TableId(depth),
                write: true,
            }),
            msaa: 1,
        }
    }

    #[test]
    fn encode_records_only_an_upload_and_declared_passes() {
        let passes = [
            pass(
                7,
                PassRun::Compute {
                    module: ModuleId(1),
                    entry: "update".into(),
                    dispatch: DispatchExtent::Fixed([1, 1, 1]),
                },
            ),
            pass(3, render(2, 3)),
        ];
        check_targets(&passes).unwrap();
        assert_eq!(
            frame_commands(&passes, 2., 0.125).collect::<Vec<_>>(),
            [
                FrameCommand::Upload([2f32.to_le_bytes(), 0.125f32.to_le_bytes()]),
                FrameCommand::Pass(PassId(7)),
                FrameCommand::Pass(PassId(3)),
            ]
        );
    }

    #[test]
    fn host_color_and_depth_are_preserved_and_occlude_in_both_directions() {
        // CPU twin of the attachment load and depth test on one covered pixel.
        fn load<T: Copy>(ops: Operations<T>, old: T) -> T {
            assert_eq!(ops.store, StoreOp::Store);
            match ops.load {
                LoadOp::Load => old,
                op => panic!(
                    "host attachment was not loaded: {:?}",
                    std::mem::discriminant(&op)
                ),
            }
        }

        let host_color = wgpu::Color::GREEN;
        for (convention, near, far) in [
            (DepthConvention::StandardZeroToOne, 0.25, 0.75),
            (DepthConvention::ReverseOneToZero, 0.75, 0.25),
        ] {
            for samples in [1, 4] {
                for depth_write in [false, true] {
                    let canvas = Canvas {
                        colors: vec![TextureFormat::Rgba8Unorm],
                        depth: Some(TextureFormat::Depth32Float),
                        samples,
                        comparison: comparison(convention),
                        depth_write,
                    };
                    let state = canvas.depth_state().unwrap();
                    let layout = canvas.depth_layout().unwrap();
                    assert_eq!(layout.depth_read_only, !depth_write);
                    assert!(layout.stencil_read_only);
                    assert_eq!(state.depth_write_enabled, Some(depth_write));
                    assert_eq!(canvas.depth_ops().is_some(), depth_write);
                    for (z, visible) in [(near, true), (far, false), (0.5, false)] {
                        let old_z = canvas.depth_ops().map_or(0.5, |ops| load(ops, 0.5));
                        let passes = match state.depth_compare.unwrap() {
                            CompareFunction::Less => z < old_z,
                            CompareFunction::Greater => z > old_z,
                            other => panic!("unexpected host depth test {other:?}"),
                        };
                        let old_color = load(canvas.color_ops().unwrap(), host_color);
                        let color = if passes { wgpu::Color::RED } else { old_color };
                        let depth = if passes && depth_write { z } else { old_z };
                        assert_eq!(
                            color,
                            if visible {
                                wgpu::Color::RED
                            } else {
                                host_color
                            }
                        );
                        assert_eq!(depth, if visible && depth_write { near } else { 0.5 });
                    }
                }
            }
        }
    }

    #[test]
    fn distinct_logical_targets_cannot_alias_the_host_views() {
        let passes = [pass(0, render(1, 2)), pass(1, render(1, 2))];
        check_targets(&passes).unwrap();
        for next in [render(3, 2), render(1, 3)] {
            assert!(matches!(
                check_targets(&[passes[0].clone(), pass(1, next)]),
                Err(RunnerError::Unsupported(_))
            ));
        }
        assert_eq!(
            check_targets(&[pass(0, render(1, 1))]),
            Err(RunnerError::AbiMismatch)
        );
    }

    #[test]
    fn render_shader_validates_and_reflects_storage_in_both_stages() {
        let shader = naga::front::wgsl::parse_str(
            r#"
            @group(1) @binding(5) var<storage, read> data: array<vec4f>;
            @vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
                return data[i];
            }
            @fragment fn fs() -> @location(0) vec4f {
                return data[3];
            }
        "#,
        )
        .unwrap();
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&shader)
        .unwrap();
        let mut decl = pass(1, render(2, 3));
        decl.bindings = [PassBinding {
            group: 1,
            slot: 5,
            resource: TableId(1),
            access: BindAccess::ReadStorage,
        }]
        .into();
        let mut reflected = BTreeMap::new();
        for (i, stage) in [ShaderStages::VERTEX, ShaderStages::FRAGMENT]
            .into_iter()
            .enumerate()
        {
            let entries =
                crate::bindings::reflect(&shader, info.get_entry_point(i), &decl).unwrap();
            merge_bindings(&mut reflected, entries, stage).unwrap();
        }
        assert_eq!(reflected[&(1, 5)].visibility, ShaderStages::VERTEX_FRAGMENT);
    }
}
