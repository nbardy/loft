//! Texture dimensions, launch geometry and sampling code for WGSL emission.
use loft_parse::ast::TexDim;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Clock {
    Load,
    Dirty(Vec<usize>),
}

pub fn size(dim: TexDim) -> [u32; 3] {
    match dim {
        TexDim::D2(w, h) => [w, h, 1],
        TexDim::D3(w, h, d) => [w, h, d],
        TexDim::Cube(n) => [n, n, 6],
    }
}

pub fn workgroup(dim: TexDim) -> [u32; 3] {
    match dim {
        TexDim::D2(..) => [8, 8, 1],
        TexDim::Cube(_) => [8, 8, 6],
        TexDim::D3(..) => [4, 4, 4],
    }
}

use loft_check::Prog;
use loft_parse::ast::BakeDecl;
use std::collections::BTreeSet;
use std::fmt::Write;

pub const SAMPLE_TEXTURE_PREFIX: &str = "loft_bake_texture_";
pub const SAMPLE_SAMPLER: &str = "loft_bake_sampler";

fn filterable(bake: &BakeDecl) -> bool {
    bake.format.name != "r32float"
}

pub(crate) fn sample_expression(bake: &BakeDecl, coord: &str, lod: &str, kernel: bool) -> String {
    let name = &bake.texel.name.name;
    let swizzle = if matches!(bake.format.name.as_str(), "r8unorm" | "r16float" | "r32float") {
        ".x"
    } else {
        ""
    };
    if filterable(bake) && !kernel {
        // Explicit LOD is legal in compute and vertex queries, where derivatives are absent.
        format!("textureSampleLevel({SAMPLE_TEXTURE_PREFIX}{name}, {SAMPLE_SAMPLER}, {coord}, {lod}){swizzle}")
    } else {
        format!("loft_bake_load_{name}({coord}, {lod}){swizzle}")
    }
}

pub(crate) fn sampled_declarations(p: &Prog<'_>, used: &BTreeSet<String>, kernel: bool) -> String {
    let mut out = String::new();
    if !kernel && used.iter().any(|name| filterable(p.bakes[name.as_str()])) {
        // Slots 0 and 1 are reserved for the existing query twin's IO.
        let _ = writeln!(out, "@group(3) @binding(2) var {SAMPLE_SAMPLER}: sampler;");
    }
    for (index, (&name, bake)) in p.bakes.iter().enumerate() {
        if !used.contains(name) { continue; }
        let slot = index + 3;
        let dim = match bake.dim {
            TexDim::D2(..) => "2d",
            TexDim::D3(..) => "3d",
            TexDim::Cube(_) => "cube",
        };
        let texture = format!("{SAMPLE_TEXTURE_PREFIX}{name}");
        let _ = writeln!(out, "@group(3) @binding({slot}) var {texture}: texture_{dim}<f32>;");
        let lanes = if matches!(bake.dim, TexDim::D3(..)) { 3 } else { 2 };
        if !filterable(bake) {
            // R32Float is unfilterable on baseline devices. Use nearest texel and mip.
            let _ = writeln!(out, "
fn loft_bake_load_{name}(uv: vec{lanes}f, lod: f32) -> vec4f {{
    let level = i32(clamp(round(lod), 0.0, f32(textureNumLevels({texture}) - 1u)));
    let size = vec{lanes}i(textureDimensions({texture}, level));
    let pixel = clamp(vec{lanes}i(floor(uv * vec{lanes}f(size))), vec{lanes}i(0), size - 1);
    return textureLoad({texture}, pixel, level);
}}");
        } else if kernel {
            // The existing runner supports sampler-free external image bindings.
            let offsets = if lanes == 2 { "i & 1, (i >> 1) & 1" } else { "i & 1, (i >> 1) & 1, (i >> 2) & 1" };
            let weights = if lanes == 2 { "w.x * w.y" } else { "w.x * w.y * w.z" };
            let count = 1 << lanes;
            let _ = writeln!(out, "
fn loft_bake_level_{name}(uv: vec{lanes}f, level: i32) -> vec4f {{
    let size = vec{lanes}i(textureDimensions({texture}, level));
    let p = clamp(uv, vec{lanes}f(0.0), vec{lanes}f(1.0)) * vec{lanes}f(size) - 0.5;
    let origin = vec{lanes}i(floor(p));
    let f = fract(p);
    var sum = vec4f(0.0);
    for (var i = 0; i < {count}; i++) {{
        let offset = vec{lanes}i({offsets});
        let w = mix(1.0 - f, f, vec{lanes}f(offset));
        let pixel = clamp(origin + offset, vec{lanes}i(0), size - 1);
        sum += {weights} * textureLoad({texture}, pixel, level);
    }}
    return sum;
}}
fn loft_bake_load_{name}(uv: vec{lanes}f, lod: f32) -> vec4f {{
    let last = i32(textureNumLevels({texture}) - 1u);
    let level = clamp(lod, 0.0, f32(last));
    let base = i32(level);
    let a = loft_bake_level_{name}(uv, base);
    if base == last || fract(level) == 0.0 {{ return a; }}
    return mix(a, loft_bake_level_{name}(uv, base + 1), fract(level));
}}");
        }
    }
    out
}
