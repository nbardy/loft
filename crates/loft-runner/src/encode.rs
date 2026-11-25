use super::*;
use command::{Dispatch, Pass, Stamps, Work, compute_pipeline as pipeline};
use loft_std::frame::Templates;
use loft_wgpu::{timed, bindings::{self, slot}, schedule::{self, Order}};
use std::collections::{BTreeSet, HashMap};
use wgpu::util::DeviceExt;
use std::time::Duration;

type Tables = BTreeMap<TableId, Table>;
pub(super) struct Group {
    pub slots: Vec<(wgpu::BindGroupLayoutEntry, TableId)>,
    ranges: BTreeMap<u32, (u64, u64)>,
    layout: wgpu::BindGroupLayout,
    cached: Option<(u64, wgpu::BindGroup)>,
}
impl Group {
    pub fn bound(&self) -> &wgpu::BindGroup { &self.cached.as_ref().expect("prepared group").1 }
    fn resolve(&mut self, d: &wgpu::Device, tables: &Tables, epoch: u64) -> Result<()> {
        if interop::stale(self.cached.as_ref().map(|(e,_)| *e), self.slots.iter().map(|(_,id)| tables[id].epoch)) {
            let limits = d.limits();
            let handles = self.slots.iter().map(|(e,id)| {
                let t = &tables[id];
                let logical = interop::logical_size(&t.decl);
                // A runtime array must see the declared bytes, even when the host allocation has padding.
                t.resource.bind(e, logical, self.ranges.get(&e.binding).copied().unwrap_or((0, logical)), &limits)
            }).collect::<Result<Vec<_>>>()?;
            let entries = self.slots.iter().zip(&handles).map(|((e,_),h)| wgpu::BindGroupEntry { binding:e.binding, resource:h.binding() }).collect::<Vec<_>>();
            self.cached = Some((epoch, bindings::group(d, &self.layout, &entries)));
        }
        Ok(())
    }
}

struct Kernel { pipeline: wgpu::ComputePipeline, layout: wgpu::PipelineLayout, module: ModuleId, entry: String }

enum Operation {
    Dispatch(Kernel, DispatchExtent),
    Ticks(Kernel, stepping::Fused),
    Draw(embed::Render),
    Mips(TableId, Option<(u64, mips::Mips)>),
}
struct Command { groups: Vec<Group>, op: Operation }
impl Command {
    fn dispatch(&self, fixed: &wgpu::BindGroup, k: &Kernel, work: Work) -> Pass {
        let groups = std::iter::once(fixed).chain(self.groups.iter().map(Group::bound)).cloned().collect();
        Pass::Compute(vec![Dispatch { pipeline: k.pipeline.clone(), groups, work }])
    }
}
pub(super) struct ComputeCache {
    fixed: wgpu::BindGroup,
    frame: wgpu::Buffer,
    input_buffer: Arc<wgpu::Buffer>,
    param_buffer: wgpu::Buffer,
    pub params: params::Params,
    pub input: inputs::Inputs,
    pub outbox: Option<TableId>,
    mip_pipelines: mips::Pipelines,
    pub creates: PipelineCreates,
    cache: Option<wgpu::PipelineCache>,
    clock: Option<fn() -> Duration>,
    rasters: Vec<(embed::RasterRecipe, wgpu::RenderPipeline)>,
    templates: Templates,
    order: Vec<Order>,
    passes: BTreeMap<PassId, Command>,
    waves: BTreeMap<PassId, u16>,
    clears: BTreeSet<TableId>,
}

#[derive(Default)]
pub struct PipelineCreates {
    pub compute: Vec<(String, f64)>,
    pub render: Vec<(String, f64)>,
}

#[cfg(not(target_arch = "wasm32"))]
fn create_all<T: Send + 'static, P: Send + 'static>(recipes: Vec<T>, create: impl Fn(T) -> P + Send + Sync + 'static) -> Result<Vec<P>> {
    if recipes.len() <= 1 {
        return Ok(recipes.into_iter().map(create).collect());
    }
    let create = Arc::new(create);
    let workers = recipes.into_iter().map(|recipe| {
        let create = Arc::clone(&create);
        loft_std::worker(move || create(recipe))
    }).collect::<Vec<_>>();
    workers.into_iter().map(|worker| worker.map_err(|e| RunnerError::InvalidShader(e.to_string()))?
        .recv().map_err(|_| RunnerError::Unsupported("pipeline worker stopped"))).collect()
}

