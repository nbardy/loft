//! Execution on a caller-owned device.
pub mod backend;
mod bindings;
mod encode;
pub use encode::render_bundle_stale;
mod interop;
mod stepping;
mod resources;
mod inputs;
pub use backend::{ExecutionBackend, WgpuBackend};
pub type Runner = WgpuBackend;

use loft_ir::*;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, PartialEq, Eq)]
pub enum RunnerError {
    UnknownInstance,
    UnknownResource,
    AbiMismatch,
    Readback,
    Bundle(bundle::BundleError),
    Capacity,
    InvalidShader(String),
    Unsupported(&'static str),
}
type Result<T> = std::result::Result<T, RunnerError>;
#[derive(Debug, PartialEq)]
pub struct FrameIdentity {
    pub instance: InstanceId,
    pub resource_epoch: u64,
    pub time_s: f64,
}

struct Table {
    decl: TableDecl,
    buffer: Option<Arc<wgpu::Buffer>>,
    texture: Option<Arc<wgpu::Texture>>,
    texture_only: bool,
    texture_contract: Option<interop::TextureContract>,
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
impl Table {
    fn bytes(&self) -> Result<&wgpu::Buffer> {
        self.buffer.as_deref().ok_or(RunnerError::AbiMismatch)
    }
    fn bounds(&self, n: u32, d: &wgpu::Device) -> Result<u64> {
        let bytes = u64::from(n) * u64::from(self.decl.stride_bytes);
        if bytes > d.limits().max_buffer_size
            || matches!(self.decl.lifetime,TableLifetime::DynamicArena{max_rows} if n>max_rows)
        {
            return Err(RunnerError::Capacity);
        }
        Ok(bytes)
    }
}
impl Instance {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, decl: &Bundle) -> Result<Self> {
        let mut tables = BTreeMap::new();
        for t in &decl.tables {
            let texture_only = decl.passes.iter().flat_map(|p| p.bindings.iter())
                .any(|b| b.resource == t.id && matches!(b.access, BindAccess::SampledTex | BindAccess::StorageTex | BindAccess::BindlessTexArray));
            if t.stride_bytes == 0
                || !t.stride_bytes.is_multiple_of(4)
                || !t.initial.len().is_multiple_of(t.stride_bytes as usize)
                || matches!(t.format,ElemFormat::Struct(i) if decl.struct_layouts.get(i as usize).is_none_or(|s|s.stride_bytes!=t.stride_bytes))
                || matches!(t.format,ElemFormat::F64 if t.stride_bytes < 8)
            {
                return Err(RunnerError::AbiMismatch);
            }
            let active = u32::try_from(t.initial.len() / t.stride_bytes as usize)
                .map_err(|_| RunnerError::Capacity)?;
            if active > t.capacity || tables.contains_key(&t.id) {
                return Err(RunnerError::AbiMismatch);
            }
            let mut table = Table {
                decl: t.clone(),
                buffer: None,
                texture: None,
                texture_only,
                texture_contract: None,
                active,
                epoch: 0,
            };
            let bytes = table.bounds(t.capacity, device)?;
            if texture_only || t.lifetime == TableLifetime::External {
                if active != 0 {
                    return Err(RunnerError::AbiMismatch);
                }
            } else {
                let buf = allocate(device, bytes);
                if active != 0 {
                    queue.write_buffer(&buf, 0, &t.initial)
                }
                table.buffer = Some(buf);
            }
            tables.insert(t.id, table);
        }
        if decl.rows.values().any(|id| !tables.contains_key(id)) {
            return Err(RunnerError::UnknownResource);
        }
        let checkpoints = if let Some(s) = &decl.state {
            let table = tables.get(&s.table).ok_or(RunnerError::UnknownResource)?;
            let size = table.bytes()?.size();
            (0..s.checkpoint_slots)
                .map(|_| allocate(device, size))
                .collect()
        } else {
            Vec::new()
        };
        let compute = encode::ComputeCache::new(device, decl, &tables)?;
        Ok(Self {
            tables,
            decl: decl.clone(),
            compute,
            edits: tier0::Edits::default(),
            resource_epoch: 0,
            checkpoints,
            history: Default::default(),
            outbox: None,
            pick_source: None,
        })
    }
    fn arena(&mut self, k: KindId) -> Result<&mut Table> {
        let id = self.decl.rows.get(&k).ok_or(RunnerError::UnknownResource)?;
        let t = self
            .tables
            .get_mut(id)
            .ok_or(RunnerError::UnknownResource)?;
        if !matches!(t.decl.lifetime, TableLifetime::DynamicArena { .. }) {
            return Err(RunnerError::AbiMismatch);
        }
        Ok(t)
    }
}
impl Runner {
    pub fn attach(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>, caps: HostCaps) -> Self {
        Self {
            device,
            queue,
            caps,
            instances: Vec::new(),
        }
    }
    fn instance(&mut self, id: InstanceId) -> Result<&mut Instance> {
        self.instances
            .get_mut(id.0 as usize)
            .ok_or(RunnerError::UnknownInstance)
    }
    pub fn load_bundle(&mut self, b: &Bundle) -> Result<InstanceId> {
        let id =
            InstanceId(u32::try_from(self.instances.len()).map_err(|_| RunnerError::Capacity)?);
        self.instances
            .push(Instance::new(&self.device, &self.queue, b)?);
        Ok(id)
    }
    pub fn upsert_rows(
        &mut self,
        id: InstanceId,
        kind: KindId,
        start: u32,
        bytes: &[u8],
    ) -> Result<()> {
        let queue = self.queue.clone();
        let t = self.instance(id)?.arena(kind)?;
        if !bytes.len().is_multiple_of(t.decl.stride_bytes as usize) {
            return Err(RunnerError::AbiMismatch);
        }
        let count = u32::try_from(bytes.len() / t.decl.stride_bytes as usize)
            .map_err(|_| RunnerError::Capacity)?;
        let end = start
            .checked_add(count)
            .filter(|&n| n <= t.decl.capacity)
            .ok_or(RunnerError::Capacity)?;
        if !bytes.is_empty() {
            queue.write_buffer(
                t.bytes()?,
                u64::from(start) * u64::from(t.decl.stride_bytes),
                bytes,
            );
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
        let t = inst
            .tables
            .get_mut(&table)
            .ok_or(RunnerError::UnknownResource)?;
        if !matches!(t.decl.lifetime, TableLifetime::DynamicArena { .. }) {
            return Err(RunnerError::AbiMismatch);
        }
        if t.active > n {
            return Err(RunnerError::Capacity);
        }
        let size = t.bounds(n, &device)?;
        let epoch = inst
            .resource_epoch
            .checked_add(1)
            .ok_or(RunnerError::Capacity)?;
        let mut enc = device.create_command_encoder(&Default::default());
        let replace = |enc: &mut wgpu::CommandEncoder, old: &wgpu::Buffer| {
            let buf = allocate(&device, size);
            enc.copy_buffer_to_buffer(old, 0, &buf, 0, Some(old.size().min(buf.size())));
            buf
        };
        t.buffer = Some(replace(&mut enc, t.bytes()?));
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
        let m = decl
            .modules
            .iter_mut()
            .find(|m| m.id == module)
            .ok_or(RunnerError::UnknownResource)?;
        m.bytes = wgsl.as_bytes().into();
        m.target = ShaderTarget::Wgsl;
        m.hash = *blake3::hash(wgsl.as_bytes()).as_bytes();
        let mut replacement = encode::ComputeCache::new(&device, &decl, &inst.tables)?;
        replacement.keep_parameters(&inst.compute);
        inst.decl = decl;
        inst.compute = replacement;
        inst.edits = tier0::Edits::default();
        Ok(())
    }
    pub fn hot_swap_bundle(&mut self, id: InstanceId, b: &Bundle) -> Result<()> {
        let mut next = Instance::new(&self.device, &self.queue, b)?;
        let old = self.instance(id)?;
        next.resource_epoch = old
            .resource_epoch
            .checked_add(1)
            .ok_or(RunnerError::Capacity)?;
        if let (Some(a), Some(b)) = (&old.decl.state, &b.state) {
            if a.layout_hash == b.layout_hash {
                let current = &old.tables[&a.table];
                let new = next
                    .tables
                    .get_mut(&b.table)
                    .ok_or(RunnerError::UnknownResource)?;
                if new.decl.stride_bytes != current.decl.stride_bytes
                    || matches!(new.decl.lifetime, TableLifetime::DynamicArena{max_rows} if max_rows<current.decl.capacity)
                {
                    return Err(RunnerError::AbiMismatch);
                }
                // Content and history belong to the state layout, independent of module IDs and source hashes.
                new.buffer = current.buffer.clone();
                new.active = current.active;
                new.decl.capacity = current.decl.capacity;
                next.checkpoints = std::mem::take(&mut old.checkpoints);
                next.history = std::mem::take(&mut old.history);
                // Ring history survives code/configuration swaps along with its live allocation.
                next.decl.state.as_mut().unwrap().checkpoint_slots = a.checkpoint_slots;
            }
        }
        for t in next.tables.values_mut() {
            t.epoch = next.resource_epoch
        }
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
    pub fn encode_into(
        &mut self,
        id: InstanceId,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        depth: Option<&wgpu::TextureView>,
        time_s: f64,
        dt_s: f32,
    ) -> Result<FrameIdentity> {
        let (device, queue) = (self.device.clone(), self.queue.clone());
        let target = embed::Target {
            color: target,
            depth,
            caps: self.caps,
        };
        let inst = self.instance(id)?;
        inst.edits.update(&device, &mut inst.compute)?;
        inst.compute.encode(
            &device,
            &queue,
            encoder,
            &inst.decl.passes,
            &inst.tables,
            inst.resource_epoch,
            time_s,
            dt_s,
            &mut inst.history,
            &inst.checkpoints,
            inst.decl.state.as_ref().map(|s| s.table),
            id,
            (time_s * 120.).round() as u32,
            0,
            &mut inst.outbox,
            Some(&target),
        )?;
        Ok(FrameIdentity {
            instance: id,
            resource_epoch: inst.resource_epoch,
            time_s,
        })
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
