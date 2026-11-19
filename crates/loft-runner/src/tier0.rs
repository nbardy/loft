use super::*;
pub(super) use loft_std::edits::Edits;
impl Runner {
    pub fn register_tier0(&mut self, id: InstanceId, kind: KindId, pass: PassId) -> Result<()> {
        let i = self.instance(id)?;
        i.edits.register(&i.decl, &i.compute, kind, pass)
    }
    pub fn hot_swap_tape(&mut self, id: InstanceId, kind: KindId, tape: &[u32]) -> Result<()> {
        let queue = self.queue.clone();
        let i = self.instance(id)?;
        i.edits.edit(kind, tape, &mut i.compute, |id| i.tables[&id].bytes(), &queue)
    }
}
