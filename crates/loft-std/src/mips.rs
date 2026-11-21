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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dimension { D2, D3, Cube }

impl Dimension {
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

    pub fn group(self, max_z: u32, max_invocations: u32) -> [u32; 3] {
        match self {
            Self::D2 => [8, 8, 1],
            Self::D3 => [4, 4, 4],
            Self::Cube => {
                let z = max_z.min(max_invocations / 64);
                [8, 8, if z >= 6 { 6 } else { 1 }]
            },
        }
    }
}

pub fn source(dim: Dimension, format: &str, scalar: bool, [wx, wy, wz]: [u32; 3]) -> String {
    use std::fmt::Write;
    let (kind, projection, vector, load, store, count) = match dim {
        Dimension::D2 => ("2d", "id.xy", "vec2", "p, 0", "at", 4),
        Dimension::D3 => ("3d", "id", "vec3", "p, 0", "at", 8),
        Dimension::Cube => ("2d_array", "id.xy", "vec2", "p, i32(id.z), 0", "at, i32(id.z)", 4),
    };
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
    src
}
