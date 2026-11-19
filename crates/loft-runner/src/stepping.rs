use super::*;
impl Runner {
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
