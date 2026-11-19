/// Arguments of the fused compute entry shared by live and baked passes (02 §4, 06 §10).
/// The row starts at `from_tick` and finishes at `to_tick`. Both endpoints can be equal.
/// The pass always writes the final state. A positive stride also saves state at each
/// absolute tick divisible by the stride. Zero keeps only the final state.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdvanceArgs {
    pub from_tick: u32,
    pub to_tick: u32,
    pub keep_stride: u32,
}

impl AdvanceArgs {
    pub const ENTRY: &str = "advance";
    pub const MAX_TICKS: u32 = 64;
    pub const DT: f32 = 1.0 / 120.0;
    /// Bind this record through the pass declaration. The compute entry gets its row from gid.x.
    pub const WGSL: &str = "struct AdvanceArgs { from_tick: u32, to_tick: u32, keep_stride: u32 }";
}

/// Form the arguments for one dispatch. Split longer spans into contiguous dispatches.
pub fn advance(from_tick: u32, to_tick: u32, keep_stride: u32) -> Option<AdvanceArgs> {
    (to_tick.checked_sub(from_tick)? <= AdvanceArgs::MAX_TICKS).then_some(AdvanceArgs {
        from_tick,
        to_tick,
        keep_stride,
    })
}
