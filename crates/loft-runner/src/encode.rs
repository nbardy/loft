use super::*;
use command::{Dispatch, Pass, Stamps, Work};
use loft_std::{bindings::slot, edits::Pipelines, frame::Templates, schedule::{self, Order}};
use std::collections::{BTreeSet, HashMap};
use wgpu::util::DeviceExt;

type Tables = BTreeMap<TableId, Table>;
pub(super) struct Group {
    pub slots: Vec<(wgpu::BindGroupLayoutEntry, TableId)>,
    layout: wgpu::BindGroupLayout,
    cached: Option<(u64, wgpu::BindGroup)>,
}
impl Group {
    pub fn bound(&self) -> &wgpu::BindGroup { &self.cached.as_ref().expect("prepared group").1 }
    fn resolve(&mut self, d: &wgpu::Device, tables: &Tables, epoch: u64) -> Result<()> {
        if interop::stale(self.cached.as_ref().map(|(e,_)| *e), self.slots.iter().map(|(_,id)| tables[id].epoch)) {
            let handles = self.slots.iter().map(|(e,id)| tables[id].resource.bind(e)).collect::<Result<Vec<_>>>()?;
            let entries = self.slots.iter().zip(&handles).map(|((e,_),h)| wgpu::BindGroupEntry { binding:e.binding, resource:h.binding() }).collect::<Vec<_>>();
            self.cached = Some((epoch, d.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &self.layout, entries: &entries })));
        }
        Ok(())
    }
}

struct Kernel { pipeline: wgpu::ComputePipeline, layout: wgpu::PipelineLayout, module: ModuleId, entry: String }

fn pipeline(d: &wgpu::Device, layout: &wgpu::PipelineLayout, module: &wgpu::ShaderModule, entry: &str) -> wgpu::ComputePipeline {
    d.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None, layout: Some(layout), module, entry_point: Some(entry), compilation_options: Default::default(), cache: None,
    })
}
enum Operation {
    Dispatch(Kernel, DispatchExtent),
    Ticks(Kernel, stepping::Fused),
    Draw(embed::Render),
    Mips(TableId, Option<(u64, mips::Mips)>),
}
struct Command { groups: Vec<Group>, op: Operation }
impl Command {
    fn kernel(&mut self) -> Result<&mut Kernel> {
        match &mut self.op {
            Operation::Dispatch(k, _) | Operation::Ticks(k, _) => Ok(k),
            Operation::Draw(_) | Operation::Mips(..) => Err(RunnerError::UnknownResource),
        }
    }
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
    templates: Templates,
    order: Vec<Order>,
    passes: BTreeMap<PassId, Command>,
    waves: BTreeMap<PassId, u16>,
    clears: BTreeSet<TableId>,
}

impl ComputeCache {
    pub fn new(d: &wgpu::Device, b: &Bundle) -> Result<Self> {
        Self::prepare(d, b, &[])
    }

