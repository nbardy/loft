//! src/compile/index.ts `compileScene` (lane S) over the modules below. Until lane S lands, `compile` refuses.
use crate::error::Error;
use crate::input::Input;

/// Lane B: src/compile/bind.ts (slots, `emitFn`).
pub mod bind;
/// Lane B: src/compile/compute.ts.
pub mod compute;
/// Lane S: src/frame/pass.ts and src/frame/lamp_shadow.ts (their lowering), src/draw/grid_detail.ts `measureRest`.
pub mod frame;
/// Lane S: src/compile/passes.ts.
pub mod passes;
/// Lane B: src/compile/rates.ts.
pub mod rates;
/// Lane B: src/compile/recur.ts.
pub mod recur;
/// Lane S: src/compile/scene_wgsl.ts.
pub mod scene_wgsl;
/// Lane B: src/compile/tables.ts (registry, packing, levels, pose stages, mixed tables).
pub mod tables;

/// compile.json: the Artefact's fields (src/artefact.ts names) as JSON values, typed arrays as
/// `{"type": "Float32Array", "base64": ...}` (tools/port/sections.ts `plain`). The lanes push fields as they make them,
/// so a run that stops at an unported stage still writes, and the harness still compares, what came before it.
pub type Fields = Vec<(&'static str, serde_json::Value)>;

/// src/compile/index.ts `compileScene` for one job. Lane B's replay of the tape's slot registrations and table
/// declarations comes first (until the drawables are Rust, the TS order of those side effects is on the tape), then
/// lane S's assembly over its result.
pub fn compile(input: &Input, fields: &mut Fields) -> Result<(), Error> {
    let _tables = tables::replay(input, fields)?;
    Err(Error::Unported { module: "src/compile/index.ts", lane: "S" })
}
