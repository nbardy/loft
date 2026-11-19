//! A prebuilt compute pass can opt into tape edits and delayed pipeline promotion.
use crate::error::*;
#[cfg(not(target_arch = "wasm32"))]
use crate::promotion::{self, Pending};
use loft_abi::Tier0Opcode;
use loft_ir::*;
use std::collections::BTreeMap;

const MAX_WORDS: usize = 2048;

struct Route {
    pass: PassId,
    module: ModuleId,
    table: TableId,
    fallback: wgpu::ComputePipeline,
}

#[cfg(not(target_arch = "wasm32"))]
struct Job {
    generation: u64,
    source: String,
    done: std::sync::mpsc::Receiver<std::result::Result<wgpu::ComputePipeline, String>>,
}

#[derive(Default)]
pub struct Edits {
    routes: BTreeMap<KindId, Route>,
    dirty: Option<KindId>,
    #[cfg(not(target_arch = "wasm32"))]
    pending: Pending,
    #[cfg(not(target_arch = "wasm32"))]
    job: Option<Job>,
    #[cfg(not(target_arch = "wasm32"))]
    cache: std::collections::VecDeque<(PassId, String, wgpu::ComputePipeline)>,
}

pub trait Pipelines {
    fn pipeline(&self, pass: PassId) -> Result<&wgpu::ComputePipeline>;
    #[cfg(not(target_arch = "wasm32"))]
    fn pipeline_layout(&self, pass: PassId) -> Result<&wgpu::PipelineLayout>;
    fn replace_pipeline(&mut self, pass: PassId, pipeline: wgpu::ComputePipeline) -> Result<()>;
}

