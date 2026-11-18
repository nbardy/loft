//! Loft's compiler core. The program driver is the integrator's (lean Rust plan §8.2); this file only declares modules.
pub mod cache;
pub mod check;
pub mod draw;
pub mod emit;
pub mod grad;
pub mod interval;
pub mod kernel;
pub mod layout;
pub mod prim;
pub mod rate;
pub mod schedule;
pub mod solve;
pub mod tape;
