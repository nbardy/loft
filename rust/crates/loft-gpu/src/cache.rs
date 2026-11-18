//! The Vulkan pipeline cache, moved from loft-loader (feat/pipecache: 0.08-1.28 ms activation from a retained cache).
//! Pass `raw()` to `Loft::load`. `open` is safe (plan §5.2): it holds an exclusive lock on the directory for the cache's
//! life, so this API is the only writer of the files the driver later trusts, and a game calls no `unsafe` function.
#![allow(unsafe_code)]
use loft_ir::Error;
use std::collections::hash_map::DefaultHasher;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// A Vulkan pipeline cache on one device, backed by a file in a caller-owned directory.
pub struct PipelineCache {
    raw: wgpu::PipelineCache,
    path: PathBuf,
    /// Held until drop: `dir/.lock`, exclusive.
    _lock: File,
}

fn io(path: &Path, e: io::Error) -> Error {
    Error::Environment(
        format!("pipeline cache {}: {e}. Fix: clear this file to rebuild the cache", path.display()).into(),
    )
}

impl PipelineCache {
    /// Opens the adapter and driver's cache and returns it with the bytes read; a missing file starts empty. The device
    /// needs `wgpu::Features::PIPELINE_CACHE`. The directory is locked until the cache drops; a second open, in this
    /// process or another, is refused as `Environment` naming the lock. Keep the directory apart from downloaded files.
    pub fn open(device: &wgpu::Device, dir: &Path) -> Result<(Self, usize), Error> {
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
        let lock_path = dir.join(".lock");
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock_path)
            .map_err(|e| io(&lock_path, e))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Err(Error::Environment(
                    format!(
                        "pipeline cache {} is open elsewhere. Fix: share one PipelineCache per device",
                        lock_path.display()
                    )
                    .into(),
                ));
            }
            Err(TryLockError::Error(e)) => return Err(io(&lock_path, e)),
        }
        let data = match fs::read(&path) {
            Ok(data) => Some(data),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(io(&path, e)),
        };
        // SAFETY: the bytes are what `save` wrote: the directory lock, held from before this read until drop, makes this
        // API its only writer, and wgpu checks the adapter and driver keys in the header before Vulkan sees the bytes.
        let raw = unsafe {
            device.create_pipeline_cache(&wgpu::PipelineCacheDescriptor {
                label: Some("loft pipeline cache"),
                data: data.as_deref(),
                fallback: false,
            })
        };
        Ok((Self { raw, path, _lock: lock }, data.as_ref().map_or(0, Vec::len)))
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
