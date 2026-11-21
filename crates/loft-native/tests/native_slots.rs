#![cfg(target_os = "macos")]
#[path = "../examples/native_bridge/slots.rs"]
mod slots;

#[test]
fn metal_passthrough_uses_generated_slots() {
    slots::check(&loft_native::gpu().unwrap());
}
