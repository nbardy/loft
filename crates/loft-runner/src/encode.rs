use super::*;
use std::collections::{BTreeSet, HashMap};
use wgpu::util::DeviceExt;

pub(super) struct Group {
    pub index: u8,
    layout: wgpu::BindGroupLayout,
    slots: Vec<(wgpu::BindGroupLayoutEntry, TableId)>,
    cached: Option<(u64, wgpu::BindGroup)>,
}
impl Group {
    pub(super) fn bound(&self) -> &wgpu::BindGroup {
        &self.cached.as_ref().expect("prepared bind group").1
    }

    fn resolve(
        &mut self,
        d: &wgpu::Device,
        tables: &BTreeMap<TableId, Table>,
        epoch: u64,
        images: &resources::Images,
    ) -> Result<&wgpu::BindGroup> {
        if interop::stale(
            self.cached.as_ref().map(|(stamp, _)| *stamp),
            self.slots.iter().map(|(_, id)| tables[id].epoch),
        ) {
            let views = self
                .slots
                .iter()
                .map(|(slot, id)| match slot.ty {
                    _ if images.views.contains_key(id) => Ok(Some(images.views[id].0.clone())),
                    wgpu::BindingType::Buffer { .. } | wgpu::BindingType::Sampler(_) => Ok(None),
                    _ if slot.count.is_some() => {
                        Err(RunnerError::Unsupported("texture binding arrays"))
                    }
                    wgpu::BindingType::Texture { .. }
                    | wgpu::BindingType::StorageTexture { .. } => interop::texture_view(
                        tables[id]
                            .texture
                            .as_deref()
                            .ok_or(RunnerError::UnknownResource)?,
                        slot.ty,
                    )
                    .map(Some),
                    _ => Err(RunnerError::Unsupported("sampler resources")),
                })
                .collect::<Result<Vec<_>>>()?;
            let entries = self
                .slots
                .iter()
                .zip(&views)
                .map(|((slot, id), view)| {
                    let resource = match view {
                        Some(view) => wgpu::BindingResource::TextureView(view),
                        None if matches!(slot.ty, wgpu::BindingType::Sampler(_)) => {
                            wgpu::BindingResource::Sampler(images.samplers.get(id).ok_or(RunnerError::UnknownResource)?)
                        }
                        None => tables[id].bytes()?.as_entire_binding(),
                    };
                    Ok(wgpu::BindGroupEntry {
                        binding: slot.binding,
                        resource,
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
        Ok(&self.cached.as_ref().unwrap().1)
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
    param_buffer: wgpu::Buffer,
    pub(super) params: params::Params,
    pub(super) input: inputs::Inputs,
    input_buffer: Arc<wgpu::Buffer>,
    images: resources::Images,
    resources: Resources,
    render: BTreeMap<PassId, (embed::Render, Vec<Group>)>,
    passes: BTreeMap<PassId, ComputePass>,
    pub(super) fused: BTreeMap<PassId, super::stepping::Fused>,
    pub(super) outbox: Option<TableId>,
}
struct ComputePass {
    pipeline: wgpu::ComputePipeline,
    #[cfg(not(target_arch = "wasm32"))]
    layout: wgpu::PipelineLayout,
    groups: Vec<Group>,
}
pub(super) pub(super) struct Shader {
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
        use wgpu::BufferUsages as U;
        let images = resources::Images::new(d, b)?;
        embed::check_host_targets(&b.passes, &images)?;
        let fixed = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame, inputs, parameters"),
            entries: &[
                slot(0, Uniform),
                slot(1, Storage { read_only: true }),
                slot(2, Storage { read_only: true }),
            ],
        });
        let frame = d.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 256,
            usage: U::UNIFORM | U::COPY_DST,
            mapped_at_creation: false,
        });
        let input = inputs::Inputs::new(&b.resources.inputs, d.limits().max_storage_buffer_binding_size)?;
        let inputs = allocate(d, input.size());
        let params = params::Params::new(&b.params, d.limits().max_storage_buffer_binding_size)?;
        let param_buffer = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            usage: U::STORAGE | U::COPY_DST,
            contents: params.bytes(),
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
                    resource: param_buffer.as_entire_binding(),
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
            if modules.insert(m.id, Shader { handle: module, ir, info }).is_some() {
                return Err(RunnerError::AbiMismatch);
            }
        }
        let mut layouts = HashMap::new();
        let mut passes = BTreeMap::new();
        let mut ids = BTreeSet::new();
        let mut fused = BTreeMap::new();
        let mut render = BTreeMap::new();
        let mut outbox = None;
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
                        slots: entries.into_iter().zip(bindings.iter().map(|x| x.resource)).collect(),
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
            if let PassRun::RenderBundle { draws, color_targets, depth, msaa } = &p.run {
                let (build, reflected) = embed::Render::new(draws, &modules, tables, color_targets,
                    depth.as_ref(), *msaa, p, b.resources.cull_front.contains(&p.id))?;
                let (layout, groups) = link(reflected, S::FRAGMENT)?;
                render.insert(p.id, (build(layout), groups));
                continue;
            }
            let (module, entry) = match &p.run {
                PassRun::Compute { module, entry, .. }
                | PassRun::ComputeFusedTicks { module, entry, .. } => (module, entry),
                _ => continue,
            };
            let Shader { handle: shader, ir, info } = modules.get(module).ok_or(RunnerError::UnknownResource)?;
            let index = ir
                .entry_points
                .iter()
                .position(|e| e.name == *entry && e.stage == naga::ShaderStage::Compute)
                .ok_or(RunnerError::AbiMismatch)?;
            let reflected = crate::bindings::reflect(ir, info.get_entry_point(index), p)?;
            if matches!(p.run, PassRun::ComputeFusedTicks { .. }) {
                fused.insert(p.id, super::stepping::Fused::reflect(ir, p)?);
            }
            for (h, v) in ir.global_variables.iter() {
                if v.name.as_deref() == Some("loft_outbox")
                    && !info.get_entry_point(index)[h].is_empty()
                {
                    let address = v.binding.as_ref().ok_or(RunnerError::AbiMismatch)?;
                    let binding = p
                        .bindings
                        .iter()
                        .find(|b| {
                            u32::from(b.group) == address.group
                                && b.slot == address.binding
                                && b.access == BindAccess::ReadWriteStorage
                        })
                        .ok_or(RunnerError::AbiMismatch)?;
                    if outbox
                        .replace(binding.resource)
                        .is_some_and(|old| old != binding.resource)
                    {
                        return Err(RunnerError::AbiMismatch);
                    }
                }
            }
            match &p.run {
                PassRun::Compute {
                    dispatch: DispatchExtent::FromRowCount { table, workgroup_x },
                    ..
                } if *workgroup_x == 0 || !tables.contains_key(table) => {
                    return Err(RunnerError::AbiMismatch);
                }
                PassRun::Compute {
                    dispatch: DispatchExtent::Indirect { buf, offset_bytes },
                    ..
                } if !offset_bytes.is_multiple_of(4) || !tables.contains_key(buf) => {
                    return Err(RunnerError::AbiMismatch);
                }
                _ => {}
            }
            let (layout, groups) = link(reflected, S::COMPUTE)?;
            let pipeline = d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: None,
                layout: Some(&layout),
                module: shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            });
            passes.insert(
                p.id,
                ComputePass {
                    pipeline,
                    groups,
                    #[cfg(not(target_arch = "wasm32"))]
                    layout,
                },
            );
        }
        Ok(Self {
            fixed: g0,
            frame,
            input,
            input_buffer: inputs,
            images,
            resources: b.resources.clone(),
            render,
            param_buffer,
            params,
            passes,
            fused,
            outbox,
        })
    }
    pub(super) fn resource_bindings(
        &self,
        id: TableId,
    ) -> impl Iterator<Item = &wgpu::BindGroupLayoutEntry> {
        self.passes
            .values()
            .flat_map(|p| &p.groups)
            .chain(self.render.values().flat_map(|(_, groups)| groups))
            .flat_map(|g| &g.slots)
            .filter_map(move |(entry, table)| (*table == id).then_some(entry))
    }

    #[cfg(test)]
    pub(super) fn bound_groups(&self, pass: PassId) -> Vec<wgpu::BindGroup> {
        self.passes[&pass].groups.iter().map(|g| g.cached.as_ref().unwrap().1.clone()).collect()
    }

    pub(super) fn pipeline(&self, pass: PassId) -> Result<&wgpu::ComputePipeline> {
        self.passes
            .get(&pass)
            .map(|p| &p.pipeline)
            .ok_or(RunnerError::UnknownResource)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn pipeline_layout(&self, pass: PassId) -> Result<&wgpu::PipelineLayout> {
        self.passes
            .get(&pass)
            .map(|p| &p.layout)
            .ok_or(RunnerError::UnknownResource)
    }

    pub(super) fn replace_pipeline(
        &mut self,
        pass: PassId,
        pipeline: wgpu::ComputePipeline,
    ) -> Result<()> {
        self.passes
            .get_mut(&pass)
            .ok_or(RunnerError::UnknownResource)?
            .pipeline = pipeline;
        Ok(())
    }

    pub(super) fn keep_parameters(&mut self, previous: &Self) {
        self.params.copy_from(&previous.params);
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn encode(
        &mut self,
        d: &wgpu::Device,
        queue: &wgpu::Queue,
        enc: &mut wgpu::CommandEncoder,
        passes: &[PassDecl],
        tables: &BTreeMap<TableId, Table>,
        epoch: u64,
        time: f64,
        dt: f32,
        history: &mut super::stepping::History,
        checkpoints: &[Arc<wgpu::Buffer>],
        state: Option<TableId>,
        instance: InstanceId,
        to: u32,
        keep: u32,
        outbox: &mut Option<super::events::Outbox>,
        target: Option<&embed::Target<'_>>,
    ) -> Result<()> {
        // Validate and resolve the entire sequence before touching the caller's encoder.
        for p in passes {
            if let Some((render, groups)) = self.render.get_mut(&p.id) {
                let target = target.ok_or(RunnerError::Unsupported("render attachments are required"))?;
                for g in groups.iter_mut() {
                    g.resolve(d, tables, epoch, &self.images)?;
                }
                render.prepare(d, target, &self.fixed, &self.images, groups, tables, epoch)?;
                continue;
            }
            let step = self
                .passes
                .get_mut(&p.id)
                .ok_or(RunnerError::Unsupported("pass execution"))?;
            for g in &mut step.groups {
                g.resolve(d, tables, epoch, &self.images)?;
            }
            if self.fused.contains_key(&p.id) {
                self.check_fused(p.id, tables, to, history)?;
            }
            if let PassRun::Compute {
                dispatch: DispatchExtent::Indirect { buf, offset_bytes },
                ..
            } = &p.run
            {
                if u64::from(*offset_bytes) + 12 > tables[buf].bytes()?.size() {
                    return Err(RunnerError::Capacity);
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
        let event_buffer = self.outbox.map(|id| tables[&id].bytes()).transpose()?;
        if let Some(buffer) = event_buffer {
            if outbox.is_none() {
                *outbox = Some(super::events::Outbox::new(d, buffer.size())?);
            }
            let events = outbox.as_mut().unwrap();
            events.prepare()?;
            events.begin(d, enc, buffer)?;
        }
        if let Some(bytes) = self.params.take_upload() {
            queue.write_buffer(&self.param_buffer, 0, bytes);
        }
        let tick = (time / f64::from(dt)).round() as u64;
        if let Some(bytes) = self.input.advance(tick)? {
            upload(enc, bytes, &self.input_buffer);
        }
        for (buf, initial) in resets {
            if !initial.is_empty() { upload(enc, initial, buf); }
        }
        if frame_data.is_none() {
            let data = [(time as f32).to_le_bytes(), dt.to_le_bytes()];
            upload(enc, data.as_flattened(), &self.frame);
        }
        let mut written = BTreeSet::new();
        let mut current = None;
        for p in passes {
            if let Some((offset, main, variants)) = &frame_data {
                let template = variants.get(&p.id).copied().unwrap_or(main);
                if current != Some(template.as_ptr()) {
                    let mut bytes = template.to_vec();
                    bytes[*offset..*offset + 4].copy_from_slice(&(time as f32).to_le_bytes());
                    upload(enc, &bytes, &self.frame);
                    current = Some(template.as_ptr());
                }
            }
            if let Some((render, _)) = self.render.get(&p.id) {
                render.encode(enc, target.expect("validated render targets"), &self.images, &mut written);
                continue;
            }
            match &p.run {
                PassRun::Compute { dispatch, .. } => match *dispatch {
                    DispatchExtent::Fixed(size) => self.encode_dispatch(enc, p.id, size),
                    DispatchExtent::FromRowCount { table, workgroup_x } => self.encode_dispatch(
                        enc,
                        p.id,
                        [tables[&table].active.div_ceil(workgroup_x), 1, 1],
                    ),
                    DispatchExtent::Indirect { buf, offset_bytes } => {
                        self.dispatch_pass(enc, p.id).dispatch_workgroups_indirect(
                            tables[&buf].bytes()?,
                            u64::from(offset_bytes),
                        )
                    }
                },
                PassRun::ComputeFusedTicks { .. } => self.encode_fused(
                    d,
                    enc,
                    tables,
                    p.id,
                    instance,
                    to,
                    keep,
                    history,
                    checkpoints,
                    state,
                )?,
                _ => unreachable!("validated compute pass"),
            }
        }
        if let (Some(buffer), Some(events)) = (event_buffer, outbox) {
            events.end(enc, buffer);
        }
        Ok(())
    }

    pub(super) fn encode_dispatch(
        &self,
        enc: &mut wgpu::CommandEncoder,
        id: PassId,
        [x, y, z]: [u32; 3],
    ) {
        self.dispatch_pass(enc, id).dispatch_workgroups(x, y, z);
    }

    fn dispatch_pass<'a>(
        &self,
        enc: &'a mut wgpu::CommandEncoder,
        id: PassId,
    ) -> wgpu::ComputePass<'a> {
        let ComputePass {pipeline, groups, ..} = &self.passes[&id];
        let mut pass = enc.begin_compute_pass(&Default::default());
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &self.fixed, &[]);
        for (i, g) in groups.iter().enumerate() {
            pass.set_bind_group(i as u32 + 1, &g.cached.as_ref().unwrap().1, &[])
        }
        pass
    }
}

#[cfg(all(test,not(target_arch="wasm32")))]
mod edit_gate;
// The caller encoder accepts only an upload and the declared passes. It owns submission.
#[derive(Debug, PartialEq, Eq)]
#[cfg(test)]
pub(super) enum FrameCommand {
    Upload([[u8; 4]; 2]),
    Pass(PassId),
}

#[cfg(test)]
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
