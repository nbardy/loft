//! loft-lower: the TypeScript compiler's pure lowering, translated module for module (agent_notes/2026-11-19_port/
//! PLAN.md). Input: the job src/record/port.ts writes and the GPU bake outputs. Output: the artefact directory
//! `loft build` writes, byte for byte, and on the way the compiler's Artefact as compile.json (tools/port/sections.ts).
//!
//! One module per lane; a lane edits only its own files. Until a lane lands, its entry point refuses as
//! `Error::Unported`, which tools/port/diff.ts reports by name.
/// Lane LW: src/artefact.ts as Rust types, read from and written as compile.json.
pub mod artefact;
/// Lanes B (bind, rates, tables, recur, compute) and S (index, scene_wgsl, passes, frame): src/compile/*, src/frame/*.
pub mod compile;
/// Lanes D1..D5: src/draw/*, src/soft/*, src/spacetime/* drawables.
pub mod draw;
pub mod error;
/// Lane G2: src/expr.ts `compileJs` (the CPU reading) and src/grad.ts.
pub mod eval;
/// Lane SW: src/host/files.ts `writeBundle`, chunk names, JS-exact JSON.
pub mod files;
/// Lane G1: src/expr.ts constructors, interning, folding, substitution, emit and literal spelling.
pub mod graph;
/// Lane W: the job (src/record/port.ts) decoded into the typed input the compiler reads.
pub mod input;
/// Lane LW: src/host/lower.ts, the pure part of src/host/detail.ts, src/host/manifest.ts.
pub mod lower;
/// Lane SW: src/host/share.ts.
pub mod share;
/// Lane LIB: the shared WGSL library text (one .wgsl source, read by TS and by `include_str!` here).
pub mod wgsl;

pub use error::Error;
