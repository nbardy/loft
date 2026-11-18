use crate::{scoped, Artefact, Error, WarmedArtefact};
use std::collections::hash_map::DefaultHasher;
use std::fs::{self, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// A Vulkan pipeline cache on one device, backed by a file in a caller-owned directory.
pub struct PipelineCache {
    device: wgpu::Device,
    raw: wgpu::PipelineCache,
    path: PathBuf,
}

impl PipelineCache {
    /// Opens the adapter and driver's cache, returning the cache and the number of bytes read.
    /// A missing file starts an empty cache. Enable `wgpu::Features::PIPELINE_CACHE` at device creation.
    ///
    /// # Safety
    /// The directory must be trusted. Any existing cache file must contain bytes previously saved by this API
    /// (from wgpu's `PipelineCache::get_data`). Keep this directory separate from downloaded artefacts.
    pub unsafe fn open(device: &wgpu::Device, dir: &Path) -> Result<(Self, usize), Error> {
        let info = device.adapter_info();
        let key = wgpu::util::pipeline_cache_key(&info)
            .filter(|_| device.features().contains(wgpu::Features::PIPELINE_CACHE))
            .ok_or(Error::PipelineCacheUnsupported(info.backend))?;
        // The wgpu filename keys the adapter. Driver strings separate files across driver changes;
        // wgpu also checks its binary driver version key in the bytes before handing them to Vulkan.
        let mut driver = DefaultHasher::new();
        (&info.driver, &info.driver_info).hash(&mut driver);
        let path = dir.join(format!("{key}_{:016x}.bin", driver.finish()));
        fs::create_dir_all(dir).map_err(|source| cache_io(dir, source))?;
        let data = match fs::read(&path) {
            Ok(data) => Some(data),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(cache_io(&path, e)),
        };
        let raw = scoped(
            device,
            || {
                format!(
                    "pipeline cache {}: clear this file to rebuild the cache",
                    path.display()
                )
            },
            || {
                // SAFETY: the caller vouches for the provenance of the cache bytes.
                unsafe {
                    device.create_pipeline_cache(&wgpu::PipelineCacheDescriptor {
                        label: Some("Loft pipeline cache"),
                        data: data.as_deref(),
                        fallback: false,
                    })
                }
            },
        )?;
        Ok((
            Self {
                device: device.clone(),
                raw,
                path,
            },
            data.as_ref().map_or(0, Vec::len),
        ))
    }

    /// Compiles the artefact on this cache's device, adding any new pipelines to the cache.
    pub fn warm(&self, artefact: Artefact) -> Result<WarmedArtefact, Error> {
        artefact.warm_on(&self.device, Some(&self.raw))
    }

    /// Atomically saves a snapshot of this device's cache and returns its size in bytes.
    /// Reuse one cache across the artefacts to retain them in the same snapshot.
    pub fn save(&self) -> Result<usize, Error> {
        let data = self
            .raw
            .get_data()
            .ok_or_else(|| Error::PipelineCacheNoData(self.path.clone()))?;
        write_atomically(&self.path, &data)?;
        Ok(data.len())
    }
}

fn cache_io(path: &Path, source: io::Error) -> Error {
    Error::PipelineCacheIo {
        path: path.into(),
        source,
    }
}

fn write_atomically(path: &Path, data: &[u8]) -> Result<(), Error> {
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let (temp, mut file) = loop {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let temp = path.with_extension(format!("{}.{serial}.tmp", std::process::id()));
        match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(file) => break (temp, file),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(cache_io(&temp, e)),
        }
    };
    let written = file.write_all(data).and_then(|_| file.sync_all());
    drop(file);
    // A reader racing the save sees the previous complete snapshot or the new complete one.
    let result = written.and_then(|_| fs::rename(&temp, path));
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result.map_err(|source| cache_io(path, source))
}
