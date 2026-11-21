#[derive(Clone, Copy)]
pub enum Quant {
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
    pub const ALL: [Self; 11] = [
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

    pub fn name(self) -> &'static str {
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

    pub fn resolve(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|op| op.name() == name)
    }
}

#[derive(Clone, Copy)]
pub enum Fp8 {
    E4M3,
    E5M2,
}

impl Fp8 {
    pub fn layout(self) -> (u32, u32, u32) {
        match self {
            Self::E4M3 => (3, 7, 0x7f),
            Self::E5M2 => (2, 15, 0x7c),
        }
    }

}
