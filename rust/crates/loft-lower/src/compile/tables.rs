//! Lane B. src/compile/tables.ts, translated 1:1.
use super::Fields;
use crate::error::Error;
use crate::input::Input;

/// The tables and slots after replaying the tape's registrations and declarations in TS order; lane B defines it.
pub struct Replayed;

/// Lane B's entry: replays the job's slot registrations and table declarations (src/compile/bind.ts `Slots`,
/// `Tables.declare`), lays the tables out and pushes its Artefact fields (inputs, inputFloats, params, curves, dynamic,
/// statics, stages, compute, recurrences, tables, and the mixed tables).
pub fn replay(_input: &Input, _fields: &mut Fields) -> Result<Replayed, Error> {
    Err(Error::Unported { module: "src/compile/tables.ts", lane: "B" })
}
