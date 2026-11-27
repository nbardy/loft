//! Fixed-step dispatch windows and checkpoint history.
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

pub struct Fused {
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
            rows: *instances,
            args,
            workgroup: e.workgroup_size[0],
            max_ticks: *max_ticks,
        })
    }
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
