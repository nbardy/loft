//! std::quant, over the ordinary u32 words of the frozen storage ABI.
use crate::eval::{Scalar, Val};
use loft_ir::{Code, Diag, GpuFeatures, Res, Span};
use loft_parse::ast::Ty;

#[derive(Clone, Copy)]
pub(crate) enum Quant {
    DotI8,
    DotU8,
    Bf16,
    PackBf16,
    U4Lo,
    U4Hi,
    I4Lo,
    I4Hi,
    U2,
    Fp8E4M3,
    Fp8E5M2,
}

impl Quant {
    const ALL: [Self; 11] = [
        Self::DotI8,
        Self::DotU8,
        Self::Bf16,
        Self::PackBf16,
        Self::U4Lo,
        Self::U4Hi,
        Self::I4Lo,
        Self::I4Hi,
        Self::U2,
        Self::Fp8E4M3,
        Self::Fp8E5M2,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::DotI8 => "dot4_i8",
            Self::DotU8 => "dot4_u8",
            Self::Bf16 => "unpack_bf16x2",
            Self::U4Lo => "unpack_u4_lo",
            Self::U4Hi => "unpack_u4_hi",
            Self::I4Lo => "unpack_i4_lo",
            Self::I4Hi => "unpack_i4_hi",
            Self::U2 => "unpack_u2",
            Self::Fp8E4M3 => "unpack_fp8_e4m3",
            Self::Fp8E5M2 => "unpack_fp8_e5m2",
            Self::PackBf16 => "pack_bf16x2",
        }
    }

    pub(crate) fn resolve(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|op| op.name() == name)
    }

    pub(crate) fn signature(self) -> (&'static [Ty], Ty) {
        match self {
            Self::DotI8 => (&[Ty::U32, Ty::U32], Ty::I32),
            Self::DotU8 => (&[Ty::U32, Ty::U32], Ty::U32),
            Self::Bf16 => (&[Ty::U32], Ty::VecN(2)),
            Self::PackBf16 => (&[Ty::VecN(2)], Ty::U32),
            Self::U2 => (&[Ty::U32, Ty::U32], Ty::F32),
            Self::U4Lo | Self::U4Hi | Self::I4Lo | Self::I4Hi | Self::Fp8E4M3 | Self::Fp8E5M2 => {
                (&[Ty::U32], Ty::VecN(4))
            }
        }
    }

    pub(crate) fn twin<S: Scalar>(self, words: &[Val<S>], span: Span) -> Res<Val<S>> {
        let word = |i: usize| {
            let Val::U(value) = words[i] else {
                unreachable!("checked packed word")
            };
            value
        };
        let vector =
            |lanes: &[f32]| Val::V(lanes.iter().map(|&x| S::c(f64::from(x))).collect(), 0.0);
        Ok(match self {
            Self::DotI8 => Val::I(
                word(0)
                    .to_le_bytes()
                    .into_iter()
                    .zip(word(1).to_le_bytes())
                    .map(|(a, b)| i32::from(a as i8) * i32::from(b as i8))
                    .sum(),
            ),
            Self::DotU8 => Val::U(
                word(0)
                    .to_le_bytes()
                    .into_iter()
                    .zip(word(1).to_le_bytes())
                    .map(|(a, b)| u32::from(a) * u32::from(b))
                    .sum(),
            ),
            Self::Bf16 => {
                let a = word(0);
                vector(&[f32::from_bits(a << 16), f32::from_bits(a & 0xffff_0000)])
            }
            Self::PackBf16 => {
                let [Val::V(lanes, _)] = words else {
                    unreachable!("checked BF16 pair")
                };
                let lane = |i: usize| {
                    lanes[i]
                        .exact()
                        .map(|x| round_bf16(x as f32))
                        .ok_or_else(|| {
                            Diag::new(
                                Code::E211,
                                span,
                                "BF16 packing requires exact input lanes",
                                "pack outside spatial proof",
                            )
                        })
                };
                Val::U(lane(0)? | (lane(1)? << 16))
            }
            Self::U4Lo => vector(&unpack_nibbles(word(0), false)),
            Self::U4Hi => vector(&unpack_nibbles(word(0) >> 16, false)),
            Self::I4Lo => vector(&unpack_nibbles(word(0), true)),
            Self::I4Hi => vector(&unpack_nibbles(word(0) >> 16, true)),
            // WGSL extractBits returns zero for an empty extraction at offset 32.
            Self::U2 => Val::S(S::c(f64::from(
                word(0).checked_shr(word(1).min(16) * 2).unwrap_or(0) & 3,
            ))),
            Self::Fp8E4M3 => vector(&word(0).to_le_bytes().map(|lane| Fp8::E4M3.unpack(lane))),
            Self::Fp8E5M2 => vector(&word(0).to_le_bytes().map(|lane| Fp8::E5M2.unpack(lane))),
        })
    }

    fn wgsl(self, features: GpuFeatures) -> String {
        let (params, ty, body) = match self {
            Self::DotI8 => ("a: u32, b: u32", "i32", dot_wgsl(true, features)),
            Self::DotU8 => ("a: u32, b: u32", "u32", dot_wgsl(false, features)),
            Self::Bf16 => (
                "a: u32",
                "vec2<f32>",
                "vec2<f32>(bitcast<f32>(a << 16u), bitcast<f32>(a & 0xffff0000u))".into(),
            ),
            Self::PackBf16 => (
                "a: vec2<f32>",
                "u32",
                "loft_round_bf16(a.x) | (loft_round_bf16(a.y) << 16u)".into(),
            ),
            Self::U4Lo => ("a: u32", "vec4<f32>", nibble_wgsl(0, false)),
            Self::U4Hi => ("a: u32", "vec4<f32>", nibble_wgsl(16, false)),
            Self::I4Lo => ("a: u32", "vec4<f32>", nibble_wgsl(0, true)),
            Self::I4Hi => ("a: u32", "vec4<f32>", nibble_wgsl(16, true)),
            Self::U2 => (
                "a: u32, i: u32",
                "f32",
                "f32(extractBits(a, min(i, 16u) * 2u, 2u))".into(),
            ),
            Self::Fp8E4M3 => ("a: u32", "vec4<f32>", Fp8::E4M3.wgsl_call()),
            Self::Fp8E5M2 => ("a: u32", "vec4<f32>", Fp8::E5M2.wgsl_call()),
        };
        let helper = match self {
            Self::PackBf16 => BF16_WGSL.to_owned(),
            Self::Fp8E4M3 => Fp8::E4M3.wgsl_decode(),
            Self::Fp8E5M2 => Fp8::E5M2.wgsl_decode(),
            Self::DotI8
            | Self::DotU8
            | Self::Bf16
            | Self::U4Lo
            | Self::U4Hi
            | Self::I4Lo
            | Self::I4Hi
            | Self::U2 => String::new(),
        };
        format!(
            "{helper}fn loft_quant_{}({params}) -> {ty} {{ return {body}; }}\n",
            self.name()
        )
    }
}

