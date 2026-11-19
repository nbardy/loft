//! The browser host supplies the same GPUDevice it uses for its own WebGPU work.
use loft_format::bundle::{decode, validate_wgsl};
use loft_ir::*;
use std::collections::BTreeMap;
use web_sys::{
    js_sys::{Array, Function, Object, Reflect, Uint8Array},
    wasm_bindgen::{JsCast, JsValue},
};

pub struct WebGpu {
    pub device: JsValue,
    pub queue: JsValue,
}

pub struct WebBundle {
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
            decl,
            modules,
            buffers,
        })
    }
}

fn method(target: &JsValue, name: &str) -> Result<Function, JsValue> {
    Reflect::get(target, &name.into())?
        .dyn_into()
        .map_err(|_| JsValue::from_str(name))
}
