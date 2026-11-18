// An N-API addon resolves napi_* from the process that loads it (Bun or Node). macOS links a cdylib with every
// symbol defined unless told otherwise; Linux allows undefined symbols in a shared object by default.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-undefined,dynamic_lookup");
    }
}
