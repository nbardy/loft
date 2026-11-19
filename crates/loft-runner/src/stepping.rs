use super::*;
use loft_abi::{AdvanceArgs, advance};
use std::ops::Range;
use wgpu::util::DeviceExt;

use loft_std::compute_twin as twin;

const ARGUMENTS: &str = "loft_advance_args";
type Buffers = BTreeMap<(u32, u32), Vec<u8>>;

pub(super) struct Fused {
    pub module: naga::Module,
    pub entry: String,
    pub rows: TableId,
    pub args: PassBinding,
    pub workgroup: u32,
    pub max_ticks: u32,
}
impl Fused {
    pub(super) fn reflect(ir: &naga::Module, pass: &PassDecl) -> Result<Self> {
        let PassRun::ComputeFusedTicks {
            entry,
            max_ticks,
            instances,
            ..
        } = &pass.run
        else {
            return Err(RunnerError::AbiMismatch);
        };
        let e = ir
            .entry_points
            .iter()
            .find(|e| e.name == *entry && e.stage == naga::ShaderStage::Compute)
            .ok_or(RunnerError::AbiMismatch)?;
        if *max_ticks == 0 || *max_ticks > AdvanceArgs::MAX_TICKS || e.workgroup_size[1..] != [1, 1]
        {
            return Err(RunnerError::AbiMismatch);
        }
        let binding = ir
            .global_variables
            .iter()
            .find(|(_, v)| v.name.as_deref() == Some(ARGUMENTS))
            .and_then(|(_, v)| v.binding.as_ref())
            .ok_or(RunnerError::AbiMismatch)?;
        let args = *pass
            .bindings
            .iter()
            .find(|b| {
                u32::from(b.group) == binding.group
                    && b.slot == binding.binding
                    && b.access == BindAccess::ReadStorage
            })
            .ok_or(RunnerError::AbiMismatch)?;
        if !pass
            .bindings
            .iter()
            .any(|b| b.resource == *instances && b.access == BindAccess::ReadWriteStorage)
        {
            return Err(RunnerError::AbiMismatch);
        }
        Ok(Self {
            module: ir.clone(),
            entry: entry.clone(),
            rows: *instances,
            args,
            workgroup: e.workgroup_size[0],
            max_ticks: *max_ticks,
        })
    }
}

fn key(b: PassBinding) -> (u32, u32) {
    (u32::from(b.group), b.slot)
}

fn arguments(inst: InstanceId, rows: &Range<u32>, ticks: AdvanceArgs) -> [u8; 24] {
    [
        ticks.from_tick,
        ticks.to_tick,
        ticks.keep_stride,
        rows.start,
        rows.end,
        inst.0,
    ]
    .map(u32::to_le_bytes)
    .as_flattened()
    .try_into()
    .unwrap()
}

fn chunks(from: u32, to: u32, max: u32, keep: u32) -> impl Iterator<Item = AdvanceArgs> {
    let mut cursor = Some(from);
    std::iter::from_fn(move || {
        let start = cursor?;
        let stop = if keep == 0 {
            u32::MAX
        } else {
            start
                .checked_div(keep)
                .and_then(|n| n.checked_add(1))
                .and_then(|n| n.checked_mul(keep))
                .unwrap_or(u32::MAX)
        };
        let end = to.min(start.saturating_add(max)).min(stop);
        cursor = (end < to).then_some(end);
        advance(start, end, keep)
    })
}

#[derive(Default)]
pub(super) struct History {
    ticks: BTreeMap<PassId, Vec<u32>>,
    saved: Vec<Option<u32>>,
    next: usize,
    returned: Vec<u8>,
}
impl History {
    fn seek(&mut self, pass: PassId, active: u32) -> &mut [u32] {
        let ticks = self.ticks.entry(pass).or_default();
        if ticks.len() < active as usize {
            ticks.resize(active as usize, 0);
        }
        &mut ticks[..active as usize]
    }
    fn retain(&mut self, ring: &[Arc<wgpu::Buffer>], tick: u32) -> Option<usize> {
        if ring.is_empty() {
            return None;
        }
        self.saved.resize(ring.len(), None);
        let i = self.next;
        self.saved[i] = Some(tick);
        self.next = (i + 1) % ring.len();
        Some(i)
    }
}

