//! The Vulkan pipeline cache, moved from loft-loader (feat/pipecache: 0.08-1.28 ms activation from a retained cache).
//! Pass `raw()` to `Loft::load`. Lane 4.1 makes `open` safe with an exclusive lock on the directory.
#![allow(unsafe_code)]
use loft_ir::Error;
use std::collections::hash_map::DefaultHasher;
use std::fs::{self, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// A Vulkan pipeline cache on one device, backed by a file in a caller-owned directory.
pub struct PipelineCache {
    raw: wgpu::PipelineCache,
    path: PathBuf,
}

fn io(path: &Path, e: io::Error) -> Error {
    Error::Environment(
        format!("pipeline cache {}: {e}. Fix: clear this file to rebuild the cache", path.display()).into(),
    )
}

impl PipelineCache {
    /// Opens the adapter and driver's cache and returns it with the bytes read; a missing file starts empty. The device
    /// needs `wgpu::Features::PIPELINE_CACHE`.
    ///
    /// # Safety
    /// The directory must be trusted: any cache file in it holds bytes this API saved (wgpu's `get_data`). Keep it apart
    /// from downloaded bundles.
    pub unsafe fn open(device: &wgpu::Device, dir: &Path) -> Result<(Self, usize), Error> {
        let info = device.adapter_info();
        let key = wgpu::util::pipeline_cache_key(&info)
            .filter(|_| device.features().contains(wgpu::Features::PIPELINE_CACHE))
            .ok_or_else(|| {
                Error::Environment(
                    format!("{:?} has no pipeline cache. Fix: use a Vulkan adapter with PIPELINE_CACHE", info.backend)
                        .into(),
                )
            })?;
        // The wgpu file name keys the adapter; driver strings separate files across driver changes, and wgpu checks its
        // binary driver key in the bytes before Vulkan sees them.
        let mut driver = DefaultHasher::new();
        (&info.driver, &info.driver_info).hash(&mut driver);
        let path = dir.join(format!("{key}_{:016x}.bin", driver.finish()));
        fs::create_dir_all(dir).map_err(|e| io(dir, e))?;
        let data = match fs::read(&path) {
            Ok(data) => Some(data),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(io(&path, e)),
        };
        // SAFETY: the caller vouches for the provenance of the cache bytes (this function's contract).
        let raw = unsafe {
            device.create_pipeline_cache(&wgpu::PipelineCacheDescriptor {
                label: Some("loft pipeline cache"),
                data: data.as_deref(),
                fallback: false,
            })
        };
        Ok((Self { raw, path }, data.as_ref().map_or(0, Vec::len)))
    }

    pub fn raw(&self) -> &wgpu::PipelineCache {
        &self.raw
    }

    /// Atomically saves a snapshot and returns its size. Share one cache across a device's loads, then save once.
    pub fn save(&self) -> Result<usize, Error> {
        let data = self.raw.get_data().ok_or_else(|| io(&self.path, io::ErrorKind::Unsupported.into()))?;
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let (temp, mut file) = loop {
            let temp = self.path.with_extension(format!(
                "{}.{}.tmp",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            match OpenOptions::new().write(true).create_new(true).open(&temp) {
                Ok(file) => break (temp, file),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(io(&temp, e)),
            }
        };
        let written = file.write_all(&data).and_then(|_| file.sync_all());
        drop(file);
        // A reader racing the save sees the previous complete snapshot or the new one.
        let result = written.and_then(|_| fs::rename(&temp, &self.path));
        if result.is_err() {
            let _ = fs::remove_file(temp);
        }
        result.map_err(|e| io(&self.path, e))?;
        Ok(data.len())
    }
}
