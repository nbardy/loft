use crate::{command::{Dispatch, Pass, Stamps, Work}, error::{Result, RunnerError}};
use wgpu::*;

fn storage_view(dim: Dimension) -> TextureViewDimension {
    match dim {
        Dimension::D2 => TextureViewDimension::D2,
        Dimension::D3 => TextureViewDimension::D3,
        Dimension::Cube => TextureViewDimension::D2Array,
    }
}

pub use loft_std::mips::{Clock, Dimension};
pub fn source(dim: Dimension, format: TextureFormat, limits: &Limits) -> Result<String> {
    let (format, scalar) = match format {
        TextureFormat::R8Unorm => ("r8unorm", true),
        TextureFormat::R16Float => ("r16float", true),
        TextureFormat::R32Float => ("r32float", true),
        TextureFormat::Rgba8Unorm => ("rgba8unorm", false),
        TextureFormat::Rgba16Float => ("rgba16float", false),
        _ => return Err(RunnerError::Unsupported("unsupported compute mip format")),
    };
    Ok(loft_std::mips::source(dim, format, scalar,
        dim.group(limits.max_compute_workgroup_size_z, limits.max_compute_invocations_per_workgroup)))
}

pub fn portable(tex: &Texture) -> crate::error::Result<Dimension> {
    if tex.dimension() != TextureDimension::D2
        || tex.depth_or_array_layers() != 1
        || tex.mip_level_count() < 2
        || tex.format() != TextureFormat::Rgba16Float
        || !tex
            .usage()
            .contains(TextureUsages::TEXTURE_BINDING | TextureUsages::STORAGE_BINDING)
    {
        return Err(crate::error::RunnerError::Unsupported(
            "mips require a 2D rgba16float texture with sampled and storage usages",
        ));
    }
    Ok(Dimension::D2)
}

pub fn bindings(dim: Dimension, format: TextureFormat) -> [BindGroupLayoutEntry; 2] {
    let view_dimension = storage_view(dim);
    let entry = |binding, ty| BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::COMPUTE,
        ty,
        count: None,
    };
    [
        entry(
            0,
            BindingType::Texture {
                sample_type: TextureSampleType::Float { filterable: false },
                view_dimension,
                multisampled: false,
            },
        ),
        entry(
            1,
            BindingType::StorageTexture {
                access: StorageTextureAccess::WriteOnly,
                format,
                view_dimension,
            },
        ),
    ]
}

use std::{collections::HashMap, sync::Arc};

#[derive(Default)]
pub struct Pipelines(HashMap<(Dimension, TextureFormat, Option<[u32; 3]>), ComputePipeline>);
pub struct Mips(Vec<(Pass, Option<Packed>)>);

enum Packing {
    Unorm8,
    HalfNearest,
    HalfTowardZero,
}

pub struct Packed {
    pub buffer: Arc<Buffer>,
    texture: Texture,
    extent: Extent3d,
    level: u32,
    pitch: u32,
    dim: Dimension,
    packing: Packing,
}
impl Packed {
    pub fn new(device: &Device, texture: &Texture, dim: Dimension, level: u32) -> Result<Self> {
        let [width, height, depth_or_array_layers] = dim.extent(
            [texture.width(), texture.height(), texture.depth_or_array_layers()], level);
        let (bytes, packing) = match (texture.format(), device.adapter_info().backend) {
            (TextureFormat::R8Unorm, _) => (1, Packing::Unorm8),
            (TextureFormat::R16Float, Backend::Metal) => (2, Packing::HalfTowardZero),
            (TextureFormat::R16Float, _) => (2, Packing::HalfNearest),
            _ => return Err(RunnerError::Unsupported("packed bake format")),
        };
        let pitch = width.checked_mul(bytes).and_then(|r| r.checked_next_multiple_of(COPY_BYTES_PER_ROW_ALIGNMENT))
            .ok_or(RunnerError::Capacity)?;
        let size = u64::from(pitch) * u64::from(height) * u64::from(depth_or_array_layers);
        let limits = device.limits();
        if size > limits.max_buffer_size.min(limits.max_storage_buffer_binding_size) { return Err(RunnerError::Capacity); }
        Ok(Self {
            buffer: crate::allocate(device, size, BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST),
            texture: texture.clone(), extent: Extent3d { width, height, depth_or_array_layers }, level, pitch, dim, packing,
        })
    }

