use super::*;
use std::collections::{BTreeSet, HashMap};
use wgpu::util::DeviceExt;

pub(super) struct Group {
    pub index: u8,
    layout: wgpu::BindGroupLayout,
    pub(super) slots: Vec<(wgpu::BindGroupLayoutEntry, TableId)>,
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
    ) -> Result<&wgpu::BindGroup> {
        if interop::stale(
            self.cached.as_ref().map(|(stamp, _)| *stamp),
            self.slots.iter().map(|(_, id)| tables[id].epoch),
        ) {
            let handles = self
                .slots
                .iter()
                .map(|(slot, id)| tables[id].resource.bind(slot))
                .collect::<Result<Vec<_>>>()?;
            let entries: Vec<_> = self
                .slots
                .iter()
                .zip(&handles)
                .map(|((slot, _), handle)| wgpu::BindGroupEntry {
                    binding: slot.binding,
                    resource: handle.binding(),
                })
                .collect();
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
    resources: Resources,
    roots: Vec<PassId>,
    passes: BTreeMap<PassId, Command>,
    pub(super) outbox: Option<TableId>,
}
struct ComputePass {
    pipeline: wgpu::ComputePipeline,
    #[cfg(not(target_arch = "wasm32"))]
    layout: wgpu::PipelineLayout,
    groups: Vec<Group>,
}
enum Command {
    Dispatch(ComputePass, DispatchExtent),
    Ticks(ComputePass, stepping::Fused),
    Draw(Box<embed::Render>, Vec<Group>),
    Loop { count: u32, children: Vec<PassId> },
    Mips(TableId, Option<mips::Chain>),
}

impl Command {
    fn groups(&self) -> &[Group] {
        match self {
            Self::Dispatch(pass, _) | Self::Ticks(pass, _) => &pass.groups,
            Self::Draw(_, groups) => groups,
            Self::Loop { .. } | Self::Mips(..) => &[],
        }
    }
    fn compute(&mut self) -> Option<&mut ComputePass> {
        match self {
            Self::Dispatch(pass, _) | Self::Ticks(pass, _) => Some(pass),
            Self::Draw(..) | Self::Loop { .. } | Self::Mips(..) => None,
        }
    }
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
        let input = inputs::Inputs::new(
            &b.resources.inputs,
            d.limits().max_storage_buffer_binding_size,
        )?;
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
        if !loft_std::bundle::distinct_passes(&b.passes) {
            return Err(RunnerError::AbiMismatch);
        }
        let mut outbox = None;
        for p in loft_std::bundle::passes(&b.passes) {
            let mut slots = BTreeSet::new();
            if p.bindings.iter().any(|x| {
                x.group != x.access.group()
                    || !tables.contains_key(&x.resource)
                    || !slots.insert((x.group, x.slot))
            }) {
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
                        slots: entries
                            .into_iter()
                            .zip(bindings.iter().map(|x| x.resource))
                            .collect(),
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
            let (module, entry) = match &p.run {
                PassRun::Compute { module, entry, .. }
                | PassRun::ComputeFusedTicks { module, entry, .. } => (module, entry),
                PassRun::RenderBundle {
                    draws,
                    color_targets,
                    depth,
                    msaa,
                } => {
                    let (build, reflected) = embed::Render::new(
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
                    passes.insert(p.id, Command::Draw(Box::new(build(layout)), groups));
                    continue;
                }
                PassRun::PassLoop {
                    max_iters,
                    cond_buf,
                    ..
                } => {
                    if cond_buf.is_some() {
                        return Err(RunnerError::Unsupported(
                            "conditional loop requires shader lowering",
                        ));
                    }
                    passes.insert(
                        p.id,
                        Command::Loop {
                            count: *max_iters,
                            children: loft_std::bundle::children(&p.run)
                                .iter()
                                .map(|p| p.id)
                                .collect(),
                        },
                    );
                    continue;
                }
                PassRun::GenerateMips { texture } => {
                    if !matches!(tables[texture].resource, Resource::Texture { .. }) {
                        return Err(RunnerError::Unsupported(
                            "mip generation requires a declared texture mip chain",
                        ));
                    }
                    passes.insert(p.id, Command::Mips(*texture, None));
                    continue;
                }
            };
            let Shader {
                handle: shader,
                ir,
                info,
            } = modules.get(module).ok_or(RunnerError::UnknownResource)?;
            let index = ir
                .entry_points
                .iter()
                .position(|e| e.name == *entry && e.stage == naga::ShaderStage::Compute)
                .ok_or(RunnerError::AbiMismatch)?;
            let reflected = crate::bindings::reflect(ir, info.get_entry_point(index), p)?;
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
            let pass = ComputePass {
                pipeline,
                groups,
                #[cfg(not(target_arch = "wasm32"))]
                layout,
            };
            let command = match &p.run {
                PassRun::Compute { dispatch, .. } => Command::Dispatch(pass, dispatch.clone()),
                PassRun::ComputeFusedTicks { .. } => {
                    Command::Ticks(pass, stepping::Fused::reflect(ir, p)?)
                }
                _ => unreachable!("handled during preparation"),
            };
            passes.insert(p.id, command);
        }
        Ok(Self {
            fixed: g0,
            frame,
            input,
            input_buffer: inputs,
            resources: b.resources.clone(),
            param_buffer,
            params,
            roots: b.passes.iter().map(|p| p.id).collect(),
            passes,
            outbox,
        })
    }
    pub(super) fn resource_bindings(
        &self,
        id: TableId,
    ) -> impl Iterator<Item = &wgpu::BindGroupLayoutEntry> {
        self.passes
            .values()
            .flat_map(Command::groups)
            .flat_map(|g| &g.slots)
            .filter_map(move |(entry, table)| (*table == id).then_some(entry))
    }

    #[cfg(test)]
    pub(super) fn bound_groups(&self, pass: PassId) -> Vec<wgpu::BindGroup> {
        self.passes[&pass]
            .groups()
            .iter()
            .map(|g| g.cached.as_ref().unwrap().1.clone())
            .collect()
    }

    pub(super) fn pipeline(&self, pass: PassId) -> Result<&wgpu::ComputePipeline> {
        self.passes
            .get(&pass)
            .and_then(|p| match p {
                Command::Dispatch(pass, _) | Command::Ticks(pass, _) => Some(&pass.pipeline),
                _ => None,
            })
            .ok_or(RunnerError::UnknownResource)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn pipeline_layout(&self, pass: PassId) -> Result<&wgpu::PipelineLayout> {
        self.passes
            .get(&pass)
            .and_then(|p| match p {
                Command::Dispatch(pass, _) | Command::Ticks(pass, _) => Some(&pass.layout),
                _ => None,
            })
            .ok_or(RunnerError::UnknownResource)
    }

    pub(super) fn replace_pipeline(
        &mut self,
        pass: PassId,
        pipeline: wgpu::ComputePipeline,
    ) -> Result<()> {
        self.passes
            .get_mut(&pass)
            .and_then(Command::compute)
            .ok_or(RunnerError::UnknownResource)?
            .pipeline = pipeline;
        Ok(())
    }

    pub(super) fn keep_parameters(&mut self, previous: &Self) {
        self.params.copy_from(&previous.params);
    }

    pub(super) fn fused(&self, id: PassId) -> Option<&stepping::Fused> {
        match self.passes.get(&id)? {
            Command::Ticks(_, fused) => Some(fused),
            _ => None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn encode(
        &mut self,
        d: &wgpu::Device,
        enc: &mut wgpu::CommandEncoder,
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
        timer: Option<&wgpu::QuerySet>,
    ) -> Result<()> {
        if let Some(timer) = timer {
            let [id] = self.roots.as_slice() else {
                return Err(RunnerError::Unsupported(
                    "timed advance requires one fixed-step pass",
                ));
            };
            if self
                .fused(*id)
                .is_none_or(|pass| tables[&pass.rows].active == 0)
            {
                return Err(RunnerError::Unsupported(
                    "timed advance requires active fixed-step rows",
                ));
            }
            if !matches!(timer.ty(), wgpu::QueryType::Timestamp) || timer.count() < 2 {
                return Err(RunnerError::AbiMismatch);
            }
        }
        // Resolve every resource-dependent command before recording into the caller's encoder.
        let limit = d.limits().max_compute_workgroups_per_dimension;
        for (&id, command) in &mut self.passes {
            let groups = match command {
                Command::Dispatch(pass, extent) => {
                    validate_dispatch(extent, tables, limit)?;
                    &mut pass.groups
                }
                Command::Ticks(pass, fused) => {
                    stepping::check_fused(fused, id, tables, to, history, limit)?;
                    &mut pass.groups
                }
                Command::Draw(draw, groups) => {
                    let target = target
                        .ok_or(RunnerError::Unsupported("render attachments are required"))?;
                    for group in groups.iter_mut() {
                        group.resolve(d, tables, epoch)?;
                    }
                    draw.prepare(d, target, &self.fixed, groups, tables, epoch)?;
                    continue;
                }
                Command::Mips(id, chain) => {
                    mips::Chain::prepare(d, &tables[id], chain)?;
                    continue;
                }
                Command::Loop { .. } => continue,
            };
            for group in groups {
                group.resolve(d, tables, epoch)?;
            }
        }
        let validate_frame = |id: TableId| {
            let bytes = tables
                .get(&id)
                .ok_or(RunnerError::UnknownResource)?
                .decl
                .initial
                .as_ref();
            if bytes.len() as u64 != self.frame.size() {
                return Err(RunnerError::AbiMismatch);
            }
            Ok(bytes)
        };
        let frame_data = self
            .resources
            .frame
            .as_ref()
            .map(|f| {
                let offset = f.time_offset as usize;
                if offset
                    .checked_add(4)
                    .is_none_or(|end| end as u64 > self.frame.size())
                {
                    return Err(RunnerError::AbiMismatch);
                }
                let main = validate_frame(f.main)?;
                let variants = f
                    .variants
                    .iter()
                    .map(|&(pass, table)| Ok((pass, validate_frame(table)?)))
                    .collect::<Result<BTreeMap<_, _>>>()?;
                Ok((offset, main, variants))
            })
            .transpose()?;
        let resets = self
            .resources
            .resets
            .iter()
            .map(|id| {
                let table = tables.get(id).ok_or(RunnerError::UnknownResource)?;
                Ok((table.bytes()?, table.decl.initial.as_ref()))
            })
            .collect::<Result<Vec<_>>>()?;
        let tick = (time / f64::from(dt)).round() as u64;
        let inputs = self.input.advance(tick)?;
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
            upload(d, enc, bytes, &self.param_buffer, 0);
        }
        if let Some(bytes) = inputs {
            upload(d, enc, bytes, &self.input_buffer, 0);
        }
        for (buf, initial) in resets {
            if !initial.is_empty() {
                upload(d, enc, initial, buf, 0);
            }
        }
        if frame_data.is_none() {
            let data = [(time as f32).to_le_bytes(), dt.to_le_bytes()];
            upload(d, enc, data.as_flattened(), &self.frame, 0);
        }
        let mut written = BTreeSet::new();
        let mut current = None;
        let mut before_pass = |id, enc: &mut wgpu::CommandEncoder| {
            if let Some((offset, main, variants)) = &frame_data {
                let template = variants.get(&id).copied().unwrap_or(main);
                if current != Some(template.as_ptr()) {
                    let mut bytes = template.to_vec();
                    bytes[*offset..*offset + 4].copy_from_slice(&(time as f32).to_le_bytes());
                    upload(d, enc, &bytes, &self.frame, 0);
                    current = Some(template.as_ptr());
                }
            }
        };
        let mut frame = Recording {
            device: d,
            enc,
            tables,
            target,
            written: &mut written,
            history,
            checkpoints,
            state,
            instance,
            to,
            keep,
            before_pass: &mut before_pass,
            timer,
            started: false,
        };
        self.record(&self.roots, &mut frame);
        if let (Some(buffer), Some(events)) = (event_buffer, outbox) {
            events.end(enc, buffer);
        }
        Ok(())
    }

    fn record(&self, order: &[PassId], frame: &mut Recording<'_, '_>) {
        for id in order.iter().copied() {
            (frame.before_pass)(id, frame.enc);
            match &self.passes[&id] {
                Command::Dispatch(pass, extent) => self.dispatch(frame, pass, extent),
                Command::Ticks(pass, fused) => self.tick(frame, id, pass, fused),
                Command::Draw(render, _) => render.encode(
                    frame.enc,
                    frame.target.expect("validated render attachments"),
                    frame.tables,
                    frame.written,
                ),
                Command::Loop { count, children } => {
                    for _ in 0..*count {
                        self.record(children, frame);
                    }
                }
                Command::Mips(_, chain) => chain
                    .as_ref()
                    .expect("prepared mip chain")
                    .record(frame.enc),
            }
        }
    }

    fn dispatch(&self, frame: &mut Recording<'_, '_>, pass: &ComputePass, extent: &DispatchExtent) {
        let mut dispatch = self.dispatch_pass(frame.enc, pass, None);
        match *extent {
            DispatchExtent::Fixed([x, y, z]) => dispatch.dispatch_workgroups(x, y, z),
            DispatchExtent::FromRowCount { table, workgroup_x } => dispatch.dispatch_workgroups(
                frame.tables[&table].active.div_ceil(workgroup_x),
                1,
                1,
            ),
            DispatchExtent::Indirect { buf, offset_bytes } => dispatch
                .dispatch_workgroups_indirect(
                    frame.tables[&buf]
                        .bytes()
                        .expect("validated indirect buffer"),
                    offset_bytes.into(),
                ),
        }
    }

    fn tick(
        &self,
        frame: &mut Recording<'_, '_>,
        id: PassId,
        pass: &ComputePass,
        fused: &stepping::Fused,
    ) {
        use loft_std::stepping::{Tick, arguments};
        let table = &frame.tables[&fused.rows];
        let live = table.bytes().expect("validated state table");
        let args = frame.tables[&fused.args.resource]
            .bytes()
            .expect("validated advance arguments");
        let keep = if frame.state == Some(fused.rows) && !frame.checkpoints.is_empty() {
            frame.keep
        } else {
            0
        };
        frame.history.advance(
            id,
            table.active,
            frame.to,
            fused.max_ticks,
            keep,
            frame.checkpoints.len(),
            |tick| match tick {
                Tick::Dispatch(rows, ticks, final_dispatch) => {
                    upload(
                        frame.device,
                        frame.enc,
                        &arguments(frame.instance, &rows, ticks),
                        args,
                        0,
                    );
                    // Metal can omit empty marker passes. Timestamp the dispatched work itself.
                    let stamps = frame
                        .timer
                        .filter(|_| !frame.started || final_dispatch)
                        .map(|query_set| wgpu::ComputePassTimestampWrites {
                            query_set,
                            beginning_of_pass_write_index: (!frame.started).then_some(0),
                            end_of_pass_write_index: final_dispatch.then_some(1),
                        });
                    frame.started = true;
                    self.dispatch_pass(frame.enc, pass, stamps)
                        .dispatch_workgroups(
                            (rows.end - rows.start).div_ceil(fused.workgroup),
                            1,
                            1,
                        );
                }
                Tick::Checkpoint(slot) => frame.enc.copy_buffer_to_buffer(
                    live,
                    0,
                    &frame.checkpoints[slot],
                    0,
                    Some(live.size()),
                ),
            },
        );
    }

    fn dispatch_pass<'a>(
        &self,
        enc: &'a mut wgpu::CommandEncoder,
        command: &ComputePass,
        timestamp_writes: Option<wgpu::ComputePassTimestampWrites<'_>>,
    ) -> wgpu::ComputePass<'a> {
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes,
        });
        pass.set_pipeline(&command.pipeline);
        pass.set_bind_group(0, &self.fixed, &[]);
        for group in &command.groups {
            pass.set_bind_group(u32::from(group.index), group.bound(), &[]);
        }
        pass
    }
}

