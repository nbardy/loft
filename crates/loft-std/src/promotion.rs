//! Native asynchronous pipeline preparation on a host worker.
use std::time::{Duration, Instant};

const QUIET: Duration = Duration::from_millis(400);

#[derive(Default)]
pub struct Pending {
    generation: u64,
    request: Option<(Instant, String)>,
}

impl Pending {
    pub fn request(&mut self, shader: String) { self.request_at(Instant::now(), shader); }

    pub fn ready(&mut self) -> Option<(u64, String)> { self.ready_at(Instant::now()) }

    pub fn edited(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.request = None;
    }

    fn request_at(&mut self, now: Instant, shader: String) {
        self.generation = self.generation.wrapping_add(1);
        self.request = Some((now + QUIET, shader));
    }

    #[cfg(any(test, not(target_arch = "wasm32")))]
    fn ready_at(&mut self, now: Instant) -> Option<(u64, String)> {
        let (deadline, _) = self.request.as_ref()?;
        (*deadline <= now).then(|| (self.generation, self.request.take().unwrap().1))
    }

    #[cfg(any(test, not(target_arch = "wasm32")))]
    pub fn current(&self, generation: u64) -> bool {
        generation == self.generation
    }
}


pub fn start(
    device: wgpu::Device, layout: wgpu::PipelineLayout, source: String,
) -> std::io::Result<std::sync::mpsc::Receiver<Result<wgpu::ComputePipeline, String>>> {
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::Builder::new().name("loft-pipeline".into()).spawn(move || {
        let _ = send.send(compile(&device, &layout, &source));
    })?;
    Ok(receive)
}

fn compile(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    source: &str,
) -> Result<wgpu::ComputePipeline, String> {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    let ir = naga::front::wgsl::parse_str(source)
        .map_err(|e| e.emit_to_string(source))?;
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
    fn l23a_debounces_and_discards_stale_pipeline() {
        let t = Instant::now();
        let mut p = Pending::default();
        p.request_at(t, "first".into());
        assert!(p.ready_at(t + Duration::from_millis(399)).is_none());
        let (first, _) = p.ready_at(t + QUIET).unwrap();
        p.edited();
        assert!(!p.current(first));
        p.request_at(t + QUIET, "second".into());
        p.request_at(t + QUIET + Duration::from_millis(30), "latest".into());
        assert!(p.ready_at(t + 2 * QUIET).is_none());
        let (latest, code) = p.ready_at(t + 2 * QUIET + Duration::from_millis(30)).unwrap();
        assert!(p.current(latest));
        assert_eq!(code, "latest");
        assert!(p.ready_at(t + 3 * QUIET).is_none());
    }

}
