use super::*;
use std::collections::{BTreeSet, HashMap};
use wgpu::util::DeviceExt;

struct Group {
    layout: wgpu::BindGroupLayout,
    slots: Vec<(wgpu::BindGroupLayoutEntry, TableId)>,
    cached: Option<(u64, wgpu::BindGroup)>,
}
impl Group {
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
            let views = self
                .slots
                .iter()
                .map(|(slot, id)| match slot.ty {
                    wgpu::BindingType::Buffer { .. } => Ok(None),
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
        let inputs = allocate(d, 16);
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
            if modules.insert(m.id, (module, ir, info)).is_some() {
                return Err(RunnerError::AbiMismatch);
            }
        }
        let mut layouts = HashMap::new();
        let mut passes = BTreeMap::new();
        let mut ids = BTreeSet::new();
        let mut fused = BTreeMap::new();
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
            let (module, entry) = match &p.run {
                PassRun::Compute { module, entry, .. }
                | PassRun::ComputeFusedTicks { module, entry, .. } => (module, entry),
                _ => continue,
            };
            let (shader, ir, info) = modules.get(module).ok_or(RunnerError::UnknownResource)?;
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
            let mut groups = Vec::new();
            for group in 1..=p.bindings.iter().map(|x| x.group).max().unwrap_or(0) {
                let mut bindings: Vec<_> = p.bindings.iter().filter(|x| x.group == group).collect();
                bindings.sort_by_key(|x| x.slot);
                let entries = bindings
                    .iter()
                    .map(|x| match reflected.get(&(u32::from(x.group), x.slot)) {
                        Some(e) => Ok(*e),
                        None if x.group <= 2 => Ok(slot(
                            x.slot,
                            Storage {
                                read_only: x.access == BindAccess::ReadStorage,
                            },
                        )),
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
    ) -> Result<()> {
        // Validate and resolve the entire sequence before touching the caller's encoder.
        for p in passes {
            let step = self
                .passes
                .get_mut(&p.id)
                .ok_or(RunnerError::Unsupported("pass execution"))?;
            for g in &mut step.groups {
                g.resolve(d, tables, epoch)?;
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
        let data = [(time as f32).to_le_bytes(), dt.to_le_bytes()];
        let upload = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: data.as_flattened(),
            usage: wgpu::BufferUsages::COPY_SRC,
        });
        enc.copy_buffer_to_buffer(&upload, 0, &self.frame, 0, Some(8));
        for p in passes {
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