struct Recording<'a, 'target> {
    device: &'a wgpu::Device,
    enc: &'a mut wgpu::CommandEncoder,
    tables: &'a BTreeMap<TableId, Table>,
    target: Option<&'a embed::Target<'target>>,
    written: &'a mut BTreeSet<TableId>,
    history: &'a mut stepping::History,
    checkpoints: &'a [Arc<wgpu::Buffer>],
    state: Option<TableId>,
    instance: InstanceId,
    to: u32,
    keep: u32,
    before_pass: &'a mut dyn FnMut(PassId, &mut wgpu::CommandEncoder),
    timer: Option<&'a wgpu::QuerySet>,
    started: bool,
}

fn validate_dispatch(
    extent: &DispatchExtent,
    tables: &BTreeMap<TableId, Table>,
    limit: u32,
) -> Result<()> {
    let exceeds = match *extent {
        DispatchExtent::Fixed(size) => size.into_iter().any(|n| n > limit),
        DispatchExtent::FromRowCount { table, workgroup_x } => {
            tables[&table].active.div_ceil(workgroup_x) > limit
        }
        DispatchExtent::Indirect { buf, offset_bytes } => {
            let buffer = tables[&buf].bytes()?;
            !buffer.usage().contains(wgpu::BufferUsages::INDIRECT)
                || u64::from(offset_bytes) + 12 > buffer.size()
        }
    };
    if exceeds {
        Err(RunnerError::Capacity)
    } else {
        Ok(())
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
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

/// Stage bytes in the encoder so two recorded frames may share the queue without overwriting each other.
pub(super) fn upload(
    device: &wgpu::Device,
    enc: &mut wgpu::CommandEncoder,
    bytes: &[u8],
    dst: &wgpu::Buffer,
    offset: u64,
) {
    let scratch = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytes,
        usage: wgpu::BufferUsages::COPY_SRC,
    });
    enc.copy_buffer_to_buffer(&scratch, 0, dst, offset, Some(bytes.len() as u64));
}
