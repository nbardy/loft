//! A prebuilt compute pass can opt into tape edits and delayed pipeline promotion.
use super::*;
use loft_abi::Tier0Opcode;
use std::time::{Duration, Instant};

const QUIET: Duration = Duration::from_millis(400);
const MAX_WORDS: usize = 2048;

#[derive(Default)]
struct Pending {
    generation: u64,
    request: Option<(Instant, String)>,
}

impl Pending {
    fn edited(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.request = None;
    }

    fn request(&mut self, now: Instant, shader: String) {
        self.generation = self.generation.wrapping_add(1);
        self.request = Some((now + QUIET, shader));
    }

    #[cfg(any(test, not(target_arch = "wasm32")))]
    fn ready(&mut self, now: Instant) -> Option<(u64, String)> {
        let (deadline, _) = self.request.as_ref()?;
        (*deadline <= now).then(|| (self.generation, self.request.take().unwrap().1))
    }

    #[cfg(any(test, not(target_arch = "wasm32")))]
    fn current(&self, generation: u64) -> bool {
        generation == self.generation
    }
}

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
    done: std::sync::mpsc::Receiver<Result<wgpu::ComputePipeline>>,
}

#[derive(Default)]
pub(super) struct Edits {
    routes: BTreeMap<KindId, Route>,
    dirty: Option<KindId>,
    pending: Pending,
    #[cfg(not(target_arch = "wasm32"))]
    job: Option<Job>,
    #[cfg(not(target_arch = "wasm32"))]
    cache: std::collections::VecDeque<(PassId, String, wgpu::ComputePipeline)>,
}

