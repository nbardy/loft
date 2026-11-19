//! Compute mip chains and bake invalidation. Each level binds disjoint sampled and storage views.
use std::{collections::HashMap, fmt::Write};
use wgpu::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dimension {
    D2,
    D3,
    Cube,
}

impl Dimension {
    pub fn storage_view(self) -> TextureViewDimension {
        match self {
            Self::D2 => TextureViewDimension::D2,
            Self::D3 => TextureViewDimension::D3,
            Self::Cube => TextureViewDimension::D2Array,
        }
    }

    pub fn extent(self, original: [u32; 3], level: u32) -> [u32; 3] {
        let [w, h, d] = original.map(|x| (x >> level).max(1));
        [w, h, match self { Self::D2 => 1, Self::D3 => d, Self::Cube => 6 }]
    }

    fn group(self) -> [u32; 3] {
        match self { Self::D2 => [8, 8, 1], Self::D3 => [4, 4, 4], Self::Cube => [8, 8, 6] }
    }
}

#[derive(Debug)]
pub enum Clock {
    Load { done: bool },
    Dirty { words: Vec<usize>, last: Option<Vec<u32>> },
}

impl Clock {
    /// A dirty bake has no content until the first frame. Compare watched bits
    /// to the last encoded values so edits that are reverted before a frame are free.
    pub fn due(&self, loading: bool, params: &[f32]) -> bool {
        match self {
            Self::Load { done } => loading && !done,
            Self::Dirty { words, last } => !loading && last.as_ref().is_none_or(|bits| {
                words.iter().zip(bits).any(|(&word, &old)| params[word].to_bits() != old)
            }),
        }
    }

    pub fn complete(&mut self, params: &[f32]) {
        match self {
            Self::Load { done } => *done = true,
            Self::Dirty { words, last } => *last = Some(words.iter().map(|&i| params[i].to_bits()).collect()),
        }
    }
}

pub fn source(dim: Dimension, format: TextureFormat) -> Result<String, &'static str> {
    let format = match format {
        TextureFormat::R8Unorm => "r8unorm",
        TextureFormat::R16Float => "r16float",
        TextureFormat::R32Float => "r32float",
        TextureFormat::Rgba8Unorm => "rgba8unorm",
        TextureFormat::Rgba16Float => "rgba16float",
        _ => return Err("unsupported compute mip format"),
    };
    let (kind, projection, vector, load, store, count) = match dim {
        Dimension::D2 => ("2d", "id.xy", "vec2", "p, 0", "at", 4),
        Dimension::D3 => ("3d", "id", "vec3", "p, 0", "at", 8),
        Dimension::Cube => ("2d_array", "id.xy", "vec2", "p, i32(id.z), 0", "at, i32(id.z)", 4),
    };
    let [wx, wy, wz] = dim.group();
    let offset = if dim == Dimension::D3 {
        "vec3<i32>(vec3<u32>(i & 1u, (i >> 1u) & 1u, i >> 2u))"
    } else {
        "vec2<i32>(vec2<u32>(i & 1u, i >> 1u))"
    };
    let mut src = String::new();
    let _ = write!(src, r"
@group(0) @binding(0) var src: texture_{kind}<f32>;
@group(0) @binding(1) var dst: texture_storage_{kind}<{format}, write>;
@compute @workgroup_size({wx}, {wy}, {wz}) fn downsample(@builtin(global_invocation_id) id: vec3<u32>) {{
  let at = {vector}<i32>({projection});
  if any({projection} >= textureDimensions(dst)) {{ return; }}
  let last = {vector}<i32>(textureDimensions(src)) - 1;
  var sum = vec4<f32>(0.0);
  for (var i = 0u; i < {count}u; i++) {{
    let p = min(2*at + {offset}, last);
    sum += textureLoad(src, {load});
  }}
  textureStore(dst, {store}, sum / {count}.0);
}}
");
    Ok(src)
}

#[derive(Default)]
pub struct Pipelines(HashMap<(Dimension, TextureFormat), (BindGroupLayout, ComputePipeline)>);

pub struct Mips {
    pipeline: ComputePipeline,
    work: Vec<(BindGroup, [u32; 3])>,
}

