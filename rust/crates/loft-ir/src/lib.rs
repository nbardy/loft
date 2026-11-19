//! loft-ir: the graph arena and op typing, the job wire and its canonical print, the bundle file, the identity hash,
//! and the one Error (plan 2026-11-19 §1.2). No wgpu, and no float arithmetic: numbers are carried as bits.
#![deny(clippy::float_arithmetic)]
pub mod bundle;
pub mod error;
pub mod frame;
pub mod graph;
pub mod identity;
pub mod pass;
pub mod print;
pub mod wire;

pub use error::Error;