    pub fn prepare(d: &wgpu::Device, b: &Bundle, additive: &[PassId]) -> Result<Self> {
        use wgpu::{BufferBindingType::{Storage, Uniform}, BufferUsages as U};
        let schedule = schedule::Schedule::new(b, d.features())?;
        let templates = Templates::new(b, 256)?;
        let mut modules = BTreeMap::new();
        for m in &b.modules {
            let src = std::str::from_utf8(&m.bytes).map_err(|_| RunnerError::AbiMismatch)?;
            modules.insert(m.id, d.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(src.into()) }));
        }
        let entries = [Uniform, Storage { read_only:true }, Storage { read_only:true }].into_iter().enumerate().map(|(i,ty)| {
            let mut entry = slot(i as u32,ty);
            entry.visibility |= wgpu::ShaderStages::VERTEX_FRAGMENT;
            entry
        }).collect::<Vec<_>>();
        let layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &entries });
        let frame = d.create_buffer(&wgpu::BufferDescriptor { label: None, size: 256, usage: U::UNIFORM | U::COPY_DST, mapped_at_creation: false });
        let limit = d.limits().max_storage_buffer_binding_size;
        let input = inputs::Inputs::new(&b.resources.inputs, limit)?;
        let input_buffer = allocate(d, input.size());
        let params = params::Params::new(&b.params, limit)?;
        let param_buffer = d.create_buffer_init(&wgpu::util::BufferInitDescriptor { label:None, usage:U::STORAGE | U::COPY_DST, contents:params.bytes() });
        let entries = [&frame, &input_buffer, &param_buffer].into_iter().enumerate()
            .map(|(i,buf)| wgpu::BindGroupEntry { binding: i as u32, resource:buf.as_entire_binding() }).collect::<Vec<_>>();
        let fixed = d.create_bind_group(&wgpu::BindGroupDescriptor { label:None, layout:&layout, entries:&entries });
        let mut layouts = HashMap::new();
        let mut pipelines = HashMap::new();
        let mut link = |slots: schedule::Slots| {
            let groups: Vec<_> = slots.into_iter().map(|slots| {
                let entries: Vec<_> = slots.iter().map(|(e,_)| *e).collect();
                let layout = layouts.entry(entries.clone()).or_insert_with(|| d.create_bind_group_layout(
                    &wgpu::BindGroupLayoutDescriptor { label: None, entries:&entries })).clone();
                Group { slots, layout, cached:None }
            }).collect();
            let bind_group_layouts = std::iter::once(Some(&layout)).chain(groups.iter().map(|g| Some(&g.layout))).collect::<Vec<_>>();
            let key = groups.iter().map(|g| g.layout.clone()).collect::<Vec<_>>();
            let pl = pipelines.entry(key).or_insert_with(|| d.create_pipeline_layout(
                &wgpu::PipelineLayoutDescriptor { label:None, bind_group_layouts:&bind_group_layouts, immediate_size:0 })).clone();
            (groups, pl)
        };
        let mut passes = BTreeMap::new();
        for (id, pass) in schedule.passes {
            let (groups, op) = match pass {
                schedule::Pass::Kernel { module, entry, slots, work } => {
                    let (groups, layout) = link(slots);
                    let pipeline = pipeline(d, &layout, &modules[&module], &entry);
                    let k = Kernel { pipeline, layout, module, entry };
                    (groups, match work { schedule::Work::Dispatch(e) => Operation::Dispatch(k,e), schedule::Work::Ticks(f) => Operation::Ticks(k,f) })
                }
                schedule::Pass::Draw(spec, slots) => {
                    let (groups, layout) = link(slots);
                    let shaders = spec.draws.iter().map(|d| modules[&d.decl.module].clone()).collect();
                    (groups, Operation::Draw(embed::Render { spec, shaders, layout, cached:None, additive: additive.contains(&id), override_pipeline: None }))
                }
                schedule::Pass::Mips(id) => (vec![], Operation::Mips(id,None)),
            };
            passes.insert(id, Command { groups, op });
        }
        let waves = loft_std::bundle::passes(&b.passes).map(|p| (p.id, p.wave)).collect();
        let clears = b.resources.resets.iter().copied().filter(|id| b.tables.iter().any(|t|
            t.id == *id && t.initial.is_empty())).collect();
        Ok(Self { waves, clears, fixed, frame, input_buffer, param_buffer, params, input, outbox:schedule.outbox, mip_pipelines: Default::default(), templates, order:schedule.order, passes })
    }

    pub fn resource_bindings(&self, id: TableId) -> impl Iterator<Item=&wgpu::BindGroupLayoutEntry> {
        self.passes.values().flat_map(|c| &c.groups).flat_map(|g| &g.slots).filter_map(move |(e,t)| (*t == id).then_some(e))
    }
    pub fn fused(&self, id: PassId) -> Option<&stepping::Fused> {
        if let Operation::Ticks(_, f) = &self.passes.get(&id)?.op { Some(f) } else { None }
    }
    fn kernel(&self, id: PassId) -> Result<&Kernel> {
        match &self.passes.get(&id).ok_or(RunnerError::UnknownResource)?.op {
            Operation::Dispatch(k,_) | Operation::Ticks(k,_) => Ok(k),
            Operation::Draw(_) | Operation::Mips(..) => Err(RunnerError::UnknownResource),
        }
    }
    #[cfg(test)]
    pub fn bound_groups(&self, id: PassId) -> Vec<wgpu::BindGroup> {
        self.passes[&id].groups.iter().map(|g| g.bound().clone()).collect()
    }
}
impl Pipelines for ComputeCache {
    fn pipeline(&self, id: PassId) -> Result<&wgpu::ComputePipeline> { Ok(&self.kernel(id)?.pipeline) }
    #[cfg(not(target_arch = "wasm32"))]
    fn pipeline_layout(&self, id: PassId) -> Result<&wgpu::PipelineLayout> { Ok(&self.kernel(id)?.layout) }
    fn replace_pipeline(&mut self, id: PassId, p: wgpu::ComputePipeline) -> Result<()> {
        self.passes.get_mut(&id).ok_or(RunnerError::UnknownResource)?.kernel()?.pipeline = p;
        Ok(())
    }
}
pub(super) use loft_std::frame::Clock;

