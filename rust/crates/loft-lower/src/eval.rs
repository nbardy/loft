//! Lane G2. src/expr.ts `compileJs` (the CPU reading of a graph, JS-exact f64 with `Math.fround` where TS rounds) and
//! src/grad.ts.

use crate::error::Error;
use crate::input::Input;

/// `probe.eval` (tools/port/ts_side.ts `evalProbe`): each closed program root read at rest, as f64 bits in hex.
pub fn probe(_input: &Input) -> Result<String, Error> {
    Err(Error::Unported { module: "src/expr.ts compileJs", lane: "G2" })
}
