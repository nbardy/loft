use super::*;
pub use loft_ir::PickResult;
pub(super) use loft_wgpu::pick::Source;
impl Runner {
    pub fn set_pick_source(&mut self, id: InstanceId, visibility: &wgpu::Texture, depth: &wgpu::Texture, inv_vp: [[f32; 4]; 4]) -> Result<()> {
        let d = self.device.clone();
        Source::publish(&mut self.instance(id)?.pick_source, &d, visibility, depth, inv_vp)
    }

    pub fn pick_async(&mut self, id: InstanceId, px: [u32; 2], cb: Box<dyn FnOnce(Result<PickResult>) + Send>) -> Result<()> {
        let (d, q) = (self.device.clone(), self.queue.clone());
        self.instance(id)?.pick_source.as_ref().ok_or(RunnerError::UnknownResource)?.pick_async(&d, &q, px, |enc, pipeline, group| {
            command::Pass::Compute(vec![command::Dispatch { pipeline, groups: vec![group], work: command::Work::Direct([1; 3]) }]).record(enc, None);
        }, cb)
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn pick_sync(&mut self, id: InstanceId, px: [u32; 2]) -> Result<PickResult> {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        self.pick_async(
            id,
            px,
            Box::new(move |result| {
                let _ = tx.send(result);
            }),
        )?;
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|_| RunnerError::Unsupported("pick polling failed"))?;
        rx.recv().map_err(|_| RunnerError::Readback)?

    }
}