impl Edits {
    pub(super) fn request(&mut self, module: ModuleId, source: &str) -> Result<bool> {
        let Some(kind) = self.dirty else {
            return Ok(false);
        };
        if self.routes[&kind].module != module {
            if self.routes.values().any(|route| route.module == module) {
                return Err(RunnerError::Unsupported("a different kind is being edited"));
            }
            return Ok(false);
        }
        if cfg!(target_arch = "wasm32") {
            return Err(RunnerError::Unsupported(
                "asynchronous pipeline creation on this backend",
            ));
        }
        self.pending.request(Instant::now(), source.into());
        Ok(true)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn update(
        &mut self,
        device: &wgpu::Device,
        compute: &mut encode::ComputeCache,
    ) -> Result<()> {
        use std::sync::mpsc::{self, TryRecvError};
        let Some(kind) = self.dirty else {
            return Ok(());
        };
        let pass = self.routes[&kind].pass;
        if let Some(job) = &self.job {
            let result = match job.done.try_recv() {
                Ok(result) => result,
                Err(TryRecvError::Empty) => return Ok(()),
                Err(TryRecvError::Disconnected) => {
                    Err(RunnerError::Unsupported("pipeline worker stopped"))
                }
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
        let Some((generation, source)) = self.pending.ready(Instant::now()) else {
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
        let (sender, done) = mpsc::channel();
        let device = device.clone();
        let layout = compute.pipeline_layout(pass)?.clone();
        let shader = source.clone();
        std::thread::Builder::new()
            .name("loft-pipeline".into())
            .spawn(move || {
                let _ = sender.send(compile(&device, &layout, &shader));
            })
            .map_err(|e| RunnerError::InvalidShader(e.to_string()))?;
        self.job = Some(Job {
            generation,
            source,
            done,
        });
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn update(
        &mut self,
        _device: &wgpu::Device,
        _compute: &mut encode::ComputeCache,
    ) -> Result<()> {
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn compile(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    source: &str,
) -> Result<wgpu::ComputePipeline> {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    let ir = naga::front::wgsl::parse_str(source)
        .map_err(|e| RunnerError::InvalidShader(e.emit_to_string(source)))?;
    wgpu_naga_bridge::create_validator(
        device.features(),
        wgpu::DownlevelFlags::all(),
        naga::valid::ValidationFlags::all(),
    )
    .validate(&ir)
    .map_err(|e| RunnerError::InvalidShader(e.emit_to_string(source)))?;
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(ir)),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: Some(layout),
        module: &module,
        entry_point: Some("loft_tier0_main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let mut error = std::pin::pin!(scope.pop());
    loop {
        match error.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(Some(err)) => return Err(RunnerError::InvalidShader(err.to_string())),
            Poll::Ready(None) => return Ok(pipeline),
            Poll::Pending => {
                device
                    .poll(wgpu::PollType::Poll)
                    .map_err(|e| RunnerError::InvalidShader(e.to_string()))?;
                std::thread::yield_now();
            }
        }
    }
}

impl Runner {
    /// Register the bundle's precompiled Tier 0 pass for one kind.
    pub fn register_tier0(&mut self, id: InstanceId, kind: KindId, pass: PassId) -> Result<()> {
        let inst = self.instance(id)?;
        let row = inst
            .decl
            .rows
            .get(&kind)
            .ok_or(RunnerError::UnknownResource)?;
        let decl = inst
            .decl
            .passes
            .iter()
            .find(|p| p.id == pass)
            .ok_or(RunnerError::UnknownResource)?;
        let PassRun::Compute { module, entry, .. } = &decl.run else {
            return Err(RunnerError::AbiMismatch);
        };
        if entry != "loft_tier0_main"
            || inst.edits.routes.contains_key(&kind)
            || inst
                .edits
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
        if inst.edits.routes.values().any(|route| route.table == table)
            || inst.decl.rows.values().any(|&row| row == table)
        {
            return Err(RunnerError::AbiMismatch);
        }
        let fallback = inst.compute.pipeline(pass)?.clone();
        inst.edits.routes.insert(
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

    pub fn hot_swap_tape(&mut self, id: InstanceId, kind: KindId, tape: &[u32]) -> Result<()> {
        let bytes = upload_words(tape)?;
        let queue = self.queue.clone();
        let inst = self.instance(id)?;
        let route = inst
            .edits
            .routes
            .get(&kind)
            .ok_or(RunnerError::UnknownResource)?;
        if inst.edits.dirty.is_some_and(|dirty| dirty != kind) {
            return Err(RunnerError::Unsupported("a different kind is being edited"));
        }
        let buffer = inst.tables[&route.table].bytes()?;
        if buffer.size() < bytes.len() as u64
            || !buffer.usage().contains(wgpu::BufferUsages::COPY_DST)
        {
            return Err(RunnerError::AbiMismatch);
        }
        inst.compute
            .replace_pipeline(route.pass, route.fallback.clone())?;
        queue.write_buffer(buffer, 0, &bytes);
        inst.edits.pending.edited();
        inst.edits.dirty = Some(kind);
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
            Some((Tier0Opcode::LoopEnd, _, _, _, start))
                if loops.pop() == Some((usize::from(start), pc)) => {}
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
    use super::*;
    #[test]
    fn l23a_debounces_and_discards_stale_pipeline() {
        let t = Instant::now();
        let mut p = Pending::default();
        p.request(t, "first".into());
        assert!(p.ready(t + Duration::from_millis(399)).is_none());
        let (first, _) = p.ready(t + QUIET).unwrap();
        p.edited();
        assert!(!p.current(first));
        p.request(t + QUIET, "second".into());
        p.request(t + QUIET + Duration::from_millis(30), "latest".into());
        assert!(p.ready(t + 2 * QUIET).is_none());
        let (latest, code) = p.ready(t + 2 * QUIET + Duration::from_millis(30)).unwrap();
        assert!(p.current(latest));
        assert_eq!(code, "latest");
        assert!(p.ready(t + 3 * QUIET).is_none());
    }

    #[test]
    fn l23a_upload_validates_before_replacing_tape() {
        let ret = Tier0Opcode::Ret.pack(0, 0, 0, 0).unwrap();
        assert_eq!(
            upload_words(&[ret]).unwrap(),
            [1u32.to_le_bytes(), ret.to_le_bytes()].concat()
        );
        for invalid in [
            vec![],
            vec![ret, ret],
            vec![56 << 26, ret],
            vec![53 << 26, ret],
        ] {
            assert_eq!(upload_words(&invalid), Err(RunnerError::AbiMismatch));
        }
    }
}
