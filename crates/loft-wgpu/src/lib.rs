//! wgpu resource preparation for the host-owned runner.
pub use loft_std::{bundle, error, events, params, stepping};

pub mod attachments;
pub mod bindings;
pub mod frame;
pub mod interop;
pub mod mips;
pub mod outbox;
pub mod pick;
pub mod schedule;
pub mod storage;
#[cfg(all(target_arch = "wasm32", feature = "web-sys"))]
pub mod web;

pub mod command;
pub mod readback;

pub fn allocate(d: &wgpu::Device, size: u64, usage: wgpu::BufferUsages) -> std::sync::Arc<wgpu::Buffer> {
    std::sync::Arc::new(d.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: size.max(4),
        usage,
        mapped_at_creation: false,
    }))
}
pub fn timed<T>(clock: Option<fn() -> std::time::Duration>, create: impl FnOnce() -> T) -> (T, f64) {
    let start = clock.map(|now| now());
    let value = create();
    let ms = clock.zip(start).map_or(f64::NAN, |(now, start)| (now() - start).as_secs_f64() * 1e3);
    (value, ms)
}


pub mod ray_query;
