//! The N-API addon (plan §3.5), hand-written: one entry, `run(job: Uint8Array, limits: BigUint64Array) -> string`
//! (limits per `Meter`, in its order; lane 4.2), plus `stamp`, the version
//! handshake src/core/native.ts checks at load. The job is borrowed in place, never copied by the binding. The answer
//! is JSON, `{"Ok": report}` or `{"Err": {kind, code, message, error}}`, and a panic inside is caught and answered as
//! `Internal`, so the JS process lives. Wave 1 decodes and validates; the integrator wires loft-core's program driver
//! into `drive`.
use loft_ir::error::{Error, Internal, Limits, Meter};
use loft_ir::graph::Program;
use loft_ir::wire::{self, WIRE};
use serde::Serialize;
use std::panic::{self, AssertUnwindSafe};
use std::time::Instant;

/// What native.ts expects, byte for byte. Bump the abi number with any change to `run`'s arguments or answer.
const STAMP: &str = concat!("loft-node abi 2 wire ", 1);
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

fn drive(job: &[u8], limits: Limits) -> Result<Report, Error> {
    let t = Instant::now();
    let job = wire::decode(job)?;
    let decode_ns = t.elapsed().as_nanos() as u64;
    let g = &job.graph;
    limits.spend(Meter::Nodes, g.nodes.len() as u64)?;
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
    const BIGUINT64_ARRAY: i32 = 10;

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

    /// Argument `k` of 2, a typed array of type `want`, borrowed for the duration of the call.
    ///
    /// SAFETY: called only from an N-API callback with its own `env` and `info`; the slice is used before the callback
    /// returns, while the JS engine keeps the array alive and does not run JS (so nothing can detach or write it).
    unsafe fn array<'a>(env: Env, info: Info, k: usize, want: i32) -> Result<&'a [u8], &'static str> {
        let (mut argc, mut argv) = (2usize, [ptr::null_mut(); 2]);
        let (mut ty, mut len, mut data, mut buf, mut off) = (-1, 0usize, ptr::null_mut(), ptr::null_mut(), 0usize);
        const USAGE: &str = "run(job, limits) takes a Uint8Array job and a BigUint64Array of 4 limits";
        // SAFETY: argv has room for argc = 2 values; the out-pointers are live locals.
        unsafe {
            if napi_get_cb_info(env, info, &mut argc, argv.as_mut_ptr(), ptr::null_mut(), ptr::null_mut()) != OK
                || argc < 2
                || napi_get_typedarray_info(env, argv[k], &mut ty, &mut len, &mut data, &mut buf, &mut off) != OK
                || ty != want
            {
                return Err(USAGE);
            }
        }
        // `len` counts elements; a BigUint64Array's are 8 bytes.
        let bytes = len * if want == BIGUINT64_ARRAY { 8 } else { 1 };
        // SAFETY: N-API gives `data` pointing at `len` elements of the array's view; an empty view may be null.
        Ok(if bytes == 0 { &[] } else { unsafe { std::slice::from_raw_parts(data.cast::<u8>(), bytes) } })
    }

    unsafe extern "C" fn run(env: Env, info: Info) -> Value {
        // SAFETY: env and info come from N-API for this callback.
        let (job, limits) = unsafe { (array(env, info, 0, UINT8_ARRAY), array(env, info, 1, BIGUINT64_ARRAY)) };
        string(
            env,
            &super::guard(|| {
                let binding = |m: &str| super::Error::Internal(super::Internal::Binding(m.into()));
                let limits = <[u8; 32]>::try_from(limits.map_err(binding)?)
                    .map_err(|_| binding("run(job, limits): limits holds one u64 per Meter, 4"))?;
                let [nodes, tape_ops, boxes, gpu_nanos] =
                    std::array::from_fn(|k| u64::from_le_bytes(limits[8 * k..8 * k + 8].try_into().expect("8 bytes")));
                let limits = super::Limits { nodes, tape_ops, boxes, gpu_nanos };
                super::drive(job.map_err(binding)?, limits)
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
