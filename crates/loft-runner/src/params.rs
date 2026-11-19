//! The fixed group-0 parameter buffer and its deferred upload.
use super::*;

pub(super) use loft_std::params::Params;
impl Runner {
    pub fn params(&self, inst: InstanceId) -> &[ParamMeta] {
        self.instances.get(inst.0 as usize).map_or(&[], |i| i.decl.params.as_slice())
    }

    pub fn set_param(&mut self, inst: InstanceId, id: ParamId, values: &[f32]) -> Result<()> {
        let inst = self.instance(inst)?;
        let meta = inst.decl.params.iter().find(|p| p.id == id).ok_or(RunnerError::UnknownResource)?;
        inst.compute.params.set(meta, values)
    }
}