impl Edits {
    pub fn request(&mut self, module: ModuleId, _source: &str) -> Result<bool> {
        let Some(kind) = self.dirty else {
            return Ok(false);
        };
        if self.routes[&kind].module != module {
            if self.routes.values().any(|route| route.module == module) {
                return Err(RunnerError::Unsupported("a different kind is being edited"));
            }
            return Ok(false);
        }
        #[cfg(target_arch = "wasm32")]
        return Err(RunnerError::Unsupported(
            "asynchronous pipeline creation on this backend",
        ));
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.pending.request(_source.into());
            Ok(true)
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn update(&mut self, device: &wgpu::Device, compute: &mut impl Pipelines) -> Result<()> {
        use std::sync::mpsc::TryRecvError;
        let Some(kind) = self.dirty else {
            return Ok(());
        };
        let pass = self.routes[&kind].pass;
        if let Some(job) = &self.job {
            let result = match job.done.try_recv() {
                Ok(result) => result.map_err(RunnerError::InvalidShader),
                Err(TryRecvError::Empty) => return Ok(()),
                Err(TryRecvError::Disconnected) => Err(RunnerError::Unsupported("pipeline worker stopped")),
            };
            let job = self.job.take().unwrap();
            if self.pending.current(job.generation) {
                let pipeline = result?;
                compute.replace_pipeline(pass, pipeline.clone())?;
                self.cache.push_front((pass, job.source, pipeline));
                self.cache.truncate(8);
                self.dirty = None;
                return Ok(());
            }
        }
        let Some((generation, source)) = self.pending.ready() else {
            return Ok(());
        };
        if let Some(index) = self
            .cache
            .iter()
            .position(|(id, code, _)| *id == pass && *code == source)
        {
            let cached = self.cache.remove(index).unwrap();
            compute.replace_pipeline(pass, cached.2.clone())?;
            self.cache.push_front(cached);
            self.dirty = None;
            return Ok(());
        }
        let layout = compute.pipeline_layout(pass)?.clone();
        let done = promotion::start(device.clone(), layout, source.clone())
            .map_err(|e| RunnerError::InvalidShader(e.to_string()))?;
        self.job = Some(Job {
            generation,
            source,
            done,
        });
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn update(&mut self, _device: &wgpu::Device, _compute: &mut impl Pipelines) -> Result<()> {
        Ok(())
    }
}

impl Edits {
    pub fn register(
        &mut self,
        bundle: &Bundle,
        pipelines: &impl Pipelines,
        kind: KindId,
        pass: PassId,
    ) -> Result<()> {
        let row = bundle.rows.get(&kind).ok_or(RunnerError::UnknownResource)?;
        let decl = bundle
            .passes
            .iter()
            .find(|p| p.id == pass)
            .ok_or(RunnerError::UnknownResource)?;
        let PassRun::Compute { module, entry, .. } = &decl.run else {
            return Err(RunnerError::AbiMismatch);
        };
        if entry != "loft_tier0_main"
            || self.routes.contains_key(&kind)
            || self
                .routes
                .values()
                .any(|route| route.pass == pass || route.module == *module)
            || !decl
                .bindings
                .iter()
                .any(|b| b.group == 1 && b.slot == 2 && b.resource == *row)
        {
            return Err(RunnerError::AbiMismatch);
        }
        let table = decl
            .bindings
            .iter()
            .find(|b| b.group == 1 && b.slot == 0)
            .ok_or(RunnerError::AbiMismatch)?
            .resource;
        if self.routes.values().any(|route| route.table == table) || bundle.rows.values().any(|&row| row == table)
        {
            return Err(RunnerError::AbiMismatch);
        }
        let fallback = pipelines.pipeline(pass)?.clone();
        self.routes.insert(
            kind,
            Route {
                pass,
                module: *module,
                table,
                fallback,
            },
        );
        Ok(())
    }

    pub fn edit<'a>(
        &mut self,
        kind: KindId,
        tape: &[u32],
        pipelines: &mut impl Pipelines,
        buffer: impl FnOnce(TableId) -> Result<&'a wgpu::Buffer>,
        queue: &wgpu::Queue,
    ) -> Result<()> {
        let bytes = upload_words(tape)?;
        let route = self.routes.get(&kind).ok_or(RunnerError::UnknownResource)?;
        if self.dirty.is_some_and(|dirty| dirty != kind) {
            return Err(RunnerError::Unsupported("a different kind is being edited"));
        }
        let buffer = buffer(route.table)?;
        if buffer.size() < bytes.len() as u64 || !buffer.usage().contains(wgpu::BufferUsages::COPY_DST) {
            return Err(RunnerError::AbiMismatch);
        }
        pipelines.replace_pipeline(route.pass, route.fallback.clone())?;
        queue.write_buffer(buffer, 0, &bytes);
        #[cfg(not(target_arch = "wasm32"))]
        self.pending.edited();
        self.dirty = Some(kind);
        Ok(())
    }
}

fn upload_words(words: &[u32]) -> Result<Vec<u8>> {
    if words.is_empty() || words.len() > MAX_WORDS {
        return Err(RunnerError::AbiMismatch);
    }
    let mut loops = Vec::new();
    for (pc, &word) in words.iter().enumerate() {
        match Tier0Opcode::unpack(word) {
            Some((Tier0Opcode::LoopBegin, _, _, _, end))
                if loops.len() < 32 && (pc + 1..words.len()).contains(&usize::from(end)) =>
            {
                loops.push((pc, usize::from(end)))
            }
            Some((Tier0Opcode::LoopEnd, _, _, _, start)) if loops.pop() == Some((usize::from(start), pc)) => {}
            Some((Tier0Opcode::Ret, ..)) if loops.is_empty() && pc + 1 == words.len() => {
                return Ok(std::iter::once(words.len() as u32)
                    .chain(words.iter().copied())
                    .flat_map(u32::to_le_bytes)
                    .collect());
            }
            Some((Tier0Opcode::LoopBegin | Tier0Opcode::LoopEnd | Tier0Opcode::Ret, ..)) | None => {
                return Err(RunnerError::AbiMismatch);
            }
            Some(_) => {}
        }
    }
    Err(RunnerError::AbiMismatch)
}

#[cfg(test)]
mod tests {
    use super::{RunnerError, upload_words};
    use loft_abi::Tier0Opcode;
    #[test]
    fn l23a_upload_validates_before_replacing_tape() {
        let ret = Tier0Opcode::Ret.pack(0, 0, 0, 0).unwrap();
        assert_eq!(
            upload_words(&[ret]).unwrap(),
            [1u32.to_le_bytes(), ret.to_le_bytes()].concat()
        );
        for invalid in [vec![], vec![ret, ret], vec![56 << 26, ret], vec![53 << 26, ret]] {
            assert_eq!(upload_words(&invalid), Err(RunnerError::AbiMismatch));
        }
    }
}
