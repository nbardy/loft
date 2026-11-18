//! The program cache (lean Rust plan §7): a closed program's result, keyed by what determines it. Recording is
//! deterministic (§3.3), so the job bytes are canonical, and key = sha256(job ‖ options ‖ core commit) names one result:
//! a solve's curves and verdicts, a check's verdicts, or a bundle. It replaces the solve cache and `next.md`'s
//! "exact structural cache"; a hit runs nothing (heron's solve: 0 rounds).
//!
//! One file per key, written to a temporary name and renamed, so a reader sees a whole result or none. Errors are not
//! cached: a refusal is cheap to reach again and carries the author's site.
use loft_ir::error::Error;
use sha2::{Digest, Sha256};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub type Key = [u8; 32];

/// The key of a program: its recorded job, its options (budgets, precision) and the engine commit that runs it.
pub fn key(job: &[u8], options: &[u8], commit: &str) -> Key {
    let mut h = Sha256::new();
    for part in [job, options, commit.as_bytes()] {
        h.update((part.len() as u64).to_le_bytes());
        h.update(part);
    }
    h.finalize().into()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Cached,
    Ran,
}

/// The result for `key` from `dir`, or `program`'s, stored for the next caller.
pub fn through(dir: &Path, key: &Key, program: impl FnOnce() -> Result<Vec<u8>, Error>) -> Result<(Vec<u8>, Hit), Error> {
    let file: PathBuf = dir.join(key.iter().map(|b| format!("{b:02x}")).collect::<String>());
    let env = |what: &str, e: std::io::Error| Error::Environment(format!("program cache {what} {}: {e}; check the directory or remove the entry", file.display()).into());
    match std::fs::read(&file) {
        Ok(bytes) => return Ok((bytes, Hit::Cached)),
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(env("read", e)),
    }
    let bytes = program()?;
    std::fs::create_dir_all(dir).map_err(|e| env("create", e))?;
    let tmp = file.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::write(&tmp, &bytes).map_err(|e| env("write", e))?;
    std::fs::rename(&tmp, &file).map_err(|e| env("rename", e))?;
    Ok((bytes, Hit::Ran))
}