impl Pipelines {
    pub fn prepare(&mut self, device: &Device, texture: &Texture, dim: Dimension) -> Result<Mips, String> {
        let format = texture.format();
        let key = (dim, format);
        if let std::collections::hash_map::Entry::Vacant(vacant) = self.0.entry(key) {
            let text = source(dim, format)?;
            let ir = naga::front::wgsl::parse_str(&text).map_err(|e| e.emit_to_string(&text))?;
            naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::default())
                .validate(&ir).map_err(|e| e.emit_to_string(&text))?;
            let entry = |binding, ty| BindGroupLayoutEntry { binding, visibility: ShaderStages::COMPUTE, ty, count: None };
            let view_dimension = dim.storage_view();
            let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("mip bindings"),
                entries: &[
                    entry(0, BindingType::Texture { sample_type: TextureSampleType::Float { filterable: false }, view_dimension, multisampled: false }),
                    entry(1, BindingType::StorageTexture { access: StorageTextureAccess::WriteOnly, format, view_dimension }),
                ],
            });
            let pl = device.create_pipeline_layout(&PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
            let module = device.create_shader_module(ShaderModuleDescriptor { label: Some("mip downsample"), source: ShaderSource::Naga(std::borrow::Cow::Owned(ir)) });
            let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some("mip downsample"), layout: Some(&pl), module: &module, entry_point: Some("downsample"), compilation_options: Default::default(), cache: None,
            });
            vacant.insert((layout, pipeline));
        }
        let (layout, pipeline) = &self.0[&key];
        let size = texture.size();
        let size = [size.width, size.height, size.depth_or_array_layers];
        let group = dim.group();
        let view = |level| texture.create_view(&TextureViewDescriptor {
            dimension: Some(dim.storage_view()), base_mip_level: level, mip_level_count: Some(1), ..Default::default()
        });
        let work = (1..texture.mip_level_count()).map(|level| {
            let src = view(level - 1);
            let dst = view(level);
            let bg = device.create_bind_group(&BindGroupDescriptor {
                label: None, layout,
                entries: &[
                    BindGroupEntry { binding: 0, resource: BindingResource::TextureView(&src) },
                    BindGroupEntry { binding: 1, resource: BindingResource::TextureView(&dst) },
                ],
            });
            let extent = dim.extent(size, level);
            (bg, std::array::from_fn(|i| extent[i].div_ceil(group[i])))
        }).collect();
        Ok(Mips { pipeline: pipeline.clone(), work })
    }
}

impl Mips {
    pub fn encode(&self, enc: &mut CommandEncoder, last_timestamp: Option<(&QuerySet, u32)>) -> u32 {
        for (index, (group, [x, y, z])) in self.work.iter().enumerate() {
            let timestamp_writes = last_timestamp.filter(|_| index + 1 == self.work.len()).map(|(query_set, end)| ComputePassTimestampWrites {
                query_set, beginning_of_pass_write_index: None, end_of_pass_write_index: Some(end),
            });
            let mut pass = enc.begin_compute_pass(&ComputePassDescriptor { label: Some("generate mips"), timestamp_writes });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.dispatch_workgroups(*x, *y, *z);
        }
        self.work.len() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_dimension_and_format_validates() {
        for dim in [Dimension::D2, Dimension::D3, Dimension::Cube] {
            for format in [TextureFormat::R8Unorm, TextureFormat::R16Float, TextureFormat::R32Float, TextureFormat::Rgba8Unorm, TextureFormat::Rgba16Float] {
                let text = source(dim, format).unwrap();
                let ir = naga::front::wgsl::parse_str(&text).unwrap_or_else(|e| panic!("{}", e.emit_to_string(&text)));
                naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::default())
                    .validate(&ir).unwrap_or_else(|e| panic!("{}", e.emit_to_string(&text)));
            }
        }
        assert_eq!(Dimension::Cube.extent([128, 128, 6], 7), [1, 1, 6]);
        assert_eq!(Dimension::D3.extent([2, 5, 64], 3), [1, 1, 8]);
    }

    #[test]
    fn initial_and_dirty_clocks_are_edge_triggered() {
        let mut load = Clock::Load { done: false };
        let mut dirty = Clock::Dirty { words: vec![1], last: None };
        assert!(load.due(true, &[]));
        assert!(!dirty.due(true, &[]));
        load.complete(&[]);
        assert!(!load.due(true, &[]));
        assert!(!load.due(false, &[]));
        assert!(dirty.due(false, &[1.0, 0.0]));
        dirty.complete(&[1.0, 0.0]);
        assert!(!dirty.due(false, &[0.0, 0.0]));
        assert!(dirty.due(false, &[0.0, 1.0]));
        // No encode yet, so a reverted edit remains clean.
        assert!(!dirty.due(false, &[0.0, 0.0]));
        dirty.complete(&[0.0, 1.0]);
        assert!(!dirty.due(false, &[0.0, 1.0]));
    }
}
