//! Reusable host algorithms. GPU table ownership and command encoding stay
//! with the backend.
pub mod compute_twin;
pub mod params;
pub mod promotion;
pub mod events;
pub mod inputs;
pub mod pick;
pub mod stepping;
pub mod bundle;
pub mod error;
#[cfg(all(target_arch = "wasm32", feature = "web-sys"))]
pub mod web;
pub mod bindings;
pub mod edits;
pub mod outbox;
pub mod interop;
pub mod attachments;
pub mod schedule;
pub mod frame;
pub mod mips;
pub mod storage;
