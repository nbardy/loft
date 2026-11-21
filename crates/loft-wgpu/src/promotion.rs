pub use loft_std::promotion::*;
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
    let (ir, _) = loft_format::wgsl::validate(source, &mut crate::bindings::validator(device.features()))
        .map_err(|e| e.emit_to_string(source))?;
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(ir)),
    });
    let pipeline = crate::command::compute_pipeline(device, layout, &module, "loft_tier0_main", None);
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
