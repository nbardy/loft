//! Replays a kernel input dumped by `LOFT_CORE_DUMP=dir` (src/core/ffi.ts), for timing and profiling the core alone:
//! `loft-core-replay <kernel> <file> [repeats]`.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let kind: u32 = a[1].parse().expect("kernel number");
    let bytes = std::fs::read(&a[2]).expect("input file");
    let reps: usize = a.get(3).map(|s| s.parse().unwrap()).unwrap_or(1);
    let mut n = 0;
    // The fastest call: the machine is shared, and the least time is the one least disturbed.
    let mut ms = f64::INFINITY;
    for _ in 0..reps {
        let t = std::time::Instant::now();
        n = unsafe { loft_core::loft_core_call(kind, bytes.as_ptr(), bytes.len()) };
        ms = ms.min(t.elapsed().as_secs_f64() * 1000.0);
    }
    let mut out = vec![0u8; n];
    unsafe { loft_core::loft_core_take(out.as_mut_ptr()) };
    println!("kernel {kind}: {ms:.2} ms per call (fastest), status {}, {} bytes out", out[0], n);
}
