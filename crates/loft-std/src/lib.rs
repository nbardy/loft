//! Reusable host algorithms. GPU table ownership and command encoding stay
//! with the backend.
pub mod compute_twin;
pub mod params;
#[cfg(not(target_arch = "wasm32"))]
pub mod promotion;
pub mod events;
pub mod inputs;
pub mod pick;
pub mod stepping;
pub mod bundle;
