//! Tier 0 tape words (06 §13), from high to low: op:6, dst:5, a:5, b:5, imm11:11.

macro_rules! opcodes {
    ($($op:ident = $id:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[repr(u8)]
        pub enum Tier0Opcode { $($op = $id),+ }

        impl Tier0Opcode {
            /// Decode a tape word, refusing the reserved opcode IDs 56..=63.
            pub fn unpack(word: u32) -> Option<(Self, u8, u8, u8, u16)> {
                let op = match word >> 26 {
                    $($id => Self::$op,)+
                    _ => return None,
                };
                let reg = |shift: u32| ((word >> shift) & 31) as u8;
                Some((op, reg(21), reg(16), reg(11), (word & 0x7ff) as u16))
            }
        }
    };
}

opcodes! {
    Nop = 0, LoadConst = 1, LoadRow = 2, LoadInput = 3, LoadFrame = 4, Mov = 5,
    Swizzle = 6, Splat = 7,
    Add = 8, Sub = 9, Mul = 10, Div = 11, Fma = 12, Neg = 13, Abs = 14,
    Min = 15, Max = 16, Clamp = 17, Mix = 18, Select = 19,
    Lt = 20, Le = 21, Eq = 22, Step = 23, Smoothstep = 24, Smootherstep = 25,
    Floor = 26, Fract = 27,
    Sqrt = 28, Sin = 29, Cos = 30, Atan2 = 31, Exp = 32, Log = 33, Pow = 34,
    Dot3 = 35, Cross = 36, LenNorm = 37,
    XformPt = 38, XformDir = 39, MirrorX = 40, RepeatCell = 41, SdSphere = 42,
    SdBox = 43, SdRoundBox = 44, SdCapsule = 45, SminPoly = 46, SsubPoly = 47,
    GradNoise3 = 48, SmoothVor3 = 49, Fbm2 = 50, SampleLutC2 = 51, SampleTex2d = 52,
    LoopBegin = 53, LoopEnd = 54, Ret = 55,
}

impl Tier0Opcode {
    /// Refuse operands that would overwrite an adjacent bit field.
    pub fn pack(self, dst: u8, a: u8, b: u8, imm11: u16) -> Option<u32> {
        if dst >= 32 || a >= 32 || b >= 32 || imm11 >= 2048 {
            return None;
        }
        Some(
            (self as u32) << 26
                | u32::from(dst) << 21
                | u32::from(a) << 16
                | u32::from(b) << 11
                | u32::from(imm11),
        )
    }
}
