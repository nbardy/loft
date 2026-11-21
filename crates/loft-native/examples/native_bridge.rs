#[cfg(target_os = "macos")]
#[path = "native_bridge/benchmark.rs"]
mod benchmark;

#[cfg(target_os = "macos")]
fn main() {
    benchmark::run();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("native_bridge requires macOS");
}