impl Runner {
    /// Encode the fixed-step passes up to an absolute tick. The caller submits the encoder in order.
    pub fn advance_into(
        &mut self,
        id: InstanceId,
        encoder: &mut wgpu::CommandEncoder,
        to: u32,
        keep: u32,
    ) -> Result<()> {
        let (device, queue) = (self.device.clone(), self.queue.clone());
        let inst = self.instance(id)?;
        inst.edits.update(&device, &mut inst.compute)?;
        inst.compute.encode(
            &device,
            &queue,
            encoder,
            &inst.decl.passes,
            &inst.tables,
            inst.resource_epoch,
            f64::from(to) / 120.,
            AdvanceArgs::DT,
            &mut inst.history,
            &inst.checkpoints,
            inst.decl.state.as_ref().map(|s| s.table),
            id,
            to,
            keep,
            &mut inst.outbox,
            None,
        )
    }

    /// Restore an exact retained tick for the declared state table.
    pub fn restore_checkpoint(&mut self, id: InstanceId, tick: u32) -> Result<()> {
        let (device, queue) = (self.device.clone(), self.queue.clone());
        let inst = self.instance(id)?;
        let state = inst
            .decl
            .state
            .as_ref()
            .ok_or(RunnerError::UnknownResource)?
            .table;
        let slot = inst
            .history
            .saved
            .iter()
            .position(|t| *t == Some(tick))
            .ok_or(RunnerError::UnknownResource)?;
        let slot = &inst.checkpoints[slot];
        let live = inst.tables[&state].bytes()?;
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(slot, 0, live, 0, Some(live.size()));
        queue.submit([encoder.finish()]);
        for (&id, rows) in &mut inst.history.ticks {
            if inst.compute.fused.get(&id).is_some_and(|f| f.rows == state) {
                rows.fill(tick);
            }
        }
        // A subsequent seek can keep new snapshots at the old tick numbers.
        inst.history
            .saved
            .iter_mut()
            .filter(|t| t.is_some_and(|t| t > tick))
            .for_each(|t| *t = None);
        Ok(())
    }

