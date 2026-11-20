use super::command::{Dispatch, Pass, Stamps, Work};
use super::{Result, RunnerError};
pub use loft_std::mips::{Clock, Dimension};
use std::{collections::HashMap, sync::Arc};
use wgpu::*;

#[derive(Default)]
pub struct Pipelines(HashMap<(Dimension, TextureFormat, Option<[u32; 3]>), ComputePipeline>);
pub struct Mips(Vec<(Pass, Option<Packed>)>);

pub(super) struct Packed {
    pub buffer: Arc<Buffer>,
    texture: Texture,
    extent: Extent3d,
    level: u32,
    pitch: u32,
    dim: Dimension,
}
impl Packed {
    pub fn new(device: &Device, texture: &Texture, dim: Dimension, level: u32) -> Result<Self> {
        let [width, height, depth_or_array_layers] = dim.extent(
            [texture.width(), texture.height(), texture.depth_or_array_layers()], level);
        let bytes = match texture.format() {
            TextureFormat::R8Unorm => 1,
            TextureFormat::R16Float => 2,
            _ => return Err(RunnerError::Unsupported("packed bake format")),
        };
        let pitch = width.checked_mul(bytes).and_then(|r| r.checked_next_multiple_of(COPY_BYTES_PER_ROW_ALIGNMENT))
            .ok_or(RunnerError::Capacity)?;
        let size = u64::from(pitch) * u64::from(height) * u64::from(depth_or_array_layers);
        let limits = device.limits();
        if size > limits.max_buffer_size.min(limits.max_storage_buffer_binding_size) { return Err(RunnerError::Capacity); }
        Ok(Self {
            buffer: super::allocate(device, size, BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST),
            texture: texture.clone(), extent: Extent3d { width, height, depth_or_array_layers }, level, pitch, dim,
        })
    }