    pub fn shader(&self, source: &str, target: &str, group: u32, slot: u32) -> Result<String> {
        let (args, layer, dimensions) = match self.dim {
            Dimension::D2 => ("p: vec2i", "0u", format!("vec2u({}u, {}u)", self.extent.width, self.extent.height)),
            Dimension::Cube => ("p: vec2i, face: i32", "u32(face)", format!("vec2u({}u, {}u)", self.extent.width, self.extent.height)),
            Dimension::D3 => ("p: vec3i", "u32(p.z)", format!("vec3u({}u, {}u, {}u)", self.extent.width, self.extent.height, self.extent.depth_or_array_layers)),
        };
        let (per_word, value) = match self.packing {
            Packing::Unorm8 => (4, "let bits = pack4x8unorm(vec4f(v.x, 0.0, 0.0, 0.0));"),
            Packing::HalfNearest => (2, "let bits = pack2x16float(vec2f(v.x, 0.0)) & 65535u;"),
            // Metal's R16Float texture store truncates the magnitude. The pack builtin
            // rounds to nearest, so move one half step toward zero if it rounded away.
            Packing::HalfTowardZero => (2, "
  let nearest = pack2x16float(vec2f(v.x, 0.0)) & 65535u;
  let rounded = unpack2x16float(nearest).x;
  let bits = nearest - select(0u, 1u, abs(rounded) > abs(v.x));"),
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
  {value}
  atomicOr(&loft_packed[i], bits << ((x % {per_word}u) * {bits}u));
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
    pub fn prepare(&mut self, device: &Device, texture: &Texture, dim: Dimension,
        cache: Option<&PipelineCache>, clock: Option<fn() -> std::time::Duration>, times: &mut Vec<(String, f64)>) -> Result<Mips> {
        let format = texture.format();
        let limits = device.limits();
        let size = [texture.width(),texture.height(),texture.depth_or_array_layers()];
        let views = (0..texture.mip_level_count()).map(|level| texture.create_view(&TextureViewDescriptor {
            dimension: Some(storage_view(dim)), base_mip_level:level, mip_level_count:Some(1), ..Default::default()
        })).collect::<Vec<_>>();
        let levels = views.windows(2).enumerate().map(|(level, pair)| {
            let level = level as u32 + 1;
            let extent = dim.extent(size, level);
            let packed = if texture.usage().contains(TextureUsages::STORAGE_BINDING) { None }
                else { Some(Packed::new(device, texture, dim, level)?) };
            let pipeline = match self.0.entry((dim, format, packed.as_ref().map(|_| extent))) {
                std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::hash_map::Entry::Vacant(entry) => {
                    let mut text = source(dim, format, &limits)?;
                    let mut entries = bindings(dim, format);
                    if let Some(output) = &packed {
                        text = output.shader(&text, "dst", 0, 1)?;
                        entries[1] = crate::bindings::slot(1, BufferBindingType::Storage { read_only: false });
                    }
                    let group = device.create_bind_group_layout(&BindGroupLayoutDescriptor { label:None, entries:&entries });
                    let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor { label:None, bind_group_layouts:&[Some(&group)], immediate_size:0 });
                    let module = device.create_shader_module(ShaderModuleDescriptor { label:Some("mip reduction"), source:ShaderSource::Wgsl(text.into()) });
                    let (pipe, ms) = crate::timed(clock, || device.create_compute_pipeline(&ComputePipelineDescriptor { label:None, layout:Some(&layout), module:&module,
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
            let work = Work::Direct(std::array::from_fn(|i| extent[i].div_ceil(dim.group(limits.max_compute_workgroup_size_z, limits.max_compute_invocations_per_workgroup)[i])));
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