    pub fn step_pass_cpu(
        &mut self,
        inst: InstanceId,
        pass: PassId,
        rows: Range<u32>,
        ticks: u32,
    ) -> Result<&[u8]> {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (inst, pass, rows, ticks);
            Err(RunnerError::Unsupported(
                "synchronous GPU readback on WebGPU",
            ))
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (device, queue) = (self.device.clone(), self.queue.clone());
            let instance = self.instance(inst)?;
            let f = instance
                .compute
                .fused
                .get(&pass)
                .ok_or(RunnerError::Unsupported("pass is not row local"))?;
            let decl = instance
                .decl
                .passes
                .iter()
                .find(|p| p.id == pass)
                .ok_or(RunnerError::UnknownResource)?;
            let state = &instance.tables[&f.rows];
            if rows.start > rows.end || rows.end > state.active {
                return Err(RunnerError::Capacity);
            }
            if rows.is_empty() {
                instance.history.returned.clear();
                return Ok(&instance.history.returned);
            }
            let mut buffers = BTreeMap::new();
            for b in decl.bindings.iter().filter(|b| **b != f.args) {
                let mut bytes =
                    read_buffer(&device, &queue, instance.tables[&b.resource].bytes()?)?;
                if instance.compute.outbox == Some(b.resource) {
                    let n = events::capacity(bytes.len() as u64)?;
                    bytes[4..8].copy_from_slice(&n.to_le_bytes());
                }
                buffers.insert(key(*b), bytes);
            }
            buffers.insert((0, 2), instance.compute.params.bytes().to_vec());
            let cursors = instance.history.seek(pass, state.active);
            let mut first = rows.start;
            let mut updated = cursors.to_vec();
            while first < rows.end {
                let start = cursors[first as usize];
                let end = start.checked_add(ticks).ok_or(RunnerError::Capacity)?;
                let last = (first + 1..rows.end)
                    .find(|&i| cursors[i as usize] != start)
                    .unwrap_or(rows.end);
                for a in chunks(start, end, f.max_ticks, 0) {
                    run(f, inst, first..last, a, &mut buffers)?;
                }
                updated[first as usize..last as usize].fill(end);
                first = last;
            }
            let state_binding = decl
                .bindings
                .iter()
                .find(|b| b.resource == f.rows)
                .ok_or(RunnerError::AbiMismatch)?;
            let stride = state.decl.stride_bytes as usize;
            let byte_range = rows.start as usize * stride..rows.end as usize * stride;
            for binding in decl
                .bindings
                .iter()
                .filter(|b| b.access == BindAccess::ReadWriteStorage)
            {
                let buffer = instance.tables[&binding.resource].bytes()?;
                queue.write_buffer(buffer, 0, &buffers[&key(*binding)]);
            }
            cursors.copy_from_slice(&updated);
            instance.history.returned = buffers[&key(*state_binding)][byte_range].to_vec();
            Ok(&instance.history.returned)
        }
    }

    /// Evaluate a fixed-step pass without a device. The caller owns the packed table bytes.
    pub fn step_bundle_cpu(
        bundle: &Bundle,
        pass: PassId,
        tables: &mut BTreeMap<TableId, Vec<u8>>,
        inst: InstanceId,
        rows: Range<u32>,
        ticks: AdvanceArgs,
    ) -> Result<()> {
        let decl = bundle
            .passes
            .iter()
            .find(|p| p.id == pass)
            .ok_or(RunnerError::UnknownResource)?;
        let PassRun::ComputeFusedTicks { module, .. } = decl.run else {
            return Err(RunnerError::AbiMismatch);
        };
        let blob = bundle
            .modules
            .iter()
            .find(|b| b.id == module)
            .ok_or(RunnerError::UnknownResource)?;
        let code = std::str::from_utf8(&blob.bytes).map_err(|_| RunnerError::AbiMismatch)?;
        let ir = naga::front::wgsl::parse_str(code)
            .map_err(|e| RunnerError::InvalidShader(e.emit_to_string(code)))?;
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&ir)
        .map_err(|e| RunnerError::InvalidShader(e.to_string()))?;
        let f = Fused::reflect(&ir, decl)?;
        let mut buffers = BTreeMap::from([((0, 2), params::Params::new(&bundle.params, u32::MAX.into())?.bytes().to_vec())]);
        for b in decl.bindings.iter().filter(|b| **b != f.args) {
            buffers.insert(
                key(*b),
                tables
                    .get(&b.resource)
                    .ok_or(RunnerError::UnknownResource)?
                    .clone(),
            );
        }
        let decl_rows = bundle
            .tables
            .iter()
            .find(|t| t.id == f.rows)
            .ok_or(RunnerError::UnknownResource)?;
        if rows.start > rows.end
            || rows.end > decl_rows.capacity
            || advance(ticks.from_tick, ticks.to_tick, ticks.keep_stride).is_none()
            || ticks.to_tick - ticks.from_tick > f.max_ticks
        {
            return Err(RunnerError::Capacity);
        }
        run(&f, inst, rows, ticks, &mut buffers)?;
        for binding in decl
            .bindings
            .iter()
            .filter(|b| b.access == BindAccess::ReadWriteStorage)
        {
            tables.insert(binding.resource, buffers.remove(&key(*binding)).unwrap());
        }
        Ok(())
    }
}

