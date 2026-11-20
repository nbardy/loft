//! CPU twin execution and checkpoint scheduling, with no device.
use crate::compute_twin as twin;
use loft_abi::{AdvanceArgs, advance};
use loft_ir::*;
use std::{collections::BTreeMap, ops::Range};

pub use crate::error::RunnerError as Error;
use crate::error::Result;
pub enum Tick {
    Dispatch(Range<u32>, AdvanceArgs, bool),
    Checkpoint(usize),
}

const ARGUMENTS: &str = "loft_advance_args";
type Buffers = BTreeMap<(u32, u32), Vec<u8>>;

pub struct Fused {
    pub module: naga::Module,
    pub entry: String,
    pub rows: TableId,
    pub args: PassBinding,
    pub workgroup: u32,
    pub max_ticks: u32,
}
impl Fused {
    pub fn reflect(ir: &naga::Module, pass: &PassDecl) -> Result<Self> {
        let (entry, max_ticks, instances) = match &pass.run {
            PassRun::ComputeFusedTicks { entry, max_ticks, instances, .. }
            | PassRun::ComputeNonDetTicks { entry, max_ticks, instances, .. } => (entry, max_ticks, instances),
            _ => return Err(Error::AbiMismatch),
        };
        let e = ir
            .entry_points
            .iter()
            .find(|e| e.name == *entry && e.stage == naga::ShaderStage::Compute)
            .ok_or(Error::AbiMismatch)?;
        if *max_ticks == 0 || *max_ticks > AdvanceArgs::MAX_TICKS || e.workgroup_size[1..] != [1, 1]
        {
            return Err(Error::AbiMismatch);
        }
        let binding = ir
            .global_variables
            .iter()
            .find(|(_, v)| v.name.as_deref() == Some(ARGUMENTS))
            .and_then(|(_, v)| v.binding.as_ref())
            .ok_or(Error::AbiMismatch)?;
        let args = *pass
            .bindings
            .iter()
            .find(|b| {
                u32::from(b.group) == binding.group
                    && b.slot == binding.binding
                    && b.access == BindAccess::ReadStorage
            })
            .ok_or(Error::AbiMismatch)?;
        if !pass
            .bindings
            .iter()
            .any(|b| b.resource == *instances && b.access == BindAccess::ReadWriteStorage)
        {
            return Err(Error::AbiMismatch);
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

pub fn key(b: PassBinding) -> (u32, u32) {
    (u32::from(b.group), b.slot)
}

pub fn arguments(inst: InstanceId, rows: &Range<u32>, ticks: AdvanceArgs) -> [u8; 24] {
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

pub fn chunks(from: u32, to: u32, max: u32, keep: u32) -> impl Iterator<Item = AdvanceArgs> {
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
pub struct History {
    pub ticks: BTreeMap<PassId, Vec<u32>>,
    pub saved: Vec<Option<u32>>,
    next: usize,
    pub returned: Vec<u8>,
}
impl History {
    pub fn seek(&mut self, pass: PassId, active: u32) -> &mut [u32] {
        let ticks = self.ticks.entry(pass).or_default();
        if ticks.len() < active as usize {
            ticks.resize(active as usize, 0);
        }
        &mut ticks[..active as usize]
    }
    fn retain(&mut self, slots: usize, tick: u32) -> Option<usize> {
        if slots == 0 {
            return None;
        }
        self.saved.resize(slots, None);
        let i = self.next;
        self.saved[i] = Some(tick);
        self.next = (i + 1) % slots;
        Some(i)
    }

    pub fn advance(
        &mut self,
        pass: PassId,
        active: u32,
        to: u32,
        max: u32,
        keep: u32,
        slots: usize,
        mut record: impl FnMut(Tick),
    ) {
        let earliest = self.seek(pass, active).iter().copied().min().unwrap_or(to);
        for span in chunks(earliest, to, max, keep) {
            let cursors = self.seek(pass, active);
            let mut first = 0;
            while first < active {
                let from = cursors[first as usize];
                let last = (first + 1..active)
                    .find(|&row| cursors[row as usize] != from)
                    .unwrap_or(active);
                if from < span.to_tick || earliest == to {
                    // The outer window bounds every row's advance, including a seek to its current tick.
                    let args = advance(from, span.to_tick, keep).expect("validated tick window");
                    let final_dispatch = span.to_tick == to
                        && cursors[last as usize..].iter().all(|&tick| tick == to);
                    record(Tick::Dispatch(first..last, args, final_dispatch));
                    cursors[first as usize..last as usize].fill(args.to_tick);
                }
                first = last;
            }
            if keep != 0
                && span.to_tick > span.from_tick
                && span.to_tick % keep == 0
                && cursors.iter().all(|&tick| tick == span.to_tick)
                && let Some(slot) = self.retain(slots, span.to_tick)
            {
                record(Tick::Checkpoint(slot));
            }
        }
    }

    pub fn step(
        &mut self,
        f: &Fused,
        pass: PassId,
        active: u32,
        rows: Range<u32>,
        ticks: u32,
        inst: InstanceId,
        buffers: &mut Buffers,
    ) -> Result<()> {
        let cursors = self.seek(pass, active);
        let mut updated = cursors.to_vec();
        let mut first = rows.start;
        while first < rows.end {
            let start = cursors[first as usize];
            let end = start.checked_add(ticks).ok_or(Error::Capacity)?;
            let last = (first + 1..rows.end)
                .find(|&i| cursors[i as usize] != start)
                .unwrap_or(rows.end);
            for args in chunks(start, end, f.max_ticks, 0) {
                run(f, inst, first..last, args, buffers)?;
            }
            updated[first as usize..last as usize].fill(end);
            first = last;
        }
        cursors.copy_from_slice(&updated);
        Ok(())
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
        .ok_or(Error::UnknownResource)?;
    let module = match decl.run {
        PassRun::ComputeFusedTicks { module, .. } => module,
        PassRun::ComputeNonDetTicks { .. } => return Err(Error::Unsupported("NonDet ticks require GPU execution")),
        _ => return Err(Error::AbiMismatch),
    };
    let blob = bundle
        .modules
        .iter()
        .find(|b| b.id == module)
        .ok_or(Error::UnknownResource)?;
    let code = std::str::from_utf8(&blob.bytes).map_err(|_| Error::AbiMismatch)?;
    let ir = naga::front::wgsl::parse_str(code)
        .map_err(|e| Error::InvalidShader(e.emit_to_string(code)))?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&ir)
    .map_err(|e| Error::InvalidShader(e.to_string()))?;
    let f = Fused::reflect(&ir, decl)?;
    let mut buffers = BTreeMap::from([(
        (0, 2),
        crate::params::Params::new(&bundle.params, u32::MAX.into())?
            .bytes()
            .to_vec(),
    )]);
    for b in decl.bindings.iter().filter(|b| **b != f.args) {
        buffers.insert(
            key(*b),
            tables
                .get(&b.resource)
                .ok_or(Error::UnknownResource)?
                .clone(),
        );
    }
    let decl_rows = bundle
        .tables
        .iter()
        .find(|t| t.id == f.rows)
        .ok_or(Error::UnknownResource)?;
    if rows.start > rows.end
        || rows.end > decl_rows.capacity
        || advance(ticks.from_tick, ticks.to_tick, ticks.keep_stride).is_none()
        || ticks.to_tick - ticks.from_tick > f.max_ticks
    {
        return Err(Error::Capacity);
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

fn run(
    f: &Fused,
    id: InstanceId,
    rows: Range<u32>,
    ticks: AdvanceArgs,
    buffers: &mut Buffers,
) -> Result<()> {
    buffers.insert(key(f.args), arguments(id, &rows, ticks).into());
    twin::dispatch(&f.module, &f.entry, rows.end - rows.start, buffers).map_err(|err| match err {
        twin::Error::Unsupported => {
            Error::Unsupported("operation has no deterministic CPU implementation")
        }
        twin::Error::Capacity => Error::Capacity,
        twin::Error::MissingEntry => Error::UnknownResource,
    })
}

impl History {
    pub fn step_live(&mut self, f: &Fused, decl: &PassDecl, active: u32, stride_bytes: u32,
        rows: Range<u32>, ticks: u32, inst: InstanceId, outbox: Option<TableId>, params: &[u8],
        mut read: impl FnMut(TableId) -> Result<Vec<u8>>, mut write: impl FnMut(TableId, &[u8]) -> Result<()>) -> Result<&[u8]> {
        let pass = decl.id;
            if rows.start > rows.end || rows.end > active {
                return Err(Error::Capacity);
            }
            if rows.is_empty() {
                self.returned.clear();
                return Ok(&self.returned);
            }
            let mut buffers = BTreeMap::new();
            for b in decl.bindings.iter().filter(|b| **b != f.args) {
                let mut bytes =
                    read(b.resource)?;
                if outbox == Some(b.resource) {
                    let n = crate::events::capacity(bytes.len() as u64)?;
                    bytes[4..8].copy_from_slice(&n.to_le_bytes());
                }
                buffers.insert(key(*b), bytes);
            }
            buffers.insert((0, 2), params.to_vec());
            self.step(f, pass, active, rows.clone(), ticks, inst, &mut buffers)?;
            let state_binding = decl
                .bindings
                .iter()
                .find(|b| b.resource == f.rows)
                .ok_or(Error::AbiMismatch)?;
            let stride = stride_bytes as usize;
            let byte_range = rows.start as usize * stride..rows.end as usize * stride;
            for binding in decl
                .bindings
                .iter()
                .filter(|b| b.access == BindAccess::ReadWriteStorage)
            {
                write(binding.resource, &buffers[&key(*binding)])?;
            }
            self.returned = buffers[&key(*state_binding)][byte_range].to_vec();
            Ok(&self.returned)
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

impl History {
    pub fn checkpoint(&self, tick: u32) -> Result<usize> {
        self.saved.iter().position(|t| *t == Some(tick)).ok_or(Error::UnknownResource)
    }
    pub fn restored(&mut self, tick: u32, mut writes_state: impl FnMut(PassId)->bool) {
        for (&id, rows) in &mut self.ticks {
            if writes_state(id) { rows.fill(tick); }
        }
        // Rewinding invalidates snapshots on the abandoned timeline.
        for saved in &mut self.saved {
            if saved.is_some_and(|t| t > tick) { *saved = None; }
        }
    }
}
