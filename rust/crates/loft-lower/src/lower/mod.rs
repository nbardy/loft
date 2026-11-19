//! Lane LW. src/host/lower.ts `lower`: the Artefact to the manifest, chunks and blobs (docs/artefact_format.md), with
//! src/host/detail.ts `gridBounds`/`slotsUsed` and src/host/manifest.ts's types in manifest key order.
use crate::error::Error;

/// The artefact for a compile.json's text and the GPU bake outputs (tools/port/ts_side.ts input/baked/<name>.bin:
/// `recurrence.<name>` for a baked recurrence's states, `build.<key>` for a compute build's output). Writes `out`.
pub fn lower(_compile_json: &str, _baked: &std::path::Path, _out: &std::path::Path) -> Result<(), Error> {
    Err(Error::Unported { module: "src/host/lower.ts", lane: "LW" })
}
