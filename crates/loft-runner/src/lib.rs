//! Execution on a caller-owned device.
pub mod backend;
mod encode;
#[path = "../../loft-passes/src/execution.rs"]
mod execution;
mod initialize;
pub mod frame;
pub use loft_std::interop::render_bundle_stale;
mod interop;
pub mod mips;
mod stepping;
mod resources;
use loft_std::inputs;
pub use backend::{ExecutionBackend, WgpuBackend};
pub type Runner = WgpuBackend;

use loft_ir::*;
use loft_std::{interop::Resource, storage};
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
    resource_epoch: u64,
    checkpoints: Vec<Arc<wgpu::Buffer>>,
    history: stepping::History,
    outbox: Option<events::Outbox>,
    pick_source: Option<pick::Source>,
    compute: encode::ComputeCache,
    initializers: initialize::Initializers,
    edits: tier0::Edits,
}
fn allocate(d: &wgpu::Device, size: u64) -> Arc<wgpu::Buffer> {
    use wgpu::BufferUsages as U;
    Arc::new(d.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: size.max(4),
        usage: U::STORAGE | U::COPY_DST | U::COPY_SRC | U::INDIRECT | U::VERTEX | U::INDEX,
        mapped_at_creation: false,
    }))
}
impl std::ops::Deref for Table {
    type Target = Resource;
    fn deref(&self) -> &Resource { &self.resource }
}
impl Instance {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, decl: &Bundle) -> Result<Self> {
        let mut decl = decl.clone();
        let descriptor = decl.tables.iter().position(|t| t.id == execution::DESCRIPTOR);
        let exec = descriptor.map(|i| execution::Execution::read(&decl.tables.remove(i))
            .ok_or(RunnerError::AbiMismatch)).transpose()?.unwrap_or_default();
        if decl.tables.iter().any(|t| t.id == execution::DESCRIPTOR) { return Err(RunnerError::AbiMismatch); }
        let mut tables = storage::allocations(&decl, &device.limits())?.into_iter().map(|(t,active,alloc)| {
            let resource = resources::create(device,queue,t,alloc)?;
            Ok((t.id, Table { decl:t.clone(), resource, active, epoch:0 }))
        }).collect::<Result<BTreeMap<_,_>>>()?;
        let checkpoints = if let Some(s) = &decl.state {
            let table = tables.get(&s.table).ok_or(RunnerError::UnknownResource)?;
            let size = table.bytes()?.size();
            (0..s.checkpoint_slots).map(|_| allocate(device,size)).collect()
        } else {
            Vec::new()
        };
        let compute = encode::ComputeCache::prepare(device, &decl, &exec.additive)?;
        let initializers = initialize::Initializers::prepare(exec, &decl, &mut tables, &compute, device)?;
        Ok(Self { tables, decl, compute, initializers, checkpoints, resource_epoch:0,
            edits:Default::default(), history:Default::default(), outbox:None, pick_source:None })
    }
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
        Self { device, queue, caps, instances:Vec::new() }
    }
    fn instance(&mut self, id: InstanceId) -> Result<&mut Instance> {
        self.instances.get_mut(id.0 as usize).ok_or(RunnerError::UnknownInstance)
    }
    pub fn load_bundle(&mut self, b: &Bundle) -> Result<InstanceId> {
        let id = InstanceId(self.instances.len().try_into().map_err(|_| RunnerError::Capacity)?);
        self.instances.push(Instance::new(&self.device,&self.queue,b)?);
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
            let buf = allocate(&device, size);
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
        let device = self.device.clone();
        let inst = self.instance(id)?;
        if inst.edits.request(module, wgsl)? {
            return Ok(());
        }
        let mut decl = inst.decl.clone();
        let m = decl.modules.iter_mut().find(|m| m.id == module).ok_or(RunnerError::UnknownResource)?;
        m.bytes = wgsl.as_bytes().into();
        m.target = ShaderTarget::Wgsl;
        m.hash = *blake3::hash(wgsl.as_bytes()).as_bytes();
        let mut replacement = encode::ComputeCache::new(&device, &decl)?;
        replacement.params.copy_from(&inst.compute.params);
        inst.decl = decl;
        inst.compute = replacement;
        inst.edits = tier0::Edits::default();
        Ok(())
    }
    pub fn hot_swap_bundle(&mut self, id: InstanceId, b: &Bundle) -> Result<()> {
        let mut next = Instance::new(&self.device, &self.queue, b)?;
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

    /// Record the bundle into the caller's encoder without submitting it.
    ///
    /// Render passes share one caller color attachment and optional depth attachment.
    /// Use 2D renderable views in their textures' base formats with the sample count
    /// and depth convention supplied at attachment. Shader outputs must use that
    /// depth convention. The caller initializes and, for MSAA, resolves the views.
    pub fn encode_into(&mut self, id: InstanceId, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView,
                       depth: Option<&wgpu::TextureView>, time_s: f64, dt_s: f32) -> Result<FrameIdentity> {
        let d = self.device.clone();
        let target = embed::Target { color: target, depth, caps: self.caps };
        let inst = self.instance(id)?;
        inst.record(&d, encoder, encode::Clock { id, time: time_s, dt: dt_s, to: (time_s * 120.).round() as u32,
            keep: 0, timer: None }, Some(&target))?;
        Ok(FrameIdentity { instance:id, resource_epoch:inst.resource_epoch, time_s })
    }
}
pub mod bundle;
pub mod embed;
pub mod events;
pub mod params;
pub mod pick;
pub use loft_format::replay;
pub mod tier0;
#[doc(hidden)]
pub use loft_std::promotion;

#[cfg(test)]
mod tests;
pub mod command;