    pub fn shader(&self, source: &str, target: &str, group: u32, slot: u32) -> Result<String> {
        let (args, layer, dimensions) = match self.dim {
            Dimension::D2 => ("p: vec2i", "0u", format!("vec2u({}u, {}u)", self.extent.width, self.extent.height)),
            Dimension::Cube => ("p: vec2i, face: i32", "u32(face)", format!("vec2u({}u, {}u)", self.extent.width, self.extent.height)),
            Dimension::D3 => ("p: vec3i", "u32(p.z)", format!("vec3u({}u, {}u, {}u)", self.extent.width, self.extent.height, self.extent.depth_or_array_layers)),
        };
        let (per_word, mask, value) = match self.texture.format() {
            TextureFormat::R8Unorm => (4, 255, "pack4x8unorm(vec4f(v.x, 0.0, 0.0, 0.0))"),
            TextureFormat::R16Float => (2, 65535, "pack2x16float(vec2f(v.x, 0.0))"),
            _ => return Err(RunnerError::Unsupported("packed bake format")),
        };
        let declaration = format!("var {target}: texture_storage_");
        let begin = source.find(&declaration).ok_or(RunnerError::AbiMismatch)?;
        let end = begin + source[begin..].find(';').ok_or(RunnerError::AbiMismatch)? + 1;
        let attributes = source[..begin].rfind("@group(").ok_or(RunnerError::AbiMismatch)?;
        let store = format!("textureStore({target},");
        if !source[end..].contains(&store) { return Err(RunnerError::AbiMismatch); }
        let row = self.pitch / 4;
        let h = self.extent.height;
        let bits = 32 / per_word;
        // Two or four invocations can own distinct texels in one output word.
        // The pass clears the buffer first, allowing atomicOr with disjoint masks.
        let writer = format!(r"
@group({group}) @binding({slot}) var<storage, read_write> loft_packed: array<atomic<u32>>;
fn loft_pack({args}, v: vec4f) {{
  let x = u32(p.x);
  let i = ({layer} * {h}u + u32(p.y)) * {row}u + x / {per_word}u;
  atomicOr(&loft_packed[i], ({value} & {mask}u) << ((x % {per_word}u) * {bits}u));
}}
");
        let calls = source[end..].replace(&store, "loft_pack(")
            .replace(&format!("textureDimensions({target})"), &dimensions);
        Ok(format!("{}{writer}{calls}", &source[..attributes]))
    }

    pub fn record(&self, pass: &Pass, enc: &mut CommandEncoder, stamps: Option<Stamps<'_>>) -> u32 {
        enc.clear_buffer(&self.buffer, 0, None);
        let count = pass.record(enc, stamps);
        enc.copy_buffer_to_texture(
            TexelCopyBufferInfo { buffer: &self.buffer, layout: TexelCopyBufferLayout {
                offset: 0, bytes_per_row: Some(self.pitch), rows_per_image: Some(self.extent.height),
            }},
            TexelCopyTextureInfo { texture: &self.texture, mip_level: self.level, origin: Origin3d::ZERO, aspect: TextureAspect::All },
            self.extent,
        );
        count
    }
}

impl Pipelines {
    pub(super) fn prepare(&mut self, device: &Device, texture: &Texture, dim: Dimension,
        cache: Option<&PipelineCache>, times: &mut Vec<(String, f64)>) -> Result<Mips> {
        let format = texture.format();
        let limits = device.limits();
        let size = [texture.width(),texture.height(),texture.depth_or_array_layers()];
        let views = (0..texture.mip_level_count()).map(|level| texture.create_view(&TextureViewDescriptor {
            dimension: Some(dim.storage_view()), base_mip_level:level, mip_level_count:Some(1), ..Default::default()
        })).collect::<Vec<_>>();
        let levels = views.windows(2).enumerate().map(|(level, pair)| {
            let level = level as u32 + 1;
            let extent = dim.extent(size, level);
            let packed = if texture.usage().contains(TextureUsages::STORAGE_BINDING) { None }
                else { Some(Packed::new(device, texture, dim, level)?) };
            let pipeline = match self.0.entry((dim, format, packed.as_ref().map(|_| extent))) {
                std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let mut text = loft_std::mips::source(dim, format, &limits).map_err(RunnerError::Unsupported)?;
                    let mut entries = loft_std::mips::bindings(dim, format);
                    if let Some(output) = &packed {
                        text = output.shader(&text, "dst", 0, 1)?;
                        entries[1] = loft_std::bindings::slot(1, BufferBindingType::Storage { read_only: false });
                    }
                    let group = device.create_bind_group_layout(&BindGroupLayoutDescriptor { label:None, entries:&entries });
                    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor { label:None, bind_group_layouts:&[Some(&group)], immediate_size:0 });
                    let module = device.create_shader_module(ShaderModuleDescriptor { label:Some("mip reduction"), source:ShaderSource::Wgsl(text.into()) });
                    let (pipe, ms) = super::encode::timed(|| device.create_compute_pipeline(&ComputePipelineDescriptor { label:None, layout:Some(&layout), module:&module,
                        entry_point:Some("downsample"), compilation_options:Default::default(), cache }));
                    times.push((format!("mip {dim:?} {format:?}"), ms));
                    entry.insert(pipe)
                }
            };
            let layout = pipeline.get_bind_group_layout(0);
            let target = match &packed {
                Some(output) => output.buffer.as_entire_binding(),
                None => BindingResource::TextureView(&pair[1]),
            };
            let entries = [BindGroupEntry { binding:0, resource:BindingResource::TextureView(&pair[0]) },
                BindGroupEntry { binding:1, resource:target }];
            let bg = device.create_bind_group(&BindGroupDescriptor { label:None, layout:&layout, entries:&entries });
            let work = Work::Direct(std::array::from_fn(|i| extent[i].div_ceil(dim.group(&limits)[i])));
            Ok((Pass::Compute(vec![Dispatch { pipeline:pipeline.clone(), groups:vec![bg], work }]), packed))
        }).collect::<Result<_>>()?;
        Ok(Mips(levels))
    }
}
impl Mips {
    pub fn encode(&self, enc: &mut CommandEncoder, timestamps: Option<(&QuerySet,u32)>) -> u32 {
        for (i, (pass, packed)) in self.0.iter().enumerate() {
            let last = i+1 == self.0.len();
            let stamps = timestamps.filter(|_| i == 0 || last).map(|(set, start)| Stamps {
                set, start:(i == 0).then_some(start), end:last.then_some(start + 1)
            });
            match packed {
                Some(output) => output.record(pass, enc, stamps),
                None => pass.record(enc, stamps),
            };
        }
        self.0.len() as u32
    }
}
