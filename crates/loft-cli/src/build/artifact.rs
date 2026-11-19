//! Supply the CLI camera templates to the compiler's portable bundle lowering.
use loft_ir::Bundle;
pub(super) use loft_passes::bundle::parameters;
use loft_passes::{Compiled, program::Program};

pub(super) fn lower(program: &Program, compiled: &Compiled) -> Result<Bundle, String> {
    let size = (1280, 800);
    loft_passes::bundle::lower(program, compiled, size, |sun, alternate| {
        let alternate =
            alternate.map(|(lo, hi, side)| loft_cli::native::sun_camera(program.sun, lo, hi, side));
        loft_cli::native::frame_bytes(program.camera, program.sun, size, 0., sun, alternate)
    })
}
