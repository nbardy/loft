//! Reusable host algorithms. GPU table ownership and command encoding stay
//! with the backend.
pub mod compute_twin;
pub mod params;
pub mod events;
pub mod inputs;
pub mod pick;
pub mod stepping;
pub mod bundle;
pub mod error;
pub mod frame;
pub mod mips;

/// Run `prepare` on its own thread; the runner keeps threads out of its own crate.
#[cfg(not(target_arch = "wasm32"))]
pub fn worker<T: Send + 'static>(prepare: impl FnOnce() -> T + Send + 'static) -> std::io::Result<std::sync::mpsc::Receiver<T>> {
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::Builder::new().name("loft-pipeline".into()).spawn(move || { let _ = send.send(prepare()); })?;
    Ok(receive)
}
