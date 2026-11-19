use super::*;
use std::collections::{BTreeSet, HashMap};
use wgpu::util::DeviceExt;

pub(super) struct Group {
    pub index: u8,
    layout: wgpu::BindGroupLayout,
    pub slots: Vec<(u32, TableId)>,
    cached: Option<(u64, wgpu::BindGroup)>,
}
impl Group {
    fn resolve(
        &mut self,
        d: &wgpu::Device,
        tables: &BTreeMap<TableId, Table>,
        epoch: u64,
        images: &resources::Images,
    ) -> Result<&wgpu::BindGroup> {
        if self
            .cached
            .as_ref()
            .is_none_or(|(stamp, _)| self.slots.iter().any(|(_, id)| tables[id].epoch > *stamp))
        {
            let entries = self
                .slots
                .iter()
                .map(|(slot, id)| {
                    Ok(wgpu::BindGroupEntry {
                        binding: *slot,
                        resource: match (images.views.get(id), images.samplers.get(id)) {
                            (Some((view, _)), None) => wgpu::BindingResource::TextureView(view),
                            (None, Some(sampler)) => wgpu::BindingResource::Sampler(sampler),
                            (None, None) if self.index == 3 => {
                                return Err(RunnerError::Unsupported("external texture or sampler binding"));
                            }
                            (None, None) => tables[id].bytes()?.as_entire_binding(),
                            (Some(_), Some(_)) => return Err(RunnerError::AbiMismatch),
                        },
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            self.cached = Some((
                epoch,
                d.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &self.layout,
                    entries: &entries,
                }),
            ));
        }
        Ok(self.bound())
    }

    pub(super) fn bound(&self) -> &wgpu::BindGroup {
        &self.cached.as_ref().expect("bind group prepared").1
    }
}
/// A recorded draw retains its vertex and index handles even when all bind groups are current.
pub fn render_bundle_stale(
    draws: &[IndirectDraw],
    cached_epoch: u64,
    epoch: impl Fn(TableId) -> u64,
) -> bool {
    draws
        .iter()
        .flat_map(|draw| [draw.vertex, draw.index])
        .flatten()
        .any(|id| epoch(id) > cached_epoch)
}
pub(super) struct ComputeCache {
    fixed: wgpu::BindGroup,
    frame: wgpu::Buffer,
    pub(super) input: inputs::Inputs,
    input_buffer: Arc<wgpu::Buffer>,
    images: resources::Images,
    resources: Resources,
    passes: BTreeMap<PassId, (Run, Vec<Group>)>,
}
enum Run {
    Compute(wgpu::ComputePipeline, DispatchExtent),
    Render(embed::Render),
}

pub(super) struct Shader {
    pub handle: wgpu::ShaderModule,
    ir: naga::Module,
    info: naga::valid::ModuleInfo,
}

impl Shader {
    pub(super) fn reflect(
        &self,
        stage: naga::ShaderStage,
        entry: &str,
        pass: &PassDecl,
    ) -> Result<BTreeMap<(u32, u32), wgpu::BindGroupLayoutEntry>> {
        let index = self
            .ir
            .entry_points
            .iter()
            .position(|e| e.name == entry && e.stage == stage)
            .ok_or(RunnerError::AbiMismatch)?;
        crate::bindings::reflect(&self.ir, self.info.get_entry_point(index), pass)
    }
}

pub(super) fn slot(binding: u32, ty: wgpu::BufferBindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}
impl ComputeCache {
    pub(super) fn new(
        d: &wgpu::Device,
        b: &Bundle,
        tables: &BTreeMap<TableId, Table>,
    ) -> Result<Self> {
        use wgpu::BufferBindingType::{Storage, Uniform};
        use wgpu::{BufferUsages as U, ShaderStages as S};
        let images = resources::Images::new(d, b)?;
        embed::check_host_targets(&b.passes, &images)?;
        let fixed = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame, inputs, parameters"),
            entries: &[
                slot(0, Uniform),
                slot(1, Storage { read_only: true }),
                slot(2, Storage { read_only: true }),
            ]
            .map(|mut entry| {
                entry.visibility |= S::VERTEX_FRAGMENT;
                entry
            }),
        });
        let frame = d.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 256,
            usage: U::UNIFORM | U::COPY_DST,
            mapped_at_creation: false,
        });
        let input = inputs::Inputs::new(&b.resources.inputs, d.limits().max_storage_buffer_binding_size)?;
        let inputs = allocate(d, input.size());
        let mut params = vec![0; 16];
        for p in &b.params {
            let n = match p.ty {
                IrTy::F32 | IrTy::Bool | IrTy::U32 => 1,
                IrTy::Vec2 => 2,
                IrTy::Vec3 => 3,
                IrTy::Vec4 => 4,
            };
            if p.offset_bytes % 4 != 0 {
                return Err(RunnerError::AbiMismatch);
            }
            let start = p.offset_bytes as usize;
            let end = start
                .checked_add(n * 4)
                .filter(|&n| n as u64 <= u64::from(d.limits().max_storage_buffer_binding_size))
                .ok_or(RunnerError::Capacity)?;
            params.resize(params.len().max(end), 0);
            for (out, v) in params[start..end].chunks_exact_mut(4).zip(p.default) {
                let word = match p.ty {
                    IrTy::U32 => v as u32,
                    IrTy::Bool => u32::from(v != 0.),
                    _ => v.to_bits(),
                };
                out.copy_from_slice(&word.to_le_bytes());
            }
        }
        let params = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            usage: U::STORAGE | U::COPY_DST,
            contents: &params,
        });
        let g0 = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &fixed,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: frame.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: inputs.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params.as_entire_binding(),
                },
            ],
        });
        let mut modules = BTreeMap::new();
        for m in &b.modules {
            if m.target != ShaderTarget::Wgsl {
                return Err(RunnerError::Unsupported("shader target"));
            }
            let src = std::str::from_utf8(&m.bytes).map_err(|_| RunnerError::AbiMismatch)?;
            let ir = naga::front::wgsl::parse_str(src)
                .map_err(|e| RunnerError::InvalidShader(e.to_string()))?;
            let info = wgpu_naga_bridge::create_validator(
                d.features(),
                wgpu::DownlevelFlags::all(),
                naga::valid::ValidationFlags::all(),
            )
            .validate(&ir)
            .map_err(|e| RunnerError::InvalidShader(e.to_string()))?;
            let module = d.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(src.into()),
            });
            if modules
                .insert(
                    m.id,
                    Shader {
                        handle: module,
                        ir,
                        info,
                    },
                )
                .is_some()
            {
                return Err(RunnerError::AbiMismatch);
            }
        }
        let mut layouts = HashMap::new();
        let mut passes = BTreeMap::new();
        let mut ids = BTreeSet::new();
        for p in &b.passes {
            let mut slots = BTreeSet::new();
            if !ids.insert(p.id)
                || p.bindings.iter().any(|x| {
                    x.group != x.access.group()
                        || !tables.contains_key(&x.resource)
                        || !slots.insert((x.group, x.slot))
                })
            {
                return Err(RunnerError::AbiMismatch);
            }
            let mut link = |reflected: BTreeMap<_, wgpu::BindGroupLayoutEntry>, visibility| {
                let mut groups = Vec::new();
                for group in 1..=p.bindings.iter().map(|x| x.group).max().unwrap_or(0) {
                    let mut bindings: Vec<_> =
                        p.bindings.iter().filter(|x| x.group == group).collect();
                    bindings.sort_by_key(|x| x.slot);
                    let entries = bindings
                        .iter()
                        .map(|x| match reflected.get(&(u32::from(x.group), x.slot)) {
                            Some(e) => Ok(*e),
                            None if x.group <= 2 => {
                                let mut entry = slot(
                                    x.slot,
                                    Storage {
                                        read_only: x.access == BindAccess::ReadStorage,
                                    },
                                );
                                entry.visibility = visibility;
                                Ok(entry)
                            }
                            None => Err(RunnerError::AbiMismatch),
                        })
                        .collect::<Result<Vec<_>>>()?;
                    let layout = layouts.entry(entries.clone()).or_insert_with(|| {
                        d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                            label: None,
                            entries: &entries,
                        })
                    });
                    groups.push(Group {
                        index: group,
                        layout: layout.clone(),
                        slots: bindings.iter().map(|x| (x.slot, x.resource)).collect(),
                        cached: None,
                    });
                }
                let bind_group_layouts: Vec<_> = std::iter::once(Some(&fixed))
                    .chain(groups.iter().map(|g| Some(&g.layout)))
                    .collect();
                let layout = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &bind_group_layouts,
                    immediate_size: 0,
                });
                Ok::<_, RunnerError>((layout, groups))
            };
            let (run, groups) = match &p.run {
                PassRun::Compute {
                    module,
                    entry,
                    dispatch,
                } => {
                    let shader = modules.get(module).ok_or(RunnerError::UnknownResource)?;
                    let reflected = shader.reflect(naga::ShaderStage::Compute, entry, p)?;
                    match dispatch {
                        DispatchExtent::FromRowCount { table, workgroup_x }
                            if *workgroup_x == 0 || !tables.contains_key(table) =>
                        {
                            return Err(RunnerError::AbiMismatch);
                        }
                        DispatchExtent::Indirect { buf, offset_bytes }
                            if !offset_bytes.is_multiple_of(4) || !tables.contains_key(buf) =>
                        {
                            return Err(RunnerError::AbiMismatch);
                        }
                        _ => {}
                    }
                    let (layout, groups) = link(reflected, S::COMPUTE)?;
                    let pipeline = d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                        label: None,
                        layout: Some(&layout),
                        module: &shader.handle,
                        entry_point: Some(entry),
                        compilation_options: Default::default(),
                        cache: None,
                    });
                    (Run::Compute(pipeline, dispatch.clone()), groups)
                }
                PassRun::RenderBundle {
                    draws,
                    color_targets,
                    depth,
                    msaa,
                } => {
                    let (render, reflected) = embed::Render::new(
                        draws,
                        &modules,
                        tables,
                        color_targets,
                        depth.as_ref(),
                        *msaa,
                        p,
                        b.resources.cull_front.contains(&p.id),
                    )?;
                    let (layout, groups) = link(reflected, S::FRAGMENT)?;
                    (Run::Render(render(layout)), groups)
                }
                PassRun::ComputeFusedTicks { .. }
                | PassRun::PassLoop { .. }
                | PassRun::GenerateMips { .. } => continue,
            };
            passes.insert(p.id, (run, groups));
        }
        Ok(Self {
            fixed: g0,
            frame,
            input,
            input_buffer: inputs,
            images,
            resources: b.resources.clone(),
            passes,
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn encode(
        &mut self,
        d: &wgpu::Device,
        enc: &mut wgpu::CommandEncoder,
        passes: &[PassDecl],
        tables: &BTreeMap<TableId, Table>,
        epoch: u64,
        time: f64,
        dt: f32,
        target: &embed::Target<'_>,
    ) -> Result<()> {
        // Resolve the entire sequence before recording any command on the caller's encoder.
        for p in passes {
            let (run, groups) = self
                .passes
                .get_mut(&p.id)
                .ok_or(RunnerError::Unsupported("pass execution"))?;
            for g in groups.iter_mut() {
                g.resolve(d, tables, epoch, &self.images)?;
            }
            match run {
                Run::Compute(_, dispatch) => {
                    if let DispatchExtent::Indirect { buf, offset_bytes } = dispatch
                        && u64::from(*offset_bytes) + 12 > tables[buf].bytes()?.size()
                    {
                        return Err(RunnerError::Capacity);
                    }
                }
                Run::Render(render) => {
                    render.prepare(d, target, &self.fixed, &self.images, groups, tables, epoch)?
                }
            }
        }
        let validate_frame = |id: TableId| {
            let bytes = tables.get(&id).ok_or(RunnerError::UnknownResource)?.decl.initial.as_ref();
            if bytes.len() as u64 != self.frame.size() { return Err(RunnerError::AbiMismatch); }
            Ok(bytes)
        };
        let frame_data = self.resources.frame.as_ref().map(|f| {
            let offset = f.time_offset as usize;
            if offset.checked_add(4).is_none_or(|end| end as u64 > self.frame.size()) {
                return Err(RunnerError::AbiMismatch);
            }
            let main = validate_frame(f.main)?;
            let variants = f.variants.iter().map(|&(pass, table)| Ok((pass, validate_frame(table)?)))
                .collect::<Result<BTreeMap<_, _>>>()?;
            Ok((offset, main, variants))
        }).transpose()?;
        let resets = self.resources.resets.iter().map(|id| {
            let table = tables.get(id).ok_or(RunnerError::UnknownResource)?;
            Ok((table.bytes()?, table.decl.initial.as_ref()))
        }).collect::<Result<Vec<_>>>()?;
        let upload = |enc: &mut wgpu::CommandEncoder, bytes: &[u8], dst: &wgpu::Buffer| {
            let scratch = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None, contents: bytes, usage: wgpu::BufferUsages::COPY_SRC,
            });
            enc.copy_buffer_to_buffer(&scratch, 0, dst, 0, Some(bytes.len() as u64));
        };
        let tick = (time / f64::from(dt)).round() as u64;
        if let Some(bytes) = self.input.advance(tick)? {
            upload(enc, bytes, &self.input_buffer);
        }
        for (buf, initial) in resets {
            if !initial.is_empty() { upload(enc, initial, buf); }
        }
        let mut written = BTreeSet::new();
        let mut current = None;
        for command in frame_commands(passes, time, dt) {
            match command {
                FrameCommand::Upload(data) => {
                    if frame_data.is_none() { upload(enc, data.as_flattened(), &self.frame); }
                }
                FrameCommand::Pass(id) => {
                    if let Some((offset, main, variants)) = &frame_data {
                        let template = variants.get(&id).copied().unwrap_or(main);
                        if current != Some(template.as_ptr()) {
                            let mut bytes = template.to_vec();
                            bytes[*offset..*offset + 4].copy_from_slice(&(time as f32).to_le_bytes());
                            upload(enc, &bytes, &self.frame);
                            current = Some(template.as_ptr());
                        }
                    }
                    let (run, groups) = &self.passes[&id];
                    match run {
                        Run::Compute(pipeline, dispatch) => {
                            let mut pass = enc.begin_compute_pass(&Default::default());
                            pass.set_pipeline(pipeline);
                            pass.set_bind_group(0, &self.fixed, &[]);
                            for g in groups {
                                pass.set_bind_group(u32::from(g.index), g.bound(), &[]);
                            }
                            match dispatch {
                                DispatchExtent::Fixed([x, y, z]) => {
                                    pass.dispatch_workgroups(*x, *y, *z)
                                }
                                DispatchExtent::FromRowCount { table, workgroup_x } => pass
                                    .dispatch_workgroups(
                                        tables[table].active.div_ceil(*workgroup_x),
                                        1,
                                        1,
                                    ),
                                DispatchExtent::Indirect { buf, offset_bytes } => pass
                                    .dispatch_workgroups_indirect(
                                        tables[buf].bytes()?,
                                        u64::from(*offset_bytes),
                                    ),
                            }
                        }
                        Run::Render(render) => render.encode(enc, target, &self.images, &mut written),
                    }
                }
            }
        }
        Ok(())
    }
}

// The caller encoder accepts only an upload and the declared passes. It owns submission.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum FrameCommand {
    Upload([[u8; 4]; 2]),
    Pass(PassId),
}

pub(super) fn frame_commands(
    passes: &[PassDecl],
    time: f64,
    dt: f32,
) -> impl Iterator<Item = FrameCommand> + '_ {
    std::iter::once(FrameCommand::Upload([
        (time as f32).to_le_bytes(),
        dt.to_le_bytes(),
    ]))
    .chain(passes.iter().map(|pass| FrameCommand::Pass(pass.id)))
}