fn run(
    f: &Fused,
    id: InstanceId,
    rows: Range<u32>,
    ticks: AdvanceArgs,
    buffers: &mut Buffers,
) -> Result<()> {
    buffers.insert(key(f.args), arguments(id, &rows, ticks).into());
    twin::dispatch(&f.module, &f.entry, rows.end - rows.start, buffers).map_err(|err| match err {
        twin::Error::Unsupported => RunnerError::Unsupported("operation has no deterministic CPU implementation"),
        twin::Error::Capacity => RunnerError::Capacity,
        twin::Error::MissingEntry => RunnerError::UnknownResource,
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn read_buffer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
) -> Result<Vec<u8>> {
    let read = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: buffer.size(),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_buffer_to_buffer(buffer, 0, &read, 0, Some(read.size()));
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    encoder.map_buffer_on_submit(&read, wgpu::MapMode::Read, .., move |r| {
        let _ = tx.send(r);
    });
    queue.submit([encoder.finish()]);
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|_| RunnerError::Unsupported("readback failed"))?;
    rx.recv()
        .ok()
        .and_then(std::result::Result::ok)
        .ok_or(RunnerError::Unsupported("readback failed"))?;
    let mapped = read
        .get_mapped_range(..)
        .map_err(|_| RunnerError::Unsupported("readback failed"))?;
    Ok(mapped.to_vec())
}

impl encode::ComputeCache {
    pub(super) fn encode_fused(
        &self,
        device: &wgpu::Device,
        enc: &mut wgpu::CommandEncoder,
        tables: &BTreeMap<TableId, Table>,
        pass: PassId,
        id: InstanceId,
        to: u32,
        keep: u32,
        history: &mut History,
        ring: &[Arc<wgpu::Buffer>],
        state: Option<TableId>,
    ) -> Result<()> {
        let f = &self.fused[&pass];
        let table = &tables[&f.rows];
        let cursors = history.seek(pass, table.active);
        let earliest = cursors.iter().copied().min().unwrap_or(to);
        let keep = if state == Some(f.rows) && !ring.is_empty() {
            keep
        } else {
            0
        };
        let span = chunks(earliest, to, f.max_ticks, keep);
        for a in span {
            let mut first = 0;
            while first < table.active {
                let from = history.ticks[&pass][first as usize];
                let last = (first + 1..table.active)
                    .find(|&i| history.ticks[&pass][i as usize] != from)
                    .unwrap_or(table.active);
                if from < a.to_tick || earliest == to {
                    let a = advance(from, a.to_tick, keep).ok_or(RunnerError::Capacity)?;
                    let args = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: None,
                        contents: &arguments(id, &(first..last), a),
                        usage: wgpu::BufferUsages::COPY_SRC,
                    });
                    enc.copy_buffer_to_buffer(
                        &args,
                        0,
                        tables[&f.args.resource].bytes()?,
                        0,
                        Some(24),
                    );
                    self.encode_dispatch(enc, pass, [(last - first).div_ceil(f.workgroup), 1, 1]);
                    history.ticks.get_mut(&pass).unwrap()[first as usize..last as usize]
                        .fill(a.to_tick);
                }
                first = last;
            }
            if keep != 0
                && a.to_tick > a.from_tick
                && a.to_tick % keep == 0
                && history.ticks[&pass]
                    .iter()
                    .take(table.active as usize)
                    .all(|&t| t == a.to_tick)
                && let Some(slot) = history.retain(ring, a.to_tick)
            {
                let live = table.bytes()?;
                enc.copy_buffer_to_buffer(live, 0, &ring[slot], 0, Some(live.size()));
            }
        }
        Ok(())
    }

    pub(super) fn check_fused(
        &self,
        id: PassId,
        tables: &BTreeMap<TableId, Table>,
        to: u32,
        history: &mut History,
    ) -> Result<()> {
        let f = &self.fused[&id];
        if tables[&f.args.resource].bytes()?.size() < 24
            || history
                .seek(id, tables[&f.rows].active)
                .iter()
                .any(|&t| t > to)
        {
            return Err(RunnerError::Capacity);
        }
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn checkpoint_windows() {
    let windows: Vec<_> = chunks(58, 130, 64, 60)
        .map(|a| (a.from_tick, a.to_tick))
        .collect();
    assert_eq!(windows, [(58, 60), (60, 120), (120, 130)]);
    assert_eq!(chunks(8, 8, 64, 0).count(), 1);
}
