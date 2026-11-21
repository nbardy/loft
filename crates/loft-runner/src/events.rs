pub use loft_wgpu::outbox::{Outbox, OutboxEvent, capacity};
use super::*;
impl Runner {
    /// Take the oldest completed outbox eligible for the two-frame readback delay.
    pub fn drain_events(&mut self, id: InstanceId) -> Result<(Vec<OutboxEvent>, u32)> {
        let _ = self.device.poll(wgpu::PollType::Poll);
        self.instance(id)?
            .outbox
            .as_mut()
            .map_or_else(|| Ok((vec![], 0)), Outbox::drain)
    }
}
