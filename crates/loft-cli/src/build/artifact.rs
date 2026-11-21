//! Supply the CLI camera templates to the compiler's portable bundle lowering.
use loft_ir::Bundle;
use loft_passes::{Compiled, program::Program};

pub fn lower(program: &Program, compiled: &Compiled) -> Result<Bundle, String> {
    let size = (1280, 800);
    loft_passes::bundle::lower(program, compiled, size, |sun, alternate| {
        let alternate =
            alternate.map(|(lo, hi, side)| crate::native::sun_camera(program.sun, lo, hi, side));
        crate::native::frame_bytes(program.camera, program.sun, size, 0., sun, alternate)
    })
}
