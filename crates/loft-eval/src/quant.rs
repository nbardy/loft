//! std::quant, over the ordinary u32 words of the frozen storage ABI.
use crate::eval::{Scalar, Val};
use loft_check::quant::{Fp8, Quant};
use loft_ir::{Code, Diag, Res, Span};

pub(crate) fn twin<S: Scalar>(op: Quant, words: &[Val<S>], span: Span) -> Res<Val<S>> {
    let word = |i: usize| {
        let Val::U(value) = words[i] else {
            unreachable!("checked packed word")
        };
        value
    };
    let vector =
        |lanes: &[f32]| Val::V(lanes.iter().map(|&x| S::c(f64::from(x))).collect(), 0.0);
    Ok(match op {
        Quant::DotI8 => Val::I(
            word(0)
                .to_le_bytes()
                .into_iter()
                .zip(word(1).to_le_bytes())
                .map(|(a, b)| i32::from(a as i8) * i32::from(b as i8))
                .sum(),
        ),
        Quant::DotU8 => Val::U(
            word(0)
                .to_le_bytes()
                .into_iter()
                .zip(word(1).to_le_bytes())
                .map(|(a, b)| u32::from(a) * u32::from(b))
                .sum(),
        ),
        Quant::Bf16 => {
            let a = word(0);
            vector(&[f32::from_bits(a << 16), f32::from_bits(a & 0xffff_0000)])
        }
        Quant::PackBf16 => {
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
        Quant::U4Lo => vector(&unpack_nibbles(word(0), false)),
        Quant::U4Hi => vector(&unpack_nibbles(word(0) >> 16, false)),
        Quant::I4Lo => vector(&unpack_nibbles(word(0), true)),
        Quant::I4Hi => vector(&unpack_nibbles(word(0) >> 16, true)),
        // WGSL extractBits returns zero for an empty extraction at offset 32.
        Quant::U2 => Val::S(S::c(f64::from(
            word(0).checked_shr(word(1).min(16) * 2).unwrap_or(0) & 3,
        ))),
        Quant::Fp8E4M3 => vector(&word(0).to_le_bytes().map(|lane| unpack(Fp8::E4M3, lane))),
        Quant::Fp8E5M2 => vector(&word(0).to_le_bytes().map(|lane| unpack(Fp8::E5M2, lane))),
    })
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

fn round_bf16(value: f32) -> u32 {
    let bits = value.to_bits();
    if value.is_nan() {
        return (bits >> 16) | 0x0040; // Keep a payload when the low half contained all its bits.
    }
    (bits + 0x7fff + ((bits >> 16) & 1)) >> 16
}

fn unpack(format: Fp8, bits: u8) -> f32 {
    let (mantissa, bias, first_special) = format.layout();
    let magnitude = u32::from(bits & 0x7f);
    let sign = u32::from(bits & 0x80) << 24;
    if magnitude >= first_special {
        let special = match format {
            Fp8::E5M2 if magnitude == first_special => f32::INFINITY,
            Fp8::E4M3 | Fp8::E5M2 => f32::NAN,
        };
        return f32::from_bits(sign | special.to_bits());
    }
    let exp = magnitude >> mantissa;
    let frac = magnitude & ((1 << mantissa) - 1);
    let significand = frac + if exp == 0 { 0 } else { 1 << mantissa };
    let scale = f32::from_bits((127 - bias - mantissa + exp.max(1)) << 23);
    f32::from_bits(sign | (scale * significand as f32).to_bits())
}
