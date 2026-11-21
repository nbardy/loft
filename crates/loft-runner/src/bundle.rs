pub use loft_format::bundle::*;

impl crate::Runner {
    pub fn load_bundle_bytes(&mut self, bytes: &[u8]) -> crate::Result<loft_ir::InstanceId> {
        let bundle = decode(bytes).map_err(crate::RunnerError::Bundle)?;
        self.load_bundle(&bundle)
    }
}
