//! WGSL lowering for the packed word operations.
use loft_check::quant::{Fp8, Quant};
use loft_ir::GpuFeatures;

fn wgsl(op: Quant, features: GpuFeatures) -> String {
    let (params, ty, body) = match op {
        Quant::DotI8 => ("a: u32, b: u32", "i32", dot_wgsl(true, features)),
        Quant::DotU8 => ("a: u32, b: u32", "u32", dot_wgsl(false, features)),
        Quant::Bf16 => (
            "a: u32",
            "vec2<f32>",
            "vec2<f32>(bitcast<f32>(a << 16u), bitcast<f32>(a & 0xffff0000u))".into(),
        ),
        Quant::PackBf16 => (
            "a: vec2<f32>",
            "u32",
            "loft_round_bf16(a.x) | (loft_round_bf16(a.y) << 16u)".into(),
        ),
        Quant::U4Lo => ("a: u32", "vec4<f32>", nibble_wgsl(0, false)),
        Quant::U4Hi => ("a: u32", "vec4<f32>", nibble_wgsl(16, false)),
        Quant::I4Lo => ("a: u32", "vec4<f32>", nibble_wgsl(0, true)),
        Quant::I4Hi => ("a: u32", "vec4<f32>", nibble_wgsl(16, true)),
        Quant::U2 => (
            "a: u32, i: u32",
            "f32",
            "f32(extractBits(a, min(i, 16u) * 2u, 2u))".into(),
        ),
        Quant::Fp8E4M3 => ("a: u32", "vec4<f32>", wgsl_call(Fp8::E4M3)),
        Quant::Fp8E5M2 => ("a: u32", "vec4<f32>", wgsl_call(Fp8::E5M2)),
    };
    let helper = match op {
        Quant::PackBf16 => BF16_WGSL.to_owned(),
        Quant::Fp8E4M3 => wgsl_decode(Fp8::E4M3),
        Quant::Fp8E5M2 => wgsl_decode(Fp8::E5M2),
        Quant::DotI8
        | Quant::DotU8
        | Quant::Bf16
        | Quant::U4Lo
        | Quant::U4Hi
        | Quant::I4Lo
        | Quant::I4Hi
        | Quant::U2 => String::new(),
    };
    format!(
        "{helper}fn loft_quant_{}({params}) -> {ty} {{ return {body}; }}\n",
        op.name()
    )
}

fn nibble_wgsl(start: u32, signed: bool) -> String {
    let a = if signed { "bitcast<i32>(a)" } else { "a" };
    let lanes = [0, 4, 8, 12].map(|i| format!("f32(extractBits({a}, {}u, 4u))", start + i));
    format!("vec4<f32>({})", lanes.join(", "))
}

const BF16_WGSL: &str = "
fn loft_round_bf16(value: f32) -> u32 {
    let bits = bitcast<u32>(value);
    if (bits & 0x7fffffffu) > 0x7f800000u {
        return (bits >> 16u) | 0x0040u;
    }
    return (bits + 0x7fffu + ((bits >> 16u) & 1u)) >> 16u;
}
";

fn wgsl_decode(format: Fp8) -> String {
    let (mantissa, bias, first_special) = format.layout();
    let special = match format {
        Fp8::E4M3 => "0x7fc00000u".to_owned(),
        Fp8::E5M2 => format!("select(0x7fc00000u, 0x7f800000u, mag == {first_special}u)"),
    };
    format!(
        "fn loft_fp8_m{mantissa}(bits: u32) -> f32 {{\n                let mag = bits & 127u;\n                let sign = (bits & 128u) << 24u;\n                if mag >= {first_special}u {{ return bitcast<f32>(sign | {special}); }}\n                let e = mag >> {mantissa}u;\n                let frac = mag & {mask}u;\n                let significand = frac + select({lead}u, 0u, e == 0u);\n                let scale = bitcast<f32>(({base}u + max(e, 1u)) << 23u);\n                return bitcast<f32>(sign | bitcast<u32>(f32(significand) * scale));\n            }}\n",
        mask = (1 << mantissa) - 1,
        lead = 1 << mantissa,
        base = 127 - bias - mantissa,
    )
}

fn wgsl_call(format: Fp8) -> String {
    let (mantissa, _, _) = format.layout();
    let lanes = [0, 8, 16, 24].map(|i| format!("loft_fp8_m{mantissa}(a >> {i}u)"));
    format!("vec4<f32>({})", lanes.join(", "))
}

fn dot_wgsl(signed: bool, features: GpuFeatures) -> String {
    if features.contains(GpuFeatures::PACKED_INT8_DOT) {
        return format!("dot4{}8Packed(a, b)", if signed { 'I' } else { 'U' });
    }
    // extractBits sign-extends only when its *input* is signed. Bitcast keeps the top byte intact.
    let (a, b) = if signed {
        ("bitcast<i32>(a)", "bitcast<i32>(b)")
    } else {
        ("a", "b")
    };
    [0, 8, 16, 24]
        .map(|i| format!("extractBits({a}, {i}u, 8u) * extractBits({b}, {i}u, 8u)"))
        .join(" + ")
}

pub fn wgsl_prelude(shader: &str, features: GpuFeatures) -> String {
    let mut out = String::new();
    for op in Quant::ALL {
        if shader.contains(&format!("loft_quant_{}(", op.name())) {
            out.push_str(&wgsl(op, features));
        }
    }
    out
}
