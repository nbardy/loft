//! The browser host supplies the same GPUDevice it uses for its own WebGPU work.
use loft_format::bundle::{decode, validate_wgsl};
use loft_ir::*;
use std::collections::BTreeMap;
use web_sys::{
    js_sys::{Array, Function, Map, Object, Reflect, Uint8Array},
    wasm_bindgen::{JsCast, JsValue, prelude::wasm_bindgen},
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
        self.prepare(validated(bytes)?, |code| Ok(code.into()))
    }

    fn prepare(
        &self,
        decl: Bundle,
        source: impl Fn(&str) -> Result<JsValue, JsValue>,
    ) -> Result<WebBundle, JsValue> {
        let mut modules = BTreeMap::new();
        for module in &decl.modules {
            let code = std::str::from_utf8(&module.bytes)
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            let desc = Object::new();
            Reflect::set(&desc, &"code".into(), &source(code)?)?;
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

fn validated(bytes: &[u8]) -> Result<Bundle, JsValue> {
    let decl = decode(bytes).map_err(|e| JsValue::from_str(&e.to_string()))?;
    validate_wgsl(&decl).map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(decl)
}

/// Load the raster/compute subset into the caller's browser device.
/// The caller can adapt WGSL to its presentation and parameter conventions before GPU compilation.
#[wasm_bindgen(wasm_bindgen = web_sys::wasm_bindgen, js_name = loadBundle)]
pub fn load_browser(device: JsValue, bytes: &[u8], source: &Function) -> Result<Object, JsValue> {
    let gpu = WebGpu::attach(device)?;
    let decl = validated(bytes)?;
    let params = crate::params::Params::new(&decl.params, 65536).map_err(js_error)?;
    let bundle = gpu.prepare(decl, |code| {
        let slot = (params.bytes().len() / 16) as u32;
        source.call2(&JsValue::UNDEFINED, &code.into(), &slot.into())
    })?;
    let decl = &bundle.decl;
    let resources = crate::bundle::Resources::new(decl)
        .map_err(crate::error::RunnerError::from)
        .map_err(js_error)?;
    let frame = decl
        .resources
        .frame
        .as_ref()
        .ok_or("bundle has no frame templates")?;
    let table = |id| {
        decl.tables
            .iter()
            .find(|t| t.id == id)
            .ok_or(JsValue::from_str("unknown table"))
    };
    let passes = decl
        .passes
        .iter()
        .map(|pass| {
            let id = frame
                .variants
                .iter()
                .find(|(id, _)| *id == pass.id)
                .map_or(frame.main, |(_, id)| *id);
            let slots: Array = pass
                .bindings
                .iter()
                .map(|b| {
                    array([
                        b.group.into(),
                        b.slot.into(),
                        b.resource.0.into(),
                        format!("{:?}", b.access).into(),
                    ])
                })
                .collect();
            let work = match &pass.run {
                PassRun::Compute {
                    module,
                    entry,
                    dispatch,
                } => {
                    let dispatch = match *dispatch {
                        DispatchExtent::Fixed([x, y, z]) => {
                            array(["direct".into(), x.into(), y.into(), z.into()])
                        }
                        DispatchExtent::FromRowCount {
                            table: id,
                            workgroup_x,
                        } => {
                            let t = table(id)?;
                            let rows = (t.initial.len() / t.stride_bytes as usize) as u32;
                            let count = rows.div_ceil(
                                std::num::NonZeroU32::new(workgroup_x)
                                    .ok_or("zero workgroup width")?
                                    .get(),
                            );
                            array(["direct".into(), count.into(), 1.into(), 1.into()])
                        }
                        DispatchExtent::Indirect { buf, offset_bytes } => {
                            array(["indirect".into(), buf.0.into(), offset_bytes.into()])
                        }
                    };
                    array([
                        "compute".into(),
                        module.0.into(),
                        entry.as_str().into(),
                        dispatch,
                    ])
                }
                PassRun::RenderBundle {
                    draws,
                    color_targets,
                    depth,
                    msaa: 1,
                } => {
                    let [draw] = draws.as_ref() else {
                        return Err("browser pass requires one indirect draw".into());
                    };
                    if draw.index.is_some() || draw.vertex.is_some() {
                        return Err("browser pass requires vertex pulling".into());
                    }
                    let colors: Array = color_targets
                        .iter()
                        .map(|c| JsValue::from(c.table.0))
                        .collect();
                    let depth = depth
                        .as_ref()
                        .map_or(JsValue::NULL, |d| array([d.table.0.into(), d.write.into()]));
                    let cull = decl.resources.cull_front.contains(&pass.id);
                    array([
                        "render".into(),
                        draw.module.0.into(),
                        draw.vertex_entry.as_str().into(),
                        draw.fragment_entry.as_str().into(),
                        draw.args.0.into(),
                        draw.offset_bytes.into(),
                        colors.into(),
                        depth,
                        cull.into(),
                    ])
                }
                PassRun::RenderBundle { .. }
                | PassRun::ComputeFusedTicks { .. }
                | PassRun::ComputeNonDetTicks { .. }
                | PassRun::PassLoop { .. }
                | PassRun::GenerateMips { .. } => return Err("unsupported browser pass".into()),
            };
            Ok::<_, JsValue>(array([id.0.into(), slots.into(), work]))
        })
        .collect::<Result<Array, _>>()?;
    let modules = Map::new();
    for (id, module) in bundle.modules {
        modules.set(&id.0.into(), &module);
    }
    let buffers = Map::new();
    for (id, buffer) in bundle.buffers {
        buffers.set(&id.0.into(), &buffer);
    }
    let textures = decl
        .resources
        .textures
        .iter()
        .map(|t| {
            let desc =
                crate::storage::attachment(*t, &wgpu::Limits::default()).map_err(js_error)?;
            let format = match t.format {
                ImageFormat::Rg32Uint => "rg32uint",
                ImageFormat::Rgba16Float => "rgba16float",
                ImageFormat::Depth32Float => "depth32float",
            };
            let clear = t
                .clear
                .map_or(JsValue::NULL, |v| array(v.map(JsValue::from)));
            Ok::<_, JsValue>(array([
                t.table.0.into(),
                format.into(),
                array(t.extent.map(JsValue::from)),
                t.samples.into(),
                desc.usage.bits().into(),
                clear,
            ]))
        })
        .collect::<Result<Array, _>>()?;
    let samplers: Array = decl
        .resources
        .comparison_samplers
        .iter()
        .map(|id| JsValue::from(id.0))
        .collect();
    let initial = |ids: &mut dyn Iterator<Item = TableId>| -> Result<Map, JsValue> {
        let map = Map::new();
        for id in ids {
            map.set(&id.0.into(), &Uint8Array::from(table(id)?.initial.as_ref()));
        }
        Ok(map)
    };
    let frames =
        initial(&mut std::iter::once(frame.main).chain(frame.variants.iter().map(|&(_, id)| id)))?;
    let resets = initial(&mut decl.resources.resets.iter().copied())?;
    let host: Array = decl
        .tables
        .iter()
        .filter(|t| matches!(resources.role(t.id), crate::bundle::Role::HostAttachment))
        .map(|t| JsValue::from(t.id.0))
        .collect();
    let plan = Object::new();
    for (name, value) in [
        ("device", gpu.device),
        ("modules", modules.into()),
        ("buffers", buffers.into()),
        ("passes", passes.into()),
        ("textures", textures.into()),
        ("samplers", samplers.into()),
        ("frames", frames.into()),
        ("mainFrame", frame.main.0.into()),
        ("resets", resets.into()),
        ("hostAttachments", host.into()),
        ("params", Uint8Array::from(params.bytes()).into()),
    ] {
        Reflect::set(&plan, &name.into(), &value)?;
    }
    Ok(plan)
}

fn array<const N: usize>(values: [JsValue; N]) -> JsValue {
    values.into_iter().collect::<Array>().into()
}

fn js_error(error: crate::error::RunnerError) -> JsValue {
    format!("{error:?}").into()
}

fn method(target: &JsValue, name: &str) -> Result<Function, JsValue> {
    Reflect::get(target, &name.into())?
        .dyn_into()
        .map_err(|_| JsValue::from_str(name))
}
