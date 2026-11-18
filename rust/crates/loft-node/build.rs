// An N-API addon resolves napi_* from the process that loads it (Bun or Node). macOS links a cdylib with every
// symbol defined unless told otherwise; Linux allows undefined symbols in a shared object by default.
//
// LOFT_COMMIT is the commit every bundle names as its `compiler` (plan §1.9). Outside a git checkout it is the named
// value "not-a-git-checkout", which the bundle then carries; it is never left empty.
use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok().filter(|o| o.status.success())?;
    Some(String::from_utf8(out.stdout).ok()?.trim().to_string())
}

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-undefined,dynamic_lookup");
    }
    let commit = git(&["rev-parse", "--short=12", "HEAD"]).unwrap_or_else(|| "not-a-git-checkout".into());
    println!("cargo:rustc-env=LOFT_COMMIT={commit}");
    // Rebuild when HEAD moves: the HEAD file, the branch's ref and packed-refs.
    for path in
        [git(&["rev-parse", "--git-path", "HEAD"]), git(&["symbolic-ref", "-q", "HEAD"]), Some("packed-refs".into())]
            .into_iter()
            .flatten()
    {
        let p = if path.starts_with("refs/") || path == "packed-refs" {
            git(&["rev-parse", "--git-common-dir"]).map(|d| format!("{d}/{path}"))
        } else {
            Some(path)
        };
        if let Some(p) = p {
            println!("cargo:rerun-if-changed={p}");
        }
    }
}