#[derive(Default)]
pub struct FrameOptions<'a> {
    pub uniform: Option<&'a dyn Fn(TableId) -> Vec<u8>>,
    pub timestamps: Option<&'a wgpu::QuerySet>,
}

#[derive(Default)]
pub struct FrameRecord {
    pub commands: u32,
    pub stages: Vec<String>,
}

struct Batch<'a> {
    encoder: &'a mut wgpu::CommandEncoder,
    timer: Option<&'a wgpu::QuerySet>,
    pending: Option<(u16, Pass)>,
    report: FrameRecord,
}
impl Batch<'_> {
    fn flush(&mut self) {
        if let Some((wave, pass)) = self.pending.take() {
            let start = self.report.stages.len() as u32 * 2;
            let stamps = self.timer.map(|set| Stamps { set, start: Some(start), end: Some(start + 1) });
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
        let mut raster_pipelines = Vec::new();
        let limit = d.limits().max_compute_workgroups_per_dimension;
        for (&id, cmd) in &mut c.passes {
            for group in &mut cmd.groups { group.resolve(d, tables, self.resource_epoch)?; }
            match &mut cmd.op {
                Operation::Dispatch(_, extent) => { resolve(d, tables, extent)?; }
                Operation::Ticks(_, f) => stepping::check_fused(f, id, tables, to, &mut self.history, limit)?,
                Operation::Draw(draw) => draw.prepare(d, target.ok_or(RunnerError::Unsupported("render attachments are required"))?,
                    &c.fixed, &cmd.groups, tables, self.resource_epoch, &mut raster_pipelines)?,
                Operation::Mips(id, chain) => {
                    let table = &tables[id];
                    if chain.as_ref().is_none_or(|(epoch,_)| *epoch != table.epoch) {
                        let texture = table.resource.texture().ok_or(RunnerError::UnknownResource)?;
                        let dim = match self.initializers.dimensions.get(id) {
                            Some(&dim) => dim,
                            None => loft_std::mips::portable(&texture)?,
                        };
                        *chain = Some((table.epoch, c.mip_pipelines.prepare(d, &texture, dim).map_err(RunnerError::InvalidShader)?));
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
        self.edits.update(d, &mut self.compute)?;
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
            outbox.begin(d, enc, buffer)?;
        }
        if let Some(data) = c.params.take_upload() { upload(d,enc,data,&c.param_buffer,0); }
        if let Some(data) = inputs { upload(d,enc,data,&c.input_buffer,0); }
        for id in &self.decl.resources.resets {
            let t = tables.get(id).ok_or(RunnerError::UnknownResource)?;
            if c.clears.contains(id) { enc.clear_buffer(t.bytes()?, 0, None); }
            else { upload(d, enc, &t.decl.initial, t.bytes()?, 0); }
        }
        if matches!(c.templates, Templates::Clock) {
            upload(d,enc,[(clock.time as f32).to_le_bytes(),clock.dt.to_le_bytes()].as_flattened(),&c.frame,0);
        }
        let load = self.record_initializers(d, enc, None, true)?;
        let dirty = self.record_initializers(d, enc, None, false)?;
        let c = &mut self.compute;
        let tables = &self.tables;
        let mut batch = Batch { encoder: enc, timer: options.timestamps, pending: None,
            report: FrameRecord { commands: load.commands + dirty.commands, ..Default::default() } };
        let mut written = BTreeSet::new();
        let initializers: BTreeSet<_> = self.initializers.clocks.iter().flat_map(|i| &i.passes).copied().collect();
        let mut template = None;
        let mut record = |id| {
            if initializers.contains(&id) { return; }
            if let Some(bytes) = c.templates.change(id, &mut template, clock.time as f32) {
                batch.flush();
                let bytes = options.uniform.map_or(bytes, |f| f(template.expect("selected frame table")));
                upload(d,batch.encoder,&bytes,&c.frame,0);
            }
            let cmd = &c.passes[&id];
            match &cmd.op {
                Operation::Dispatch(k, extent) => {
                    let work = resolve(d, tables, extent).expect("validated extent");
                    batch.push(c.waves[&id], cmd.dispatch(&c.fixed,k,work));
                }
                Operation::Draw(draw) => {
                    let pass = draw.pass(target.expect("validated targets"),tables,&mut written);
                    batch.push(c.waves[&id], pass);
                }
                Operation::Mips(_, chain) => {
                    batch.flush();
                    let start = batch.report.stages.len() as u32 * 2;
                    batch.report.commands += chain.as_ref().expect("prepared mip chain").1.encode(batch.encoder, batch.timer.map(|set| (set, start)));
                    batch.report.stages.push(c.waves[&id].to_string());
                }
                Operation::Ticks(k, f) => {
                    batch.flush();
                    let table = &tables[&f.rows];
                    let live = table.bytes().expect("validated state table");
                    let args = tables[&f.args.resource].bytes().expect("validated arguments");
                    clock.advance(&mut self.history, id, f, table.active, self.decl.state.as_ref(), self.checkpoints.len(), |step| match step {
                        loft_std::frame::Step::Save(i) => batch.encoder.copy_buffer_to_buffer(live,0,&self.checkpoints[i],0,Some(live.size())),
                        loft_std::frame::Step::Run { args: data, work, stamps } => {
                            upload(d, batch.encoder, &data, args, 0);
                            batch.report.commands += cmd.dispatch(&c.fixed, k, Work::Direct(work)).record(batch.encoder,stamps);
                        }
                    });
                }
            }
        };
        for item in &c.order { item.visit(&mut record); }
        batch.flush();
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
                    Operation::Dispatch(k, extent) => cmd.dispatch(&c.fixed, k, resolve(d, &self.tables, extent)?).record(enc, stamp),
                    Operation::Mips(_, chain) => chain.as_ref().expect("prepared mip chain").1.encode(enc, timer.map(|set| (set, start))),
                    Operation::Draw(_) | Operation::Ticks(..) => return Err(RunnerError::AbiMismatch),
                };
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

#[cfg(all(test, not(target_arch = "wasm32")))]
mod edit_gate;
/// Stage bytes in the encoder so two recorded frames may share the queue without overwriting each other.
pub(super) fn upload(device: &wgpu::Device, enc: &mut wgpu::CommandEncoder, bytes: &[u8], dst: &wgpu::Buffer, offset: u64) {
    let scratch = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytes,
        usage: wgpu::BufferUsages::COPY_SRC,
    });
    enc.copy_buffer_to_buffer(&scratch, 0, dst, offset, Some(bytes.len() as u64));
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
        ModulePipelines { render: self.raster.create(d, &module), shade: pipeline(d, &self.layout, &module, &self.entry) }
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
