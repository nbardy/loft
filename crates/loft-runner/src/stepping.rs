use super::*;
impl Runner {
    /// Synchronously advance each selected row of a row-local pass by `ticks` on the CPU twin.
    /// `rows` is half-open. Return the resulting row bytes in ascending row order, borrowed
    /// from this runner. Subsequent CPU or GPU steps continue from that state.
    pub fn step_pass_cpu(
        &mut self,
        _inst: InstanceId,
        _pass: PassId,
        _rows: std::ops::Range<u32>,
        _ticks: u32,
    ) -> Result<&[u8]> {
        Err(RunnerError::Unsupported("CPU stepping (Phase 3)"))
    }
}

// The motion lane adds alternating CPU/GPU steps to the signature check below.
#[cfg(test)]
#[test]
fn step_pass_cpu_contract() {
    type Step = fn(
        &mut Runner,
        InstanceId,
        PassId,
        std::ops::Range<u32>,
        u32,
    ) -> std::result::Result<&[u8], RunnerError>;
    let _: Step = Runner::step_pass_cpu;
    let _: Step = <Runner as ExecutionBackend>::step_pass_cpu;
}
