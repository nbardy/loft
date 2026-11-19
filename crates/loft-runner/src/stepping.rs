use super::*;
use loft_abi::AdvanceArgs;
use std::ops::Range;

pub(super) use loft_std::stepping::{Fused, History};
use loft_std::stepping;
#[cfg(not(target_arch = "wasm32"))]
use loft_std::stepping::key;

impl From<stepping::Error> for RunnerError {
    fn from(error: stepping::Error) -> Self {
        use stepping::Error;
        match error {
            Error::Capacity => Self::Capacity,
            Error::AbiMismatch => Self::AbiMismatch,
            Error::UnknownResource => Self::UnknownResource,
            Error::InvalidShader(message) => Self::InvalidShader(message),
            Error::Unsupported(reason) => Self::Unsupported(reason),
        }
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
        self.advance(id, encoder, to, keep, None)
    }

    /// Record an advance and stamp the first compute start and the final compute end at queries 0 and 1.
    pub fn advance_timed_into(
        &mut self, id: InstanceId, encoder: &mut wgpu::CommandEncoder,
        to: u32, keep: u32, timer: &wgpu::QuerySet,
    ) -> Result<()> {
        self.advance(id, encoder, to, keep, Some(timer))
    }

    fn advance(
        &mut self, id: InstanceId, encoder: &mut wgpu::CommandEncoder,
        to: u32, keep: u32, timer: Option<&wgpu::QuerySet>,
    ) -> Result<()> {
        let device = self.device.clone();
        let inst = self.instance(id)?;
        inst.edits.update(&device, &mut inst.compute)?;
        inst.compute.encode(
            &device,
            encoder,
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
            timer,
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
            if inst.compute.fused(id).is_some_and(|f| f.rows == state) {
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
                .fused(pass)
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
            instance.history.step(f, pass, state.active, rows.clone(), ticks, inst, &mut buffers)?;
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
            instance.history.returned = buffers[&key(*state_binding)][byte_range].to_vec();
            Ok(&instance.history.returned)
        }
    }

    /// Evaluate a fixed-step pass without a device. The caller owns the packed table bytes.
    pub fn step_bundle_cpu(
        bundle: &Bundle, pass: PassId, tables: &mut BTreeMap<TableId, Vec<u8>>,
        inst: InstanceId, rows: Range<u32>, ticks: AdvanceArgs,
    ) -> Result<()> {
        Ok(stepping::step_bundle_cpu(bundle, pass, tables, inst, rows, ticks)?)
    }
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

pub(super) fn check_fused(
    fused: &Fused, id: PassId, tables: &BTreeMap<TableId, Table>, to: u32,
    history: &mut History, limit: u32,
) -> Result<()> {
    let table = &tables[&fused.rows];
    let args = tables[&fused.args.resource].bytes()?;
    table.bytes()?;
    if args.size() < 24 || !args.usage().contains(wgpu::BufferUsages::COPY_DST)
        || table.active.div_ceil(fused.workgroup) > limit
        || history.seek(id, table.active).iter().any(|&tick| tick > to)
    {
        return Err(RunnerError::Capacity);
    }
    Ok(())
}

#[cfg(test)]
#[test]
fn checkpoint_windows() {
    let windows: Vec<_> = stepping::chunks(58, 130, 64, 60)
        .map(|a| (a.from_tick, a.to_tick))
        .collect();
    assert_eq!(windows, [(58, 60), (60, 120), (120, 130)]);
    assert_eq!(stepping::chunks(8, 8, 64, 0).count(), 1);
}
