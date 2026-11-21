use crate::error::*;
use loft_ir::*;
use crate::schedule::Order;
use crate::stepping::{Fused, History, Tick, arguments};
use wgpu::{Buffer, QuerySet};
pub struct Clock<'a> {
    pub id: InstanceId,
    pub time: f64,
    pub dt: f32,
    pub to: u32,
    pub keep: u32,
    pub timer: Option<&'a wgpu::QuerySet>,
}

#[derive(Clone, Copy)]
pub struct Stamps<'a> {
    pub set: &'a QuerySet,
    pub start: Option<u32>,
    pub end: Option<u32>,
}

pub enum Work {
    Direct([u32; 3]),
    Indirect(Buffer, u64),
}

impl Work {
    pub fn resolve<'a>(
        extent: &DispatchExtent,
        limit: u32,
        rows: impl Fn(TableId) -> u32,
        bytes: impl Fn(TableId) -> Result<&'a Buffer>,
    ) -> Result<Work> {
        let work = match *extent {
            DispatchExtent::Fixed(size) => Work::Direct(size),
            DispatchExtent::FromRowCount { table, workgroup_x } => {
                Work::Direct([rows(table).div_ceil(workgroup_x), 1, 1])
            }
            DispatchExtent::Indirect { buf, offset_bytes } => {
                let buf = bytes(buf)?;
                if !buf.usage().contains(wgpu::BufferUsages::INDIRECT) || u64::from(offset_bytes) + 12 > buf.size()
                {
                    return Err(RunnerError::Capacity);
                }
                Work::Indirect(buf.clone(), offset_bytes.into())
            }
        };
        if matches!(work, Work::Direct(size) if size.into_iter().any(|n| n > limit)) {
            return Err(RunnerError::Capacity);
        }
        Ok(work)
    }
}

pub enum Step<'a> {
    Save(usize),
    Run {
        args: [u8; 24],
        work: [u32; 3],
        stamps: Option<Stamps<'a>>,
    },
}
impl Clock<'_> {
    pub fn validate(&self, order: &[Order], active: impl Fn(PassId) -> bool) -> Result<()> {
        if let Some(timer) = self.timer {
            let [Order::Run(id)] = order else {
                return Err(RunnerError::Unsupported("timed advance requires one fixed-step pass"));
            };
            if !active(*id) {
                return Err(RunnerError::Unsupported(
                    "timed advance requires active fixed-step rows",
                ));
            }
            if !matches!(timer.ty(), wgpu::QueryType::Timestamp) || timer.count() < 2 {
                return Err(RunnerError::AbiMismatch);
            }
        }
        Ok(())
    }

    pub fn advance(
        &self,
        history: &mut History,
        id: PassId,
        f: &Fused,
        rows: u32,
        state: Option<&StateStoreDecl>,
        slots: usize,
        mut emit: impl FnMut(Step<'_>),
    ) {
        let keep = state
            .filter(|s| s.table == f.rows && slots != 0)
            .map_or(0, |_| self.keep);
        let mut started = false;
        history.advance(id, rows, self.to, f.max_ticks, keep, slots, |tick| match tick {
            Tick::Checkpoint(i) => emit(Step::Save(i)),
            Tick::Dispatch(rows, ticks, last) => {
                // Metal may omit empty marker passes. Stamp the first and last actual dispatch.
                let stamps = self.timer.filter(|_| !started || last).map(|set| Stamps {
                    set,
                    start: (!started).then_some(0),
                    end: last.then_some(1),
                });
                let work = [(rows.end - rows.start).div_ceil(f.workgroup), 1, 1];
                started = true;
                emit(Step::Run {
                    args: arguments(self.id, &rows, ticks),
                    work,
                    stamps,
                });
            }
        });
    }
}
