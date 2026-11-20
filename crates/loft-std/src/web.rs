//! The browser host supplies the same GPUDevice it uses for its own WebGPU work.
use crate::{
    edits::{tape_route, upload_words},
    error::RunnerError,
    promotion::Pending,
};
use loft_format::bundle::{decode, validate_wgsl};
use loft_ir::*;
use std::collections::BTreeMap;
use web_sys::{
    js_sys::{self, Array, Function, Object, Promise, Reflect, Uint8Array, futures::JsFuture},
    wasm_bindgen::{JsCast, JsValue},
};

#[derive(Clone)]
pub struct WebGpu {
    pub device: JsValue,
    pub queue: JsValue,
}

struct EditPipeline {
    kind: KindId,
    module: ModuleId,
    tape: TableId,
    layout: JsValue,
    preview: JsValue,
    active: JsValue,
    pending: Pending<JsValue>,
    job: Option<(u64, String, JsFuture)>,
}

pub struct WebBundle {
    gpu: WebGpu,
    routes: BTreeMap<PassId, EditPipeline>,
    dirty: Option<PassId>,
    pub decl: Bundle,
    pub modules: BTreeMap<ModuleId, JsValue>,
    pub buffers: BTreeMap<TableId, JsValue>,
}

impl WebGpu {
    /// Accept a browser GPUDevice handle (including a web-sys GpuDevice converted into JsValue).
    /// Keep the host's queue and avoid requesting or destroying a device.
    pub fn attach(device: JsValue) -> Result<Self, JsValue> {
        method(&device, "createShaderModule")?;
        method(&device, "createBuffer")?;
        let queue = Reflect::get(&device, &"queue".into())?;
        method(&queue, "writeBuffer")?;
        Ok(Self { device, queue })
    }

    /// Prepare bundle resources for the host's WebGPU command encoder.
    pub fn load_bundle(&self, bytes: &[u8]) -> Result<WebBundle, JsValue> {
        let decl = decode(bytes).map_err(|e| JsValue::from_str(&e.to_string()))?;
        validate_wgsl(&decl).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let mut modules = BTreeMap::new();
        for module in &decl.modules {
            let code = std::str::from_utf8(&module.bytes)
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            let desc = Object::new();
            Reflect::set(&desc, &"code".into(), &code.into())?;
            modules.insert(
                module.id,
                method(&self.device, "createShaderModule")?.call1(&self.device, &desc)?,
            );
        }
        let mut buffers = BTreeMap::new();
        for table in decl
            .tables
            .iter()
            .filter(|t| t.lifetime != TableLifetime::External)
        {
            let size = u64::from(table.stride_bytes) * u64::from(table.capacity);
            if table.stride_bytes == 0
                || !table.stride_bytes.is_multiple_of(4)
                || table.initial.len() as u64 > size
                || !table.initial.len().is_multiple_of(4)
                || size >= 1_u64 << 53
            {
                return Err("invalid .loftb buffer size or stride".into());
            }
            let usage = wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::INDEX;
            let desc = Object::new();
            Reflect::set(&desc, &"size".into(), &(size.max(4) as f64).into())?;
            Reflect::set(&desc, &"usage".into(), &usage.bits().into())?;
            let handle = method(&self.device, "createBuffer")?.call1(&self.device, &desc)?;
            if !table.initial.is_empty() {
                let args = Array::of3(
                    &handle,
                    &0.into(),
                    &Uint8Array::from(table.initial.as_ref()),
                );
                method(&self.queue, "writeBuffer")?.apply(&self.queue, &args)?;
            }
            buffers.insert(table.id, handle);
        }
        Ok(WebBundle {
            gpu: self.clone(),
            routes: BTreeMap::new(),
            dirty: None,
            decl,
            modules,
            buffers,
        })
    }
}

fn clock() -> Result<std::time::Duration, JsValue> {
    let performance = Reflect::get(&js_sys::global(), &"performance".into())?;
    let ms = method(&performance, "now")?
        .call0(&performance)?
        .as_f64()
        .ok_or("performance.now() must return milliseconds")?;
    Ok(std::time::Duration::from_secs_f64(ms / 1000.0))
}

fn invalid(err: RunnerError) -> JsValue {
    JsValue::from_str(&format!(
        "{err:?}: register an isolated kind tape and check its layout"
    ))
}

