//! Loft's compiler core. Each entry takes one packed program from Bun and returns one packed result: the boundary is
//! crossed twice per job (`loft_core_call`, then `loft_core_take`), never per node. See agent_notes/2026-11-18_rust_core.md.

pub mod fx;
pub mod jsmath;
pub mod wire;
pub mod tape;
pub mod solve;
pub mod share;

use std::cell::RefCell;
use wire::{Reader, WireError, Writer};

/// What a call returns, as the first byte of its result: the TS side maps each to a typed outcome.
#[repr(u8)]
pub enum Status {
    Ok = 0,
    /// A typed refusal or failure of the kernel (its payload says which); the TS side reports it by name.
    Failed = 1,
    /// Malformed input bytes: a bug in the packer, never a scene's fault.
    Wire = 2,
    /// A panic inside the core: a bug, reported with its message.
    Panic = 3,
}

/// The kernels, by number (src/core/native.ts `Kernel`).
fn dispatch(kind: u32, r: &mut Reader, w: &mut Writer) -> Result<bool, WireError> {
    match kind {
        0 => probe(r, w),
        1 => solve::run(r, w),
        2 => share::run(r, w),
        3 => solve::verify(r, w),
        _ => Err(WireError(format!("no kernel {kind}"))),
    }
}

/// Math parity probe: the platform functions the tape uses, on given inputs, so Bun can compare bits.
fn probe(r: &mut Reader, w: &mut Writer) -> Result<bool, WireError> {
    let xs = r.f64s()?;
    let ys = r.f64s()?;
    r.done()?;
    for code in 0..14 {
        for &x in &xs { w.f64(jsmath::unary(code, x)); }
    }
    for (&x, &y) in xs.iter().zip(ys.iter()) {
        w.f64(jsmath::pow(x, y));
        w.f64(x.atan2(y));
    }
    Ok(true)
}

thread_local! {
    static OUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Runs kernel `kind` on `len` bytes at `ptr`; returns the result's length, fetched with `loft_core_take`.
///
/// # Safety
/// `ptr` must point to `len` readable bytes (or be null with `len` 0).
#[no_mangle]
pub unsafe extern "C" fn loft_core_call(kind: u32, ptr: *const u8, len: usize) -> usize {
    let input: &[u8] = if len == 0 { &[] } else { std::slice::from_raw_parts(ptr, len) };
    let result = std::panic::catch_unwind(|| {
        let mut w = Writer::default();
        w.u8(Status::Ok as u8);
        let mut r = Reader::new(input);
        match dispatch(kind, &mut r, &mut w) {
            Ok(true) => w.b,
            Ok(false) => { w.b[0] = Status::Failed as u8; w.b }
            Err(WireError(m)) => { let mut e = Writer::default(); e.u8(Status::Wire as u8); e.str(&m); e.b }
        }
    });
    let bytes = result.unwrap_or_else(|p| {
        let msg = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
        let mut e = Writer::default();
        e.u8(Status::Panic as u8);
        e.str(&msg);
        e.b
    });
    let n = bytes.len();
    OUT.with(|o| *o.borrow_mut() = bytes);
    n
}

/// Copies the last result into `out` (as long as `loft_core_call` returned) and releases it.
///
/// # Safety
/// `out` must point to that many writable bytes.
#[no_mangle]
pub unsafe extern "C" fn loft_core_take(out: *mut u8) {
    OUT.with(|o| {
        let b = std::mem::take(&mut *o.borrow_mut());
        std::ptr::copy_nonoverlapping(b.as_ptr(), out, b.len());
    });
}

/// The wire version: bumped with any change to a kernel's bytes, checked by src/core/native.ts at load.
#[no_mangle]
pub extern "C" fn loft_core_version() -> u32 { 1 }
