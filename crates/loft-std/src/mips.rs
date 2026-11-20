//! Compute mip chains and bake invalidation. Each level binds disjoint sampled and storage views.
use std::fmt::Write;
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
        [
            w,
            h,
            match self {
                Self::D2 => 1,
                Self::D3 => d,
                Self::Cube => 6,
            },
        ]
    }

    pub fn group(self, limits: &Limits) -> [u32; 3] {
        match self {
            Self::D2 => [8, 8, 1],
            Self::D3 => [4, 4, 4],
            Self::Cube => {
                let z = limits.max_compute_workgroup_size_z.min(limits.max_compute_invocations_per_workgroup / 64);
                [8, 8, if z >= 6 { 6 } else { 1 }]
            },
        }
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
            Self::Dirty { words, last } => {
                !loading
                    && last.as_ref().is_none_or(|bits| {
                        words
                            .iter()
                            .zip(bits)
                            .any(|(&word, &old)| params[word].to_bits() != old)
                    })
            }
        }
    }

    pub fn complete(&mut self, params: &[f32]) {
        match self {
            Self::Load { done } => *done = true,
            Self::Dirty { words, last } => *last = Some(words.iter().map(|&i| params[i].to_bits()).collect()),
        }
    }
}

pub fn source(dim: Dimension, format: TextureFormat, limits: &Limits) -> Result<String, &'static str> {
    let (format, scalar) = match format {
        TextureFormat::R8Unorm => ("r8unorm", true),
        TextureFormat::R16Float => ("r16float", true),
        TextureFormat::R32Float => ("r32float", true),
        TextureFormat::Rgba8Unorm => ("rgba8unorm", false),
        TextureFormat::Rgba16Float => ("rgba16float", false),
        _ => return Err("unsupported compute mip format"),
    };
    let (kind, projection, vector, load, store, count) = match dim {
        Dimension::D2 => ("2d", "id.xy", "vec2", "p, 0", "at", 4),
        Dimension::D3 => ("3d", "id", "vec3", "p, 0", "at", 8),
        Dimension::Cube => ("2d_array", "id.xy", "vec2", "p, i32(id.z), 0", "at, i32(id.z)", 4),
    };
    let [wx, wy, wz] = dim.group(limits);
    let zero = if scalar { "0.0" } else { "vec4<f32>(0.0)" };
    let channel = if scalar { ".x" } else { "" };
    let average = if scalar {
        format!("vec4(sum / {count}.0, 0.0, 0.0, 1.0)")
    } else {
        format!("sum / {count}.0")
    };
    let mut taps = String::new();
    // Match the original left-to-right sum and perform a texture store at each
    // level, including the quantization of an immediately consumed level.
    for i in 0..count {
        let x = i & 1;
        let y = (i >> 1) & 1;
        let offset = if dim == Dimension::D3 {
            format!("vec3({x}, {y}, {})", i >> 2)
        } else {
            format!("vec2({x}, {y})")
        };
        let _ = writeln!(
            taps,
            "  {{ let p = min(2*at + {offset}, last); sum += textureLoad(src, {load}){channel}; }}"
        );
    }
    let mut src = String::new();
    let _ = write!(
        src,
        r"
@group(0) @binding(0) var src: texture_{kind}<f32>;
@group(0) @binding(1) var dst: texture_storage_{kind}<{format}, write>;
@compute @workgroup_size({wx}, {wy}, {wz}) fn downsample(@builtin(global_invocation_id) id: vec3<u32>) {{
  let at = {vector}<i32>({projection});
  if any({projection} >= textureDimensions(dst)) {{ return; }}
  let last = {vector}<i32>(textureDimensions(src)) - 1;
  var sum = {zero};
{taps}  textureStore(dst, {store}, {average});
}}
"
    );
    Ok(src)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_dimension_and_format_validates() {
        for dim in [Dimension::D2, Dimension::D3, Dimension::Cube] {
            for format in [
                TextureFormat::R8Unorm,
                TextureFormat::R16Float,
                TextureFormat::R32Float,
                TextureFormat::Rgba8Unorm,
                TextureFormat::Rgba16Float,
            ] {
                let text = source(dim, format, &Limits::default()).unwrap();
                let ir =
                    naga::front::wgsl::parse_str(&text).unwrap_or_else(|e| panic!("{}", e.emit_to_string(&text)));
                naga::valid::Validator::new(
                    naga::valid::ValidationFlags::all(),
                    naga::valid::Capabilities::default(),
                )
                .validate(&ir)
                .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&text)));
            }
        }
        assert_eq!(Dimension::Cube.extent([128, 128, 6], 7), [1, 1, 6]);
        assert_eq!(Dimension::D3.extent([2, 5, 64], 3), [1, 1, 8]);
    }

    #[test]
    fn initial_and_dirty_clocks_are_edge_triggered() {
        let mut load = Clock::Load { done: false };
        let mut dirty = Clock::Dirty {
            words: vec![1],
            last: None,
        };
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
    let view_dimension = dim.storage_view();
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