impl WebBundle {
    /// Register the host's already-built compute pipeline and its explicit GPUPipelineLayout.
    pub fn register_tier0(
        &mut self,
        kind: KindId,
        pass: PassId,
        layout: JsValue,
        preview: JsValue,
    ) -> Result<(), JsValue> {
        let (module, tape) = tape_route(&self.decl, kind, pass).map_err(invalid)?;
        if self.routes.contains_key(&pass)
            || self
                .routes
                .values()
                .any(|r| r.kind == kind || r.module == module || r.tape == tape)
        {
            return Err(invalid(RunnerError::AbiMismatch));
        }
        if !self.buffers.contains_key(&tape) || !layout.is_object() || !preview.is_object() {
            return Err(invalid(RunnerError::UnknownResource));
        }
        self.routes.insert(
            pass,
            EditPipeline {
                kind,
                module,
                tape,
                layout,
                active: preview.clone(),
                preview,
                pending: Pending::default(),
                job: None,
            },
        );
        Ok(())
    }

    pub fn hot_swap_tape(&mut self, kind: KindId, words: &[u32]) -> Result<(), JsValue> {
        let (&pass, route) = self
            .routes
            .iter_mut()
            .find(|(_, r)| r.kind == kind)
            .ok_or_else(|| invalid(RunnerError::UnknownResource))?;
        if self.dirty.is_some_and(|p| p != pass) {
            return Err("finish promoting the current kind before editing another".into());
        }
        let bytes = upload_words(words).map_err(invalid)?;
        let buffer = &self.buffers[&route.tape];
        let size = Reflect::get(buffer, &"size".into())?
            .as_f64()
            .ok_or("invalid GPUBuffer size")?;
        if bytes.len() as f64 > size {
            return Err("increase the tape table capacity before editing".into());
        }
        let args = Array::of3(buffer, &0.into(), &Uint8Array::from(bytes.as_slice()));
        method(&self.gpu.queue, "writeBuffer")?.apply(&self.gpu.queue, &args)?;
        route.pending.edited();
        route.active = route.preview.clone();
        self.dirty = Some(pass);
        Ok(())
    }

    pub fn hot_swap_module(&mut self, module: ModuleId, source: &str) -> Result<(), JsValue> {
        let pass = self.dirty.ok_or("upload the dirty kind's tape first")?;
        let route = self.routes.get_mut(&pass).expect("registered dirty pass");
        if route.module != module {
            return Err("request promotion for the dirty kind's module".into());
        }
        route.pending.request_at(clock()?, source.to_owned());
        Ok(())
    }

    /// Use this handle for setPipeline on each frame. Polling the browser promise
    /// never blocks the JS thread, and a rejected shader leaves the preview installed.
    pub fn edit_pipeline(&mut self, pass: PassId) -> Result<JsValue, JsValue> {
        use std::{
            future::Future,
            pin::Pin,
            task::{Context, Poll, Waker},
        };
        let route = self
            .routes
            .get_mut(&pass)
            .ok_or_else(|| invalid(RunnerError::UnknownResource))?;
        if self.dirty == Some(pass) {
            if let Some((_, _, future)) = &mut route.job {
                match Pin::new(future).poll(&mut Context::from_waker(Waker::noop())) {
                    Poll::Pending => return Ok(route.active.clone()),
                    Poll::Ready(result) => {
                        let (generation, key, _) = route.job.take().expect("completed promise");
                        if let Some(pipeline) = route.pending.complete(generation, key, result)? {
                            route.active = pipeline;
                            self.dirty = None;
                        }
                    }
                }
            }
            if let Some((generation, source)) = route.pending.ready_at(clock()?) {
                if let Some(pipeline) = route.pending.cached(generation, &source) {
                    route.active = pipeline;
                    self.dirty = None;
                } else {
                    let device = &self.gpu.device;
                    let shader = Object::new();
                    Reflect::set(&shader, &"code".into(), &source.as_str().into())?;
                    let module = method(device, "createShaderModule")?.call1(device, &shader)?;
                    let compute = Object::new();
                    Reflect::set(&compute, &"module".into(), &module)?;
                    Reflect::set(&compute, &"entryPoint".into(), &"loft_tier0_main".into())?;
                    let desc = Object::new();
                    Reflect::set(&desc, &"layout".into(), &route.layout)?;
                    Reflect::set(&desc, &"compute".into(), &compute)?;
                    let promise = method(device, "createComputePipelineAsync")?
                        .call1(device, &desc)?
                        .dyn_into::<Promise>()?;
                    route.job = Some((generation, source, promise.into()));
                }
            }
        }
        Ok(route.active.clone())
    }
}

fn method(target: &JsValue, name: &str) -> Result<Function, JsValue> {
    Reflect::get(target, &name.into())?
        .dyn_into()
        .map_err(|_| JsValue::from_str(name))
}
