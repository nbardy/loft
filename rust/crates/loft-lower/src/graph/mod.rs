//! Lane G1. src/expr.ts: node constructors with the TS interning and simplification (`add`, `mul` identities, `fold` in
//! f32, `withIdentities`), substitution by reconstruction, `topo`, `leaves`, `emitWgsl` (canonical names, lane P0) and
//! the three literal spellings (expr.ts `lit`, scene_wgsl.ts `lit`, recur.ts `lit`).

use crate::error::Error;
use crate::input::Input;

/// `probe.graph` (tools/port/ts_side.ts `graphProbe`): the program roots emitted in one `emitWgsl` call with the probe
/// binding, then `out` and `libs` lines.
pub fn probe(_input: &Input) -> Result<String, Error> {
    Err(Error::Unported { module: "src/expr.ts emitWgsl", lane: "G1" })
}