#[cfg(target_arch = "wasm32")]
fn create_all<T, P>(recipes: Vec<T>, create: impl Fn(T) -> P) -> Result<Vec<P>> {
    Ok(recipes.into_iter().map(create).collect())
}

impl ComputeCache {
    pub fn prepare(d: &wgpu::Device, b: &Bundle, exec: &execution::Execution,
        adapter: Option<&wgpu::Adapter>, cache: Option<&wgpu::PipelineCache>,
        canvas: Option<(&embed::Target<'_>, &Tables)>, clock: Option<fn() -> Duration>) -> Result<Self> {
        use wgpu::{BufferBindingType::{Storage, Uniform}, BufferUsages as U};
        let schedule = schedule::Schedule::new(b, d.features())?;
        let mut ranges = BTreeMap::new();
        for range in &exec.ranges {
            if ranges.insert((range.pass, range.group, range.slot), (range.offset, range.size)).is_some() {
                return Err(RunnerError::AbiMismatch);
            }
        }
        let limits = d.limits();
        let usages = b.tables.iter().map(|t| (t.id, (interop::logical_size(t), interop::buffer_usage(b, t.id))))
            .collect::<BTreeMap<_,_>>();
        for (&id, pass) in &schedule.passes {
            let slots = match pass {
                schedule::Pass::Kernel { slots, .. } | schedule::Pass::Draw(_, slots) => slots,
                schedule::Pass::Mips(_) => continue,
            };
            for (group, slots) in (1u8..).zip(slots) {
                for (slot, table) in slots {
                    let range = ranges.remove(&(id, group, slot.binding));
                    match slot.ty {
                        wgpu::BindingType::Buffer { .. } => {
                            let (logical, usage) = usages[table];
                            let (offset, size) = range.unwrap_or((0, logical));
                            interop::buffer_range(logical, usage, logical, offset, size, slot, &limits)?;
                        }
                        wgpu::BindingType::StorageTexture { format, access, .. } if range.is_none() =>
                            interop::storage_access(d.features(), adapter, format, access)?,
                        _ if range.is_some() => return Err(RunnerError::AbiMismatch),
                        _ => {}
                    }
                }
            }
        }
        if !ranges.is_empty() { return Err(RunnerError::AbiMismatch); }
        let templates = Templates::new(b, 256)?;
        let sources = b.modules.iter().map(|module| {
            let source = std::str::from_utf8(&module.bytes).map_err(|_| RunnerError::AbiMismatch)?;
            Ok((module.id, source.to_owned()))
        }).collect::<Result<Vec<_>>>()?;
        let device = d.clone();
        let modules = create_all(sources, move |(id, source)| {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None, source: wgpu::ShaderSource::Wgsl(source.into()),
            });
            (id, module)
        })?.into_iter().collect::<BTreeMap<_, _>>();
        let entries = [Uniform, Storage { read_only:true }, Storage { read_only:true }].into_iter().enumerate().map(|(i,ty)| {
            let mut entry = slot(i as u32,ty);
            entry.visibility |= wgpu::ShaderStages::VERTEX_FRAGMENT;
            entry
        }).collect::<Vec<_>>();
        let layout = bindings::layout(d, &entries);
        let frame = d.create_buffer(&wgpu::BufferDescriptor { label: None, size: 256, usage: U::UNIFORM | U::COPY_DST, mapped_at_creation: false });
        let limit = d.limits().max_storage_buffer_binding_size;
        let input = inputs::Inputs::new(&b.resources.inputs, limit)?;
        let input_buffer = allocate(d, input.size(), U::STORAGE | U::COPY_DST);
        let params = params::Params::new(&b.params, limit)?;
        let param_buffer = d.create_buffer_init(&wgpu::util::BufferInitDescriptor { label:None, usage:U::STORAGE | U::COPY_DST, contents:params.bytes() });
        let entries = [&frame, &input_buffer, &param_buffer].into_iter().enumerate()
            .map(|(i,buf)| wgpu::BindGroupEntry { binding: i as u32, resource:buf.as_entire_binding() }).collect::<Vec<_>>();
        let fixed = bindings::group(d, &layout, &entries);
        let mut layouts = HashMap::new();
        let mut pipelines = HashMap::new();
        let mut link = |id, slots: schedule::Slots| {
            let groups: Vec<_> = slots.into_iter().enumerate().map(|(index, slots)| {
                let entries: Vec<_> = slots.iter().map(|(e,_)| *e).collect();
                let layout = layouts.entry(entries.clone()).or_insert_with(|| bindings::layout(d, &entries)).clone();
                let ranges = exec.ranges.iter().filter(|r| r.pass == id && usize::from(r.group) == index + 1)
                    .map(|r| (r.slot, (r.offset, r.size))).collect();
                Group { slots, ranges, layout, cached:None }
            }).collect();
            let bind_group_layouts = std::iter::once(Some(&layout)).chain(groups.iter().map(|g| Some(&g.layout))).collect::<Vec<_>>();
            let key = groups.iter().map(|g| g.layout.clone()).collect::<Vec<_>>();
            let pl = pipelines.entry(key).or_insert_with(|| d.create_pipeline_layout(
                &wgpu::PipelineLayoutDescriptor { label:None, bind_group_layouts:&bind_group_layouts, immediate_size:0 })).clone();
            (groups, pl)
        };
        let mut passes = BTreeMap::new();
        let mut linked = Vec::new();
        let mut unique = HashMap::new();
        let mut recipes = Vec::new();
        let mut renders = Vec::new();
        for (id, pass) in schedule.passes {
            let (groups, layout) = match &pass {
                schedule::Pass::Kernel { slots, .. } | schedule::Pass::Draw(_, slots) => {
                    let (groups, layout) = link(id, slots.clone());
                    (groups, Some(layout))
                }
                schedule::Pass::Mips(_) => (vec![], None),
            };
            match pass {
                schedule::Pass::Kernel { module, entry, work, .. } => {
                    let layout = layout.expect("kernel layout");
                    let index = *unique.entry((module, entry.clone(), layout.clone())).or_insert_with(|| {
                        let index = recipes.len();
                        recipes.push((module, entry.clone(), layout.clone()));
                        index
                    });
                    linked.push((id, groups, module, entry, layout, work, index));
                }
                schedule::Pass::Draw(spec, _) => {
                    let shaders = spec.draws.iter().map(|draw| modules[&draw.decl.module].clone()).collect();
                    let draw = embed::Render { spec, shaders, layout: layout.expect("raster layout"), cached: None,
                        additive: exec.additive.contains(&id), override_pipeline: None };
                    if let Some((target, tables)) = canvas {
                        draw.misses(target, |id| tables.get(&id)?.attachment(), &mut renders)?;
                    }
                    passes.insert(id, Command { groups, op: Operation::Draw(draw) });
                }
                schedule::Pass::Mips(texture) => {
                    passes.insert(id, Command { groups, op: Operation::Mips(texture, None) });
                }
            }
        }
        let mut creates = PipelineCreates::default();
        let mut rasters = Vec::new();
        let mut kernels = Vec::new();
        enum Recipe {
            Kernel(ModuleId, String, wgpu::PipelineLayout, wgpu::ShaderModule),
            Raster(embed::RasterRecipe, wgpu::ShaderModule),
        }
        enum Built {
            Kernel(wgpu::ComputePipeline, (String, f64)),
            Raster(embed::RasterRecipe, wgpu::RenderPipeline, f64),
        }
        let jobs = recipes.into_iter().map(|(m, e, l)| Recipe::Kernel(m, e, l, modules[&m].clone()))
            .chain(renders.into_iter().map(|(recipe, shader)| Recipe::Raster(recipe, shader))).collect();
        let device = d.clone();
        let pipeline_cache = cache.cloned();
        for built in create_all(jobs, move |recipe| match recipe {
            Recipe::Kernel(module, entry, layout, shader) => {
                let (pipe, ms) = timed(clock, || pipeline(&device, &layout, &shader, &entry, pipeline_cache.as_ref()));
                Built::Kernel(pipe, (format!("{} {entry}", module.0), ms))
            }
            Recipe::Raster(recipe, shader) => {
                let (pipe, ms) = timed(clock, || recipe.create(&device, &shader, pipeline_cache.as_ref()));
                Built::Raster(recipe, pipe, ms)
            }
        })? {
            match built {
                Built::Kernel(pipe, timing) => { kernels.push(pipe); creates.compute.push(timing); }
                Built::Raster(recipe, pipe, ms) => {
                    creates.render.push((recipe.name(), ms));
                    rasters.push((recipe, pipe));
                }
            }
        }
        for (id, groups, module, entry, layout, work, index) in linked {
            let k = Kernel { pipeline: kernels[index].clone(), layout, module, entry };
            let op = match work {
                schedule::Work::Dispatch(extent) => Operation::Dispatch(k, extent),
                schedule::Work::Ticks(fused) => Operation::Ticks(k, fused),
            };
            passes.insert(id, Command { groups, op });
        }
        let waves = loft_std::bundle::passes(&b.passes).map(|p| (p.id, p.wave)).collect();
        let clears = b.resources.resets.iter().copied().filter(|id| b.tables.iter().any(|t|
            t.id == *id && t.initial.is_empty())).collect();
        Ok(Self { waves, clears, fixed, frame, input_buffer, param_buffer, params, input, outbox:schedule.outbox, mip_pipelines: Default::default(), creates, cache: cache.cloned(), clock, rasters, templates, order:schedule.order, passes })
    }

    pub fn resource_bindings(&self, id: TableId) -> impl Iterator<Item=&wgpu::BindGroupLayoutEntry> {
        self.passes.values().flat_map(|c| &c.groups).flat_map(|g| &g.slots).filter_map(move |(e,t)| (*t == id).then_some(e))
    }
    pub fn check_buffer(&self, table: TableId, capacity: u64, usage: wgpu::BufferUsages, logical: u64, limits: &wgpu::Limits) -> Result<()> {
        for group in self.passes.values().flat_map(|c| &c.groups) {
            for (slot, _) in group.slots.iter().filter(|(_, id)| *id == table) {
                let (offset, size) = group.ranges.get(&slot.binding).copied().unwrap_or((0, logical));
                interop::buffer_range(capacity, usage, logical, offset, size, slot, limits)?;
            }
        }
        Ok(())
    }
    pub fn fused(&self, id: PassId) -> Option<&stepping::Fused> {
        if let Operation::Ticks(_, f) = &self.passes.get(&id)?.op { Some(f) } else { None }
    }
    #[cfg(test)]
    pub fn pipeline(&self, id: PassId) -> Result<&wgpu::ComputePipeline> {
        match &self.passes.get(&id).ok_or(RunnerError::UnknownResource)?.op {
            Operation::Dispatch(k,_) | Operation::Ticks(k,_) => Ok(&k.pipeline),
            Operation::Draw(_) | Operation::Mips(..) => Err(RunnerError::UnknownResource),
        }
    }
    #[cfg(test)]
    pub fn bound_groups(&self, id: PassId) -> Vec<wgpu::BindGroup> {
        self.passes[&id].groups.iter().map(|g| g.bound().clone()).collect()
    }
}
pub(super) use loft_wgpu::frame::Clock;

