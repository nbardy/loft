//! Quiet-period policy shared by native workers and browser promise adapters.
use std::{collections::VecDeque, time::Duration};
#[cfg(not(target_arch = "wasm32"))]
use std::{sync::OnceLock, time::Instant};

const QUIET: Duration = Duration::from_millis(400);
const CACHE_SIZE: usize = 8;

pub struct Pending<P = ()> {
    generation: u64,
    request: Option<(Duration, String)>,
    cache: VecDeque<(String, P)>,
    stale: u64,
}

impl<P> Default for Pending<P> {
    fn default() -> Self {
        Self {
            generation: 0,
            request: None,
            cache: VecDeque::new(),
            stale: 0,
        }
    }
}

impl<P> Pending<P> {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn request(&mut self, shader: String) {
        self.request_at(host_time(), shader);
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn ready(&mut self) -> Option<(u64, String)> {
        self.ready_at(host_time())
    }

    pub fn edited(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.request = None;
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn request_at(&mut self, now: Duration, shader: String) {
        self.edited();
        self.request = Some((now + QUIET, shader));
    }

    pub fn ready_at(&mut self, now: Duration) -> Option<(u64, String)> {
        let (deadline, _) = self.request.as_ref()?;
        (*deadline <= now).then(|| {
            (
                self.generation,
                self.request.take().expect("ready request").1,
            )
        })
    }

    pub fn current(&self, generation: u64) -> bool {
        generation == self.generation
    }

    pub fn cached(&mut self, generation: u64, key: &str) -> Option<P>
    where
        P: Clone,
    {
        if !self.current(generation) {
            return None;
        }
        let i = self.cache.iter().position(|(source, _)| source == key)?;
        let pair = self.cache.remove(i).expect("cache index");
        let pipeline = pair.1.clone();
        self.cache.push_front(pair);
        Some(pipeline)
    }

    pub fn complete<E>(
        &mut self,
        generation: u64,
        key: String,
        result: Result<P, E>,
    ) -> Result<Option<P>, E>
    where
        P: Clone,
    {
        if !self.current(generation) {
            self.stale += 1;
            return Ok(None);
        }
        let pipeline = result?;
        self.cache.retain(|(source, _)| *source != key);
        self.cache.push_front((key, pipeline.clone()));
        self.cache.truncate(CACHE_SIZE);
        Ok(Some(pipeline))
    }

    pub fn stale(&self) -> u64 {
        self.stale
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn host_time() -> Duration {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn worker<T: Send + 'static>(
    prepare: impl FnOnce() -> T + Send + 'static,
) -> std::io::Result<std::sync::mpsc::Receiver<T>> {
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("loft-pipeline".into())
        .spawn(move || {
            let _ = send.send(prepare());
        })?;
    Ok(receive)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn start(
    device: wgpu::Device,
    layout: wgpu::PipelineLayout,
    source: String,
) -> std::io::Result<std::sync::mpsc::Receiver<Result<wgpu::ComputePipeline, String>>> {
    worker(move || compile(&device, &layout, &source))
}

#[cfg(not(target_arch = "wasm32"))]
fn compile(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    source: &str,
) -> Result<wgpu::ComputePipeline, String> {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    let ir = naga::front::wgsl::parse_str(source).map_err(|e| e.emit_to_string(source))?;
    wgpu_naga_bridge::create_validator(
        device.features(),
        wgpu::DownlevelFlags::all(),
        naga::valid::ValidationFlags::all(),
    )
    .validate(&ir)
    .map_err(|e| e.emit_to_string(source))?;
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
            Poll::Ready(Some(err)) => return Err(err.to_string()),
            Poll::Ready(None) => return Ok(pipeline),
            Poll::Pending => {
                device
                    .poll(wgpu::PollType::Poll)
                    .map_err(|e| e.to_string())?;
                std::thread::yield_now();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn revisiting_a_shader_retains_it_across_lru_eviction() {
        let mut pending = Pending::default();
        for id in 0..8 {
            pending.complete(0, id.to_string(), Ok::<_, ()>(id)).unwrap();
        }
        assert_eq!(pending.cached(0, "0"), Some(0));
        pending.complete(0, "8".into(), Ok::<_, ()>(8)).unwrap();
        assert_eq!(pending.cached(0, "1"), None);
        assert_eq!(pending.cached(0, "0"), Some(0));
    }

    #[test]
    fn l23a_debounces_and_discards_stale_pipeline() {
        let t = Duration::ZERO;
        let mut p = Pending::<u32>::default();
        p.request_at(t, "first".into());
        assert!(p.ready_at(t + Duration::from_millis(399)).is_none());
        let (first, _) = p.ready_at(t + QUIET).unwrap();
        p.edited();
        assert!(!p.current(first));
        assert_eq!(p.complete(first, "first".into(), Ok::<_, ()>(17)), Ok(None));
        assert_eq!(p.stale(), 1);
        p.request_at(t + QUIET, "second".into());
        p.request_at(t + QUIET + Duration::from_millis(30), "latest".into());
        assert!(p.ready_at(t + 2 * QUIET).is_none());
        let (latest, code) = p
            .ready_at(t + 2 * QUIET + Duration::from_millis(30))
            .unwrap();
        assert!(p.current(latest));
        assert_eq!(code, "latest");
        assert_eq!(p.complete(latest, code, Ok::<_, ()>(19)), Ok(Some(19)));
        assert_eq!(p.cached(latest, "latest"), Some(19));
        assert!(p.ready_at(t + 3 * QUIET).is_none());
    }
}
