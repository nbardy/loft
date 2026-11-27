//! Execution on a caller-owned device.
pub mod backend;
mod encode;
use loft_ir::execution;
mod initialize;
pub mod frame;
pub use loft_wgpu::interop::render_bundle_stale;
mod interop;
pub use loft_wgpu::mips;
use loft_wgpu::allocate;
mod stepping;
pub use loft_wgpu::readback::{read_buffer, read_texture};
use loft_std::{inputs, params};
pub use backend::{ExecutionBackend, WgpuBackend};
pub type Runner = WgpuBackend;

use loft_ir::*;
use loft_wgpu::{interop::Resource, storage};
use std::{collections::BTreeMap, sync::Arc};

pub use loft_std::error::{RunnerError, Result};
#[derive(Debug, PartialEq)]
pub struct FrameIdentity {
    pub instance: InstanceId,
    pub resource_epoch: u64,
    pub time_s: f64,
}

struct Table {
    decl: TableDecl,
    resource: Resource,
    active: u32,
    epoch: u64,
}
struct Instance {
    tables: BTreeMap<TableId, Table>,
    decl: Bundle,
    execution: execution::Execution,
    resource_epoch: u64,
    checkpoints: Vec<Arc<wgpu::Buffer>>,
    history: stepping::History,
    uploads: wgpu::util::StagingBelt,
    outbox: Option<events::Outbox>,
    pick_source: Option<loft_wgpu::pick::Source>,
    compute: encode::ComputeCache,
    initializers: initialize::Initializers,
}
impl std::ops::Deref for Table {
    type Target = Resource;
    fn deref(&self) -> &Resource { &self.resource }
}
impl Instance {
    fn arena(&mut self, k: KindId) -> Result<&mut Table> {
        let id = self.decl.rows.get(&k).ok_or(RunnerError::UnknownResource)?;
        let t = self.tables.get_mut(id).ok_or(RunnerError::UnknownResource)?;
        if !matches!(t.decl.lifetime, TableLifetime::DynamicArena { .. }) {
            return Err(RunnerError::AbiMismatch);
        }
        Ok(t)
    }
}
impl Runner {
    pub fn attach(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>, caps: HostCaps) -> Self {
        Self { device, queue, caps, adapter: None, pipeline_cache: None, pipeline_clock: None, instances:Vec::new() }
    }
    /// Supply the adapter that granted this device and its optional pipeline cache before loading.
    pub fn with_adapter(mut self, adapter: wgpu::Adapter, cache: Option<wgpu::PipelineCache>) -> Self {
        self.adapter = Some(adapter);
        self.pipeline_cache = cache;
        self
    }
    /// Supply a host monotonic clock for per-pipeline creation measurements.
    pub fn with_pipeline_clock(mut self, clock: fn() -> std::time::Duration) -> Self {
        self.pipeline_clock = Some(clock);
        self
    }
    fn instance(&mut self, id: InstanceId) -> Result<&mut Instance> {
        self.instances.get_mut(id.0 as usize).ok_or(RunnerError::UnknownInstance)
    }
    pub fn load_bundle(&mut self, b: &Bundle) -> Result<InstanceId> {
        self.load(b, None)
    }