#[derive(Default)]
pub struct FrameOptions<'a> {
    pub uniform: Option<&'a dyn Fn(TableId) -> Vec<u8>>,
    pub timestamps: Option<&'a wgpu::QuerySet>,
    pub timestamp_start: u32,
    /// Omit a prepared wave whose output the submitting host has kept valid.
    pub omit_wave: Option<u16>,
    /// Additional waves with retained outputs. Each omitted wave leaves its attachments intact.
    pub omit_waves: &'a [u16],
    pub scissors: Option<&'a dyn Fn(u16, ModuleId) -> Option<command::ScissoredDraw>>,
}

#[derive(Default)]
pub struct FrameRecord {
    pub commands: u32,
    pub stages: Vec<String>,
    /// One entry per encoded timestamp pair; false only for known empty work.
    pub timestamp_required: Vec<bool>,
}

struct Batch<'a> {
    encoder: &'a mut wgpu::CommandEncoder,
    timer: Option<&'a wgpu::QuerySet>,
    timestamp_start: u32,
    pending: Option<(u16, Pass)>,
    report: FrameRecord,
    stamp_error: bool,
}
impl<'a> Batch<'a> {
    fn stamps(&mut self) -> Option<Stamps<'a>> {
        let set = self.timer?;
        let start = (self.report.stages.len() as u32).checked_mul(2)
            .and_then(|index| self.timestamp_start.checked_add(index));
        if let Some(start) = start.filter(|start| start.checked_add(1).is_some_and(|end| end < set.count())) {
            Some(Stamps { set, start: Some(start), end: Some(start + 1) })
        } else {
            self.stamp_error = true;
            None
        }
    }
    fn flush(&mut self) {
        if let Some((wave, pass)) = self.pending.take() {
            let stamps = self.stamps();
            self.report.timestamp_required.push(pass.timestamp_required());
            self.report.commands += pass.record(self.encoder, stamps);
            self.report.stages.push(wave.to_string());
        }
    }
    fn push(&mut self, wave: u16, pass: Pass) {
        let pass = match &mut self.pending {
            Some((w, current)) if *w == wave => match current.append(pass) {
                Ok(()) => return,
                Err(pass) => pass,
            },
            _ => pass,
        };
        self.flush();
        self.pending = Some((wave, pass));
    }
}

