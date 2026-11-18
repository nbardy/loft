//! The N-API addon (plan §3.5), hand-written: one entry, `run(job: Uint8Array) -> string`, plus `stamp`, the version
//! handshake src/core/native.ts checks at load. The job is borrowed in place, never copied by the binding. The answer
//! is JSON, `{"Ok": report}` or `{"Err": {kind, code, message, error}}`, and a panic inside is caught and answered as
//! `Internal`, so the JS process lives. Wave 1 decodes and validates; the integrator wires loft-core's program driver
//! into `drive`.
use loft_ir::error::{Error, Internal};
use loft_ir::graph::Program;
use loft_ir::wire::{self, WIRE};
use serde::Serialize;
use std::panic::{self, AssertUnwindSafe};
use std::time::Instant;

/// What native.ts expects, byte for byte. Bump the abi number with any change to `run`'s arguments or answer.
const STAMP: &str = concat!("loft-node abi 1 wire ", 1);
const _: () = assert!(WIRE == 1, "STAMP names the wire version");

#[derive(Serialize)]
struct Report {
    program: Program,
    nodes: usize,
    decls: usize,
    /// Rust's own time on the job, so the caller can tell the crossing from the work.
    decode_ns: u64,
}

#[derive(Serialize)]
enum Answer<T> {
    Ok(T),
    Err { kind: &'static str, code: u8, message: String, error: Error },
}

fn drive(job: &[u8]) -> Result<Report, Error> {
    let t = Instant::now();
    let job = wire::decode(job)?;
    let decode_ns = t.elapsed().as_nanos() as u64;
    let g = &job.graph;
    let decls = g.fns.len() + g.raws.len() + g.loops.len() + g.kernels.len() + g.params.len() + g.inputs.len();
    let report = Report { program: job.program, nodes: g.nodes.len(), decls, decode_ns };
    match job.program {
        Program::Scene | Program::Solve | Program::Evaluate | Program::Check => Ok(report),
    }
}

/// Runs `f` with panics caught, and answers in JSON.
fn guard<T: Serialize>(f: impl FnOnce() -> Result<T, Error>) -> String {
    let result = panic::catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|p| {
        let msg = p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned());
        Err(Error::Internal(Internal::Panic(msg.unwrap_or_else(|| "a panic without a message".into()).into())))
    });
    let answer = match result {
        Ok(t) => Answer::Ok(t),
        Err(e) => Answer::Err { kind: e.kind(), code: e.code(), message: e.to_string(), error: e },
    };
    // An answer is plain data and always serialises; this literal is the typed Internal for the impossible case.
    serde_json::to_string(&answer).unwrap_or_else(|_| {
        r#"{"Err":{"kind":"internal","code":1,"message":"answer not serialisable","error":null}}"#.into()
    })
}

#[allow(unsafe_code)]
mod napi {
    use std::ffi::{c_char, c_void};
    use std::ptr;

    type Env = *mut c_void;
    type Value = *mut c_void;
    type Info = *mut c_void;
    type Callback = unsafe extern "C" fn(Env, Info) -> Value;
    const OK: i32 = 0;
    const UINT8_ARRAY: i32 = 1;

    // SAFETY: the N-API C ABI (node_api.h, version 1 functions only), resolved from the host process at load: Bun and
    // Node both export these symbols. On macOS build.rs links with `-undefined dynamic_lookup` for that reason.
    unsafe extern "C" {
        fn napi_get_cb_info(
            env: Env,
            info: Info,
            argc: *mut usize,
            argv: *mut Value,
            this: *mut Value,
            data: *mut *mut c_void,
        ) -> i32;
        fn napi_get_typedarray_info(
            env: Env,
            v: Value,
            ty: *mut i32,
            len: *mut usize,
            data: *mut *mut c_void,
            buf: *mut Value,
            off: *mut usize,
        ) -> i32;
        fn napi_create_string_utf8(env: Env, s: *const c_char, len: usize, out: *mut Value) -> i32;
        fn napi_create_function(
            env: Env,
            name: *const c_char,
            len: usize,
            cb: Callback,
            data: *mut c_void,
            out: *mut Value,
        ) -> i32;
        fn napi_set_named_property(env: Env, obj: Value, name: *const c_char, v: Value) -> i32;
    }

    fn string(env: Env, s: &str) -> Value {
        let mut v = ptr::null_mut();
        // SAFETY: `s` is valid UTF-8 for `s.len()` bytes; N-API copies it before returning.
        unsafe { napi_create_string_utf8(env, s.as_ptr().cast(), s.len(), &mut v) };
        v
    }

    /// The single Uint8Array argument, borrowed for the duration of the call.
    ///
    /// SAFETY: called only from an N-API callback with its own `env` and `info`; the slice is used before the callback
    /// returns, while the JS engine keeps the array alive and does not run JS (so nothing can detach or write it).
    unsafe fn bytes<'a>(env: Env, info: Info) -> Result<&'a [u8], &'static str> {
        let (mut argc, mut argv) = (1usize, [ptr::null_mut(); 1]);
        let (mut ty, mut len, mut data, mut buf, mut off) = (-1, 0usize, ptr::null_mut(), ptr::null_mut(), 0usize);
        // SAFETY: argv has room for argc = 1 value; the out-pointers are live locals.
        unsafe {
            if napi_get_cb_info(env, info, &mut argc, argv.as_mut_ptr(), ptr::null_mut(), ptr::null_mut()) != OK
                || argc < 1
            {
                return Err("run(job) takes one Uint8Array");
            }
            if napi_get_typedarray_info(env, argv[0], &mut ty, &mut len, &mut data, &mut buf, &mut off) != OK
                || ty != UINT8_ARRAY
            {
                return Err("run(job): job is not a Uint8Array");
            }
        }
        // SAFETY: N-API gives `data` pointing at `len` bytes of the array's view; an empty view may have a null pointer.
        Ok(if len == 0 { &[] } else { unsafe { std::slice::from_raw_parts(data.cast::<u8>(), len) } })
    }

    unsafe extern "C" fn run(env: Env, info: Info) -> Value {
        // SAFETY: env and info come from N-API for this callback.
        let job = unsafe { bytes(env, info) };
        string(
            env,
            &super::guard(|| {
                super::drive(job.map_err(|m| super::Error::Internal(super::Internal::Binding(m.into())))?)
            }),
        )
    }

    #[cfg(feature = "probe")]
    unsafe extern "C" fn probe(env: Env, _info: Info) -> Value {
        string(env, &super::guard::<()>(|| panic!("probe: a deliberate panic inside the binding")))
    }

    fn export(env: Env, exports: Value, name: &str, cb: Callback) {
        let mut f = ptr::null_mut();
        // SAFETY: `name` is UTF-8 of the given length; the C name below is NUL-terminated by the caller's literal.
        unsafe {
            napi_create_function(env, name.as_ptr().cast(), name.len(), cb, ptr::null_mut(), &mut f);
            napi_set_named_property(env, exports, [name, "\0"].concat().as_ptr().cast(), f);
        }
    }

    /// N-API's module entry, found by name when the host dlopens the addon.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn napi_register_module_v1(env: Env, exports: Value) -> Value {
        export(env, exports, "run", run);
        #[cfg(feature = "probe")]
        export(env, exports, "panic", probe);
        // SAFETY: a NUL-terminated literal name and a value created on this env.
        unsafe { napi_set_named_property(env, exports, c"stamp".as_ptr(), string(env, super::STAMP)) };
        exports
    }
}