    fn load(&mut self, b: &Bundle, target: Option<embed::Target<'_>>) -> Result<InstanceId> {
        let id = InstanceId(self.instances.len().try_into().map_err(|_| RunnerError::Capacity)?);
        let mut instance = Instance::new(&self.device, &self.queue, b, self.adapter.as_ref(), self.pipeline_cache.as_ref(), target.as_ref(), self.pipeline_clock)?;
        if let Some(target) = &target { instance.prepare(&self.device, Some(target), 0)?; }
        self.instances.push(instance);
        Ok(id)
    }
    pub fn upsert_rows(&mut self, id: InstanceId, kind: KindId, start: u32, bytes: &[u8]) -> Result<()> {
        let queue = self.queue.clone();
        let t = self.instance(id)?.arena(kind)?;
        let (offset,end) = storage::rows(&t.decl,start,bytes)?;
        if !bytes.is_empty() {
            queue.write_buffer(t.bytes()?,offset,bytes);
            t.active = t.active.max(end);
        }
        Ok(())
    }
    pub fn set_row_count(&mut self, id: InstanceId, kind: KindId, n: u32) -> Result<()> {
        let t = self.instance(id)?.arena(kind)?;
        if n > t.decl.capacity {
            return Err(RunnerError::Capacity);
        }
        t.active = n;
        Ok(())
    }
    pub fn resize_table(&mut self, id: InstanceId, table: TableId, n: u32) -> Result<()> {
        let (device, queue) = (self.device.clone(), self.queue.clone());
        let inst = self.instance(id)?;
        let t = inst.tables.get_mut(&table).ok_or(RunnerError::UnknownResource)?;
        if !matches!(t.decl.lifetime, TableLifetime::DynamicArena { .. }) {
            return Err(RunnerError::AbiMismatch);
        }
        if t.active > n {
            return Err(RunnerError::Capacity);
        }
        let size = storage::bounds(&t.decl,n,&device.limits())?;
        let epoch = interop::next_epoch(inst.resource_epoch)?;
        let mut enc = device.create_command_encoder(&Default::default());
        let replace = |enc: &mut wgpu::CommandEncoder, old: &wgpu::Buffer| {
            let buf = allocate(&device, size, old.usage());
            enc.copy_buffer_to_buffer(old, 0, &buf, 0, Some(old.size().min(buf.size())));
            buf
        };
        t.resource = Resource::Buffer(Some(replace(&mut enc, t.bytes()?)));
        if inst.decl.state.as_ref().is_some_and(|s| s.table == table) {
            for ring in &mut inst.checkpoints {
                *ring = replace(&mut enc, ring)
            }
        }
        queue.submit([enc.finish()]);
        t.decl.capacity = n;
        t.epoch = epoch;
        inst.resource_epoch = epoch;
        Ok(())
    }
    pub fn hot_swap_module(&mut self, id: InstanceId, module: ModuleId, wgsl: &str) -> Result<()> {
        let (device, adapter, cache, clock) = (self.device.clone(), self.adapter.clone(), self.pipeline_cache.clone(), self.pipeline_clock);
        let inst = self.instance(id)?;
        let mut decl = inst.decl.clone();
        let m = decl.modules.iter_mut().find(|m| m.id == module).ok_or(RunnerError::UnknownResource)?;
        m.bytes = wgsl.as_bytes().into();
        m.target = ShaderTarget::Wgsl;
        m.hash = *blake3::hash(wgsl.as_bytes()).as_bytes();
        let mut replacement = encode::ComputeCache::prepare(&device, &decl, &inst.execution, adapter.as_ref(), cache.as_ref(), None, clock)?;
        replacement.params.copy_from(&inst.compute.params);
        inst.decl = decl;
        inst.compute = replacement;
        Ok(())
    }
    pub fn hot_swap_bundle(&mut self, id: InstanceId, b: &Bundle) -> Result<()> {
        let mut next = Instance::new(&self.device, &self.queue, b, self.adapter.as_ref(), self.pipeline_cache.as_ref(), None, self.pipeline_clock)?;
        let old = self.instance(id)?;
        next.resource_epoch = interop::next_epoch(old.resource_epoch)?;
        if let Some((from, state)) = storage::compatible_state(old.decl.state.as_ref(),next.decl.state.as_mut()) {
            let current = &old.tables[&from];
            let dest = next.tables.get_mut(&state.table).ok_or(RunnerError::UnknownResource)?;
            storage::preserve_capacity(&mut dest.decl, &current.decl)?;
            dest.resource = Resource::Buffer(current.buffer().cloned());
            dest.active = current.active;
            next.checkpoints = std::mem::take(&mut old.checkpoints);
            next.history = std::mem::take(&mut old.history);
        }
        for t in next.tables.values_mut() { t.epoch = next.resource_epoch; }
        *old = next;
        Ok(())
    }
    pub fn push_input(&mut self, id: InstanceId, slot: InputId, tick: u64, value: InputValue<'_>) -> Result<()> {
        self.instance(id)?.compute.input.push(slot, tick, value)
    }