impl Instance {
    pub fn prepare(&mut self, d: &wgpu::Device, target: Option<&embed::Target<'_>>, to: u32) -> Result<()> {
        let c = &mut self.compute;
        let tables = &self.tables;
        let mut recipes = Vec::new();
        for cmd in c.passes.values() {
            if let Operation::Draw(draw) = &cmd.op {
                let target = target.ok_or(RunnerError::Unsupported("render attachments are required"))?;
                draw.misses(target, |id| tables.get(&id)?.attachment(), &mut recipes)?;
            }
        }
        recipes.retain(|(recipe, _)| !c.rasters.iter().any(|(r, _)| r == recipe));
        let (device, cache, clock) = (d.clone(), c.cache.clone(), c.clock);
        let created = create_all(recipes, move |(recipe, shader)| {
            let (pipe, ms) = timed(clock, || recipe.create(&device, &shader, cache.as_ref()));
            (recipe, pipe, ms)
        })?;
        for (recipe, pipe, ms) in created {
            c.creates.render.push((recipe.name(), ms));
            c.rasters.push((recipe, pipe));
        }
        let limit = d.limits().max_compute_workgroups_per_dimension;
        for (&id, cmd) in &mut c.passes {
            for group in &mut cmd.groups { group.resolve(d, tables, self.resource_epoch)?; }
            match &mut cmd.op {
                Operation::Dispatch(_, extent) => { resolve(d, tables, extent)?; }
                Operation::Ticks(_, f) => stepping::check_fused(f, id, tables, to, &mut self.history, limit)?,
                Operation::Draw(draw) => draw.prepare(d, target.ok_or(RunnerError::Unsupported("render attachments are required"))?,
                    &c.fixed, &cmd.groups, tables, self.resource_epoch, &c.rasters)?,
                Operation::Mips(id, chain) => {
                    let table = &tables[id];
                    if chain.as_ref().is_none_or(|(epoch,_)| *epoch != table.epoch) {
                        let texture = table.resource.texture().ok_or(RunnerError::UnknownResource)?;
                        let dim = match self.initializers.dimensions.get(id) {
                            Some(&dim) => dim,
                            None => loft_wgpu::mips::portable(&texture)?,
                        };
                        *chain = Some((table.epoch, c.mip_pipelines.prepare(d, &texture, dim, c.cache.as_ref(), c.clock, &mut c.creates.compute)?));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn record(&mut self, d: &wgpu::Device, enc: &mut wgpu::CommandEncoder, clock: Clock<'_>, target: Option<&embed::Target<'_>>) -> Result<()> {
        self.record_frame(d, enc, clock, target, FrameOptions::default()).map(|_| ())
    }

    pub fn record_frame(&mut self, d: &wgpu::Device, enc: &mut wgpu::CommandEncoder, clock: Clock<'_>,
        target: Option<&embed::Target<'_>>, options: FrameOptions<'_>) -> Result<FrameRecord> {
        let result = self.record_uploaded_frame(d, enc, clock, target, options);
        // Encoder-owned copies preserve per-pass data even across deferred submissions.
        self.uploads.finish_and_recall_on_submit(enc);
        result
    }

    fn record_uploaded_frame(&mut self, d: &wgpu::Device, enc: &mut wgpu::CommandEncoder, clock: Clock<'_>,
        target: Option<&embed::Target<'_>>, options: FrameOptions<'_>) -> Result<FrameRecord> {
        if options.timestamps.is_some_and(|set| !matches!(set.ty(), wgpu::QueryType::Timestamp) || options.timestamp_start >= set.count()) {
            return Err(RunnerError::AbiMismatch);
        }
        self.prepare(d, target, clock.to)?;
        let c = &mut self.compute;
        let tables = &self.tables;
        clock.validate(&c.order, |id| c.fused(id).is_some_and(|f| tables[&f.rows].active > 0))?;
        let inputs = c.input.advance((clock.time / f64::from(clock.dt)).round() as u64)?;
        let event_buffer = c.outbox.map(|id| tables[&id].bytes().cloned()).transpose()?;
        if let Some(buffer) = &event_buffer {
            let outbox = match &mut self.outbox {
                Some(old) => old,
                slot @ None => slot.insert(events::Outbox::new(d, buffer.size())?),
            };
            outbox.prepare()?;
            outbox.begin(&mut self.uploads, enc, buffer)?;
        }
        if let Some(data) = c.params.take_upload() { upload(&mut self.uploads,enc,data,&c.param_buffer,0); }
        if let Some(data) = inputs { upload(&mut self.uploads,enc,data,&c.input_buffer,0); }
        for id in &self.decl.resources.resets {
            let t = tables.get(id).ok_or(RunnerError::UnknownResource)?;
            if c.clears.contains(id) { enc.clear_buffer(t.bytes()?, 0, None); }
            else { upload(&mut self.uploads, enc, &t.decl.initial, t.bytes()?, 0); }
        }
        if matches!(c.templates, Templates::Clock) {
            upload(&mut self.uploads,enc,[(clock.time as f32).to_le_bytes(),clock.dt.to_le_bytes()].as_flattened(),&c.frame,0);
        }
        let load = self.record_initializers(d, enc, None, true)?;
        let dirty = self.record_initializers(d, enc, None, false)?;
        let c = &mut self.compute;
        let tables = &self.tables;
        let mut batch = Batch { encoder: enc, timer: options.timestamps, timestamp_start: options.timestamp_start, pending: None, stamp_error: false,
            report: FrameRecord { commands: load.commands + dirty.commands, ..Default::default() } };
        let mut written = BTreeSet::new();
        let initializers: BTreeSet<_> = self.initializers.clocks.iter().flat_map(|i| &i.passes).copied().collect();
        let mut template = None;
        // The first due tick of each lockstep group: every pass in it advances to each tick in turn.
        let froms: Vec<u32> = c.order.iter().map(|item| match item {
            Order::Lockstep(ids) => ids.iter().filter_map(|&id| c.fused(id).map(|f|
                self.history.seek(id, tables[&f.rows].active).iter().copied().min().unwrap_or(clock.to))).min().unwrap_or(clock.to),
            Order::Run(_) | Order::Repeat(..) => clock.to,
        }).collect();
        // `lockstep`: the tick this call advances to, and whether it is the group's first and last dispatch.
        let mut record = |id, lockstep: Option<(u32, bool, bool)>| {
            let wave = c.waves[&id];
            if initializers.contains(&id) || options.omit_wave == Some(wave) || options.omit_waves.contains(&wave) { return; }
            if let Some(bytes) = c.templates.change(id, &mut template, clock.time as f32) {
                batch.flush();
                let bytes = options.uniform.map_or(bytes, |f| f(template.expect("selected frame table")));
                upload(&mut self.uploads,batch.encoder,&bytes,&c.frame,0);
            }
            let cmd = &c.passes[&id];
            match &cmd.op {
                Operation::Dispatch(k, extent) => {
                    let work = resolve(d, tables, extent).expect("validated extent");
                    batch.push(c.waves[&id], cmd.dispatch(&c.fixed,k,work));
                }
                Operation::Draw(draw) => {
                    let pass = draw.pass(target.expect("validated targets"), tables, &mut written,
                        &c.fixed, &cmd.groups, |module| options.scissors.and_then(|f| f(wave, module)));
                    batch.push(c.waves[&id], pass);
                }
                Operation::Mips(_, chain) => {
                    batch.flush();
                    let stamps = batch.stamps().map(|s| (s.set, s.start.expect("first mip stamp")));
                    batch.report.commands += chain.as_ref().expect("prepared mip chain").1.encode(batch.encoder, stamps);
                    batch.report.timestamp_required.push(true);
                    batch.report.stages.push(c.waves[&id].to_string());
                }
                Operation::Ticks(k, f) => {
                    batch.flush();
                    let table = &tables[&f.rows];
                    let live = table.bytes().expect("validated state table");
                    let args = tables[&f.args.resource].bytes().expect("validated arguments");
                    let step_clock = match lockstep { Some((tick, ..)) => Clock { to: tick, ..clock }, None => clock };
                    let mut started = false;
                    step_clock.advance(&mut self.history, id, f, table.active, self.decl.state.as_ref(), self.checkpoints.len(), |step| match step {
                        loft_wgpu::frame::Step::Save(i) => batch.encoder.copy_buffer_to_buffer(live,0,&self.checkpoints[i],0,Some(live.size())),
                        loft_wgpu::frame::Step::Run { args: data, work, stamps } => {
                            // A timed lockstep advance stamps the group's first dispatch start and last dispatch end.
                            let stamps = match lockstep {
                                Some((_, first, last)) => clock.timer.filter(|_| (first && !started) || last).map(|set|
                                    Stamps { set, start: (first && !started).then_some(0), end: last.then_some(1) }),
                                None => stamps,
                            };
                            started = true;
                            upload(&mut self.uploads, batch.encoder, &data, args, 0);
                            batch.report.commands += cmd.dispatch(&c.fixed, k, Work::Direct(work)).record(batch.encoder,stamps);
                        }
                    });
                }
            }
        };
        for (item, &from) in c.order.iter().zip(&froms) {
            match item {
                Order::Lockstep(ids) => {
                    // Nothing due still records one zero-tick dispatch per pass, as a single pass does: it writes
                    // the state and evaluates the checks at the current tick.
                    let ticks = if from < clock.to { from + 1..=clock.to } else { clock.to..=clock.to };
                    let (first, last) = (*ticks.start(), *ticks.end());
                    for tick in ticks {
                        for (n, &id) in ids.iter().enumerate() {
                            record(id, Some((tick, tick == first && n == 0, tick == last && n + 1 == ids.len())));
                        }
                    }
                }
                Order::Run(_) | Order::Repeat(..) => item.visit(&mut |id| record(id, None)),
            }
        }
        batch.flush();
        if batch.stamp_error { return Err(RunnerError::Capacity); }
        if let (Some(buf), Some(events)) = (event_buffer.as_ref(), &mut self.outbox) { events.end(batch.encoder,buf); }
        Ok(batch.report)
    }

    pub fn record_initializers(&mut self, d: &wgpu::Device, enc: &mut wgpu::CommandEncoder,
        timer: Option<&wgpu::QuerySet>, loading: bool) -> Result<FrameRecord> {
        let c = &self.compute;
        let params = c.params.bytes().as_chunks::<4>().0.iter().copied().map(f32::from_le_bytes).collect::<Vec<_>>();
        let mut report = FrameRecord::default();
        for init in self.initializers.clocks.iter_mut().filter(|i| i.clock.due(loading, &params)) {
            for (i, id) in init.passes.iter().enumerate() {
                let cmd = &c.passes[id];
                let start = report.stages.len() as u32 * 2;
                let stamp = timer.map(|set| Stamps { set, start: Some(start), end: Some(start + 1) });
                report.commands += match &cmd.op {
                    Operation::Dispatch(k, extent) => {
                        let pass = cmd.dispatch(&c.fixed, k, resolve(d, &self.tables, extent)?);
                        match self.initializers.packed.get(id) {
                            Some(output) => output.record(&pass, enc, stamp),
                            None => pass.record(enc, stamp),
                        }
                    },
                    Operation::Mips(_, chain) => chain.as_ref().expect("prepared mip chain").1.encode(enc, timer.map(|set| (set, start))),
                    Operation::Draw(_) | Operation::Ticks(..) => return Err(RunnerError::AbiMismatch),
                };
                report.timestamp_required.push(true);
                report.stages.push(format!("{} {}", init.name, if i == 0 { "base" } else { "mips" }));
            }
            init.clock.complete(&params);
        }
        Ok(report)
    }
}

fn resolve(d: &wgpu::Device, tables: &Tables, extent: &DispatchExtent) -> Result<Work> {
    Work::resolve(extent, d.limits().max_compute_workgroups_per_dimension, |id| tables[&id].active, |id| tables[&id].bytes())
}

/// Stage bytes in the encoder so two recorded frames may share the queue without overwriting each other.
fn upload(belt: &mut wgpu::util::StagingBelt, enc: &mut wgpu::CommandEncoder, bytes: &[u8], dst: &wgpu::Buffer, offset: u64) {
    if let Some(size) = wgpu::BufferSize::new(bytes.len() as u64) {
        belt.write_buffer(enc, dst, offset, size).copy_from_slice(bytes);
    }
}

#[derive(Clone)]
pub struct ModulePipelines {
    pub render: wgpu::RenderPipeline,
    pub shade: wgpu::ComputePipeline,
}

#[derive(Clone)]
pub struct ModuleRecipe {
    raster: embed::RasterRecipe,
    layout: wgpu::PipelineLayout,
    entry: String,
}

impl ModuleRecipe {
    pub fn compile(&self, d: &wgpu::Device, source: &str) -> ModulePipelines {
        let module = d.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None, source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        ModulePipelines { render: self.raster.create(d, &module, None), shade: pipeline(d, &self.layout, &module, &self.entry, None) }
    }
}

impl ComputeCache {
    pub(super) fn recipe(&self, id: ModuleId) -> Result<(ModuleRecipe, ModulePipelines)> {
        let (raster, render) = self.passes.values().find_map(|cmd| match &cmd.op {
            Operation::Draw(draw) if draw.spec.draws.iter().any(|d| d.decl.module == id) => draw.recipe(),
            _ => None,
        }).ok_or(RunnerError::UnknownResource)?;
        let k = self.passes.values().find_map(|cmd| match &cmd.op {
            Operation::Dispatch(k, DispatchExtent::Indirect { .. }) if k.module == id => Some(k),
            _ => None,
        }).ok_or(RunnerError::UnknownResource)?;
        let shade = k.pipeline.clone();
        Ok((ModuleRecipe { raster, layout: k.layout.clone(), entry: k.entry.clone() }, ModulePipelines { render, shade }))
    }

    pub(super) fn install(&mut self, id: ModuleId, pipes: &ModulePipelines) {
        for cmd in self.passes.values_mut() {
            match &mut cmd.op {
                Operation::Draw(draw) if draw.spec.draws.iter().any(|d| d.decl.module == id) => {
                    draw.override_pipeline = Some(pipes.render.clone());
                    draw.cached = None;
                }
                Operation::Dispatch(k, DispatchExtent::Indirect { .. }) if k.module == id => k.pipeline = pipes.shade.clone(),
                _ => (),
            }
        }
    }
}
