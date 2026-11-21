pub use loft_format::bundle::*;

#[cfg(all(target_arch = "wasm32", feature = "web-sys"))]
pub use crate::Runner as WebGpu;

#[cfg(all(target_arch = "wasm32", feature = "web-sys"))]
pub use loft_ir::InstanceId as WebBundle;

impl crate::Runner {
    pub fn load_bundle_bytes(&mut self, bytes: &[u8]) -> crate::Result<loft_ir::InstanceId> {
        let bundle = decode(bytes).map_err(crate::RunnerError::Bundle)?;
        self.load_bundle(&bundle)
    }
}