    pub fn load_bundle_bytes(&mut self, bytes: &[u8]) -> Result<InstanceId> {
        let bundle = bundle::decode(bytes).map_err(RunnerError::Bundle)?;
        self.load_bundle(&bundle)
    }

    /// Take the oldest completed outbox eligible for the two-frame readback delay.
    pub fn drain_events(&mut self, id: InstanceId) -> Result<(Vec<events::OutboxEvent>, u32)> {
        let _ = self.device.poll(wgpu::PollType::Poll);
        self.instance(id)?
            .outbox
            .as_mut()
            .map_or_else(|| Ok((vec![], 0)), events::Outbox::drain)
    }

    pub fn params(&self, inst: InstanceId) -> &[ParamMeta] {
        self.instances.get(inst.0 as usize).map_or(&[], |i| i.decl.params.as_slice())
    }

    pub fn set_param(&mut self, inst: InstanceId, id: ParamId, values: &[f32]) -> Result<()> {
        let inst = self.instance(inst)?;
        let meta = inst.decl.params.iter().find(|p| p.id == id).ok_or(RunnerError::UnknownResource)?;
        inst.compute.params.set(meta, values)
    }

    pub fn set_pick_source(&mut self, id: InstanceId, visibility: &wgpu::Texture, depth: &wgpu::Texture, inv_vp: [[f32; 4]; 4]) -> Result<()> {
        let d = self.device.clone();
        loft_wgpu::pick::Source::publish(&mut self.instance(id)?.pick_source, &d, visibility, depth, inv_vp)
    }

    pub fn pick_async(&mut self, id: InstanceId, px: [u32; 2], cb: Box<dyn FnOnce(Result<pick::PickResult>) + Send>) -> Result<()> {
        let (d, q) = (self.device.clone(), self.queue.clone());
        self.instance(id)?.pick_source.as_ref().ok_or(RunnerError::UnknownResource)?.pick_async(&d, &q, px, |enc, pipeline, group| {
            command::Pass::Compute(vec![command::Dispatch { pipeline, groups: vec![group], work: command::Work::Direct([1; 3]) }]).record(enc, None);
        }, cb)
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn pick_sync(&mut self, id: InstanceId, px: [u32; 2]) -> Result<pick::PickResult> {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        self.pick_async(
            id,
            px,
            Box::new(move |result| {
                let _ = tx.send(result);
            }),
        )?;
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|_| RunnerError::Unsupported("pick polling failed"))?;
        rx.recv().map_err(|_| RunnerError::Readback)?
    }

    /// Record the bundle into the caller's encoder without submitting it.
    ///
    /// Render passes share one caller color attachment and optional depth attachment.
    /// Use 2D renderable views in their textures' base formats with the sample count
    /// and depth convention supplied at attachment. Shader outputs must use that
    /// depth convention. The caller initializes and, for MSAA, resolves the views.
    pub fn encode_into(&mut self, id: InstanceId, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView,
                       depth: Option<&wgpu::TextureView>, time_s: f64, dt_s: f32) -> Result<FrameIdentity> {
        self.record_frame(id, encoder, target, depth, time_s, dt_s, Default::default())?;
        Ok(FrameIdentity { instance: id, resource_epoch: self.instance(id)?.resource_epoch, time_s })
    }
}
pub use loft_format::bundle;
pub mod embed;
pub use loft_wgpu::outbox as events;
pub mod pick {
    pub use loft_ir::PickResult;
}
pub use loft_format::replay;

pub use loft_wgpu::command;

pub use loft_wgpu::ray_query;