fn unpack_nibbles(word: u32, signed: bool) -> [f32; 4] {
    [0, 4, 8, 12].map(|offset| {
        let nibble = ((word >> offset) & 15) as i32;
        if signed {
            ((nibble << 28) >> 28) as f32
        } else {
            nibble as f32
        }
    })
}

fn nibble_wgsl(start: u32, signed: bool) -> String {
    let a = if signed { "bitcast<i32>(a)" } else { "a" };
    let lanes = [0, 4, 8, 12].map(|i| format!("f32(extractBits({a}, {}u, 4u))", start + i));
    format!("vec4<f32>({})", lanes.join(", "))
}

fn round_bf16(value: f32) -> u32 {
    let bits = value.to_bits();
    if value.is_nan() {
        return (bits >> 16) | 0x0040; // Keep a payload when the low half contained all its bits.
    }
    (bits + 0x7fff + ((bits >> 16) & 1)) >> 16
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

#[derive(Clone, Copy)]
enum Fp8 {
    E4M3,
    E5M2,
}

impl Fp8 {
    fn layout(self) -> (u32, u32, u32) {
        match self {
            Self::E4M3 => (3, 7, 0x7f),
            Self::E5M2 => (2, 15, 0x7c),
        }
    }

    fn unpack(self, bits: u8) -> f32 {
        let (mantissa, bias, first_special) = self.layout();
        let magnitude = u32::from(bits & 0x7f);
        let sign = u32::from(bits & 0x80) << 24;
        if magnitude >= first_special {
            let special = match self {
                Self::E5M2 if magnitude == first_special => f32::INFINITY,
                Self::E4M3 | Self::E5M2 => f32::NAN,
            };
            return f32::from_bits(sign | special.to_bits());
        }
        let exp = magnitude >> mantissa;
        let frac = magnitude & ((1 << mantissa) - 1);
        let significand = frac + if exp == 0 { 0 } else { 1 << mantissa };
        let scale = f32::from_bits((127 - bias - mantissa + exp.max(1)) << 23);
        f32::from_bits(sign | (scale * significand as f32).to_bits())
    }

    fn wgsl_decode(self) -> String {
        let (mantissa, bias, first_special) = self.layout();
        let special = match self {
            Self::E4M3 => "0x7fc00000u".to_owned(),
            Self::E5M2 => format!("select(0x7fc00000u, 0x7f800000u, mag == {first_special}u)"),
        };
        format!(
            "fn loft_fp8_m{mantissa}(bits: u32) -> f32 {{\n                let mag = bits & 127u;\n                let sign = (bits & 128u) << 24u;\n                if mag >= {first_special}u {{ return bitcast<f32>(sign | {special}); }}\n                let e = mag >> {mantissa}u;\n                let frac = mag & {mask}u;\n                let significand = frac + select({lead}u, 0u, e == 0u);\n                let scale = bitcast<f32>(({base}u + max(e, 1u)) << 23u);\n                return bitcast<f32>(sign | bitcast<u32>(f32(significand) * scale));\n            }}\n",
            mask = (1 << mantissa) - 1,
            lead = 1 << mantissa,
            base = 127 - bias - mantissa,
        )
    }

    fn wgsl_call(self) -> String {
        let (mantissa, _, _) = self.layout();
        let lanes = [0, 8, 16, 24].map(|i| format!("loft_fp8_m{mantissa}(a >> {i}u)"));
        format!("vec4<f32>({})", lanes.join(", "))
    }
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

pub(crate) fn wgsl_prelude(shader: &str, features: GpuFeatures) -> String {
    let mut out = String::new();
    for op in Quant::ALL {
        if shader.contains(&format!("loft_quant_{}(", op.name())) {
            out.push_str(&op.wgsl(features));
        }
    }
    out
}

/// Compile using the supplied host capabilities. The ordinary compiler
/// entry point targets the portable baseline.
pub fn compile(
    sources: &loft_ir::Sources,
    overrides: &[(String, f64)],
    caps: loft_ir::HostCaps,
) -> loft_ir::Res<(crate::program::Program, crate::Report, crate::Compiled)> {
    crate::compile_impl(
        sources,
        overrides,
        &[],
        std::path::Path::new("target/loft_cache"),
        caps.features,
    )
}
