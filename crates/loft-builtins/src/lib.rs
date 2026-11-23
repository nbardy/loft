//! Metadata for compiler-recognized pure calls, including compiler-generated
//! calls and names recognized only by motion or AD. Recognition in this catalog
//! alone does not make a call legal in a given stage.
//!
//! SDF and noise library functions are ordinary authored functions. Operators,
//! effectful calls, map stages and world queries have their own dispatch.

#![no_std]

/// An argument in a fixed signature.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ArgShape {
    /// f32, or an integer literal contextually converted to f32.
    Float,
    /// Requires an already typed f32.
    F32,
    /// Any numeric scalar, including a typed integer.
    Scalar,
    U32,
    Vec2,
    Vec3,
    /// Spatial value of any kind.
    Spatial,
    /// Named verdict reason.
    Reason,
    Bsdf,
}

/// A result whose shape is independent of the operands.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum RetShape {
    Bool,
    F32,
    I32,
    U32,
    Vec2,
    Vec3,
    Vec4,
    Verdict,
    Bsdf,
}

/// Rules that depend on operand types, spaces or a referenced declaration.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PolyRule {
    /// Numeric operands meet, with scalar/vector broadcasting and an f32
    /// result for all-abstract inputs. Spatial values are rejected.
    Componentwise,
    /// Float and vector inputs fill N lanes, with a one-scalar splat.
    VectorConstructor(u8),
    /// Componentwise, or two spatial values of one kind and space plus a scalar.
    Mix,
    /// Two arms of a common type and a Boolean condition.
    Select,
    /// f32 or a vector (raw or spatial) to f32.
    Length,
    /// Two raw vectors of equal width or two points in one space, to f32.
    Distance,
    /// Two raw vectors of equal width or vector-like values in one space, to f32.
    Dot,
    /// Like dot, returning a raw vec3 or a spatial vector in the shared space.
    Cross,
    /// Preserves raw width or a spatial normal, otherwise returns a direction.
    Normalize,
    /// Any spatial value to a direction in its space.
    Direction,
    /// Any spatial value to a displacement in its space.
    Displacement,
    /// Point plus optional float yaw, to an open-to-point-space transform.
    Place,
    /// Reverses a transform's source and destination spaces.
    Inverse,
    /// Point and three spatial axes sharing a space, to a frame in that space.
    FrameAxes,
    /// Type-directed zero. A spatial input produces a displacement and an
    /// abstract integer produces i32.
    Zero,
    /// Uses the named scan's frame type and space.
    ScanFrame,
    /// Uses the named texture declaration's coordinate and result types.
    TextureSample,
    /// Observed in motion classification or AD, with no checker signature.
    Unchecked,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SigRule {
    Fixed(&'static [ArgShape], RetShape),
    Poly(PolyRule),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum WgslLowering {
    /// Call the named WGSL intrinsic or emitted helper.
    Direct(&'static str),
    Prefix(&'static str),
    Infix(&'static str),
    /// Lowering needs type, declaration, stage, or argument information.
    Custom,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MotionClass {
    Native,
    /// Deterministic motion uses the fixed implementation.
    Fixed,
    /// Unavailable in deterministic motion.
    Unavailable,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct BuiltinSpec {
    pub id: Builtin,
    pub name: &'static str,
    pub min_arity: u8,
    pub max_arity: u8,
    pub sig: SigRule,
    pub wgsl: WgslLowering,
    pub motion_math: MotionClass,
    /// AD has an explicit derivative rule, including a zero derivative.
    pub has_ad: bool,
    /// The interval evaluator handles this call on its supported types.
    /// A rule can return an unbounded result. Exact-only numeric calls
    /// do not count as interval rules.
    pub has_lip: bool,
}

macro_rules! catalog {
    ($(
        $id:ident $name:literal $min:literal ..= $max:literal;
        $sig:expr; $wgsl:expr; $motion:ident; $ad:literal, $lip:literal;
    )+) => {
        /// Compiler-recognized call spelling. Aliases have distinct entries.
        #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
        pub enum Builtin { $($id,)+ }

        pub const BUILTINS: &[BuiltinSpec] = &[
            $(BuiltinSpec {
                id: Builtin::$id,
                name: $name,
                min_arity: $min,
                max_arity: $max,
                sig: $sig,
                wgsl: $wgsl,
                motion_math: MotionClass::$motion,
                has_ad: $ad,
                has_lip: $lip,
            },)+
        ];

        pub fn lookup(name: &str) -> Option<&'static BuiltinSpec> {
            match name {
                $($name => Some(spec(Builtin::$id)),)+
                _ => None,
            }
        }

        pub fn spec(id: Builtin) -> &'static BuiltinSpec {
            // Enum and table share the same order. Check every index at compile time.
            match id {
                $(Builtin::$id => const { &BUILTINS[Builtin::$id as usize] },)+
            }
        }
    };
}

use PolyRule::*;
use SigRule::{Fixed as Signature, Poly};
use WgslLowering::{Custom, Direct};

catalog! {
    // Componentwise math and scalar-only overloads.
    Abs "abs" 1..=1;
        Poly(Componentwise); Direct("abs"); Native; false, true;
    Acos "acos" 1..=1;
        Poly(Componentwise); Direct("acos"); Fixed; false, false;
    Atan2 "atan2" 2..=2;
        Signature(&[ArgShape::F32, ArgShape::F32], RetShape::F32);
        Direct("atan2"); Fixed; false, false;
    Clamp "clamp" 3..=3;
        Poly(Componentwise); Direct("clamp"); Native; false, true;
    Cos "cos" 1..=1;
        Poly(Componentwise); Direct("cos"); Fixed; true, true;
    Exp "exp" 1..=1;
        Poly(Componentwise); Direct("exp"); Fixed; true, true;
    Floor "floor" 1..=1;
        Poly(Componentwise); Direct("floor"); Native; false, true;
    Fma "fma" 3..=3;
        Signature(&[ArgShape::F32; 3], RetShape::F32); Direct("fma"); Native; false, false;
    Fract "fract" 1..=1;
        Poly(Componentwise); Direct("fract"); Native; false, true;
    Log "log" 1..=1;
        Poly(Componentwise); Direct("log"); Fixed; false, false;
    Max "max" 2..=2;
        Poly(Componentwise); Direct("max"); Native; false, true;
    Min "min" 2..=2;
        Poly(Componentwise); Direct("min"); Native; false, true;
    Pow "pow" 2..=2;
        Poly(Componentwise); Direct("pow"); Unavailable; true, true;
    Saturate "saturate" 1..=1;
        Poly(Componentwise); Direct("saturate"); Native; false, true;
    Sign "sign" 1..=1;
        Poly(Componentwise); Direct("sign"); Native; false, true;
    Sin "sin" 1..=1;
        Poly(Componentwise); Direct("sin"); Fixed; true, true;
    Smoothstep "smoothstep" 3..=3;
        Poly(Componentwise); Direct("smoothstep"); Native; true, true;
    Smootherstep "smootherstep" 3..=3;
        Signature(&[ArgShape::Float; 3], RetShape::F32);
        Direct("loft_smootherstep"); Native; true, true;
    DSmoothstep "loft_dsmoothstep" 3..=3;
        Signature(&[ArgShape::Float; 3], RetShape::F32);
        Direct("loft_dsmoothstep"); Native; false, true;
    DSmootherstep "loft_dsmootherstep" 3..=3;
        Signature(&[ArgShape::Float; 3], RetShape::F32);
        Direct("loft_dsmootherstep"); Native; false, true;
    Sqrt "sqrt" 1..=1;
        Poly(Componentwise); Direct("sqrt"); Native; true, true;
    Step "step" 2..=2;
        Poly(Componentwise); Direct("step"); Native; false, true;

    // Names encountered by motion/AD without a checker signature at this revision.
    Asin "asin" 1..=1;
        Poly(Unchecked); Direct("asin"); Unavailable; false, false;
    Atan "atan" 1..=1;
        Poly(Unchecked); Direct("atan"); Unavailable; false, false;
    Tan "tan" 1..=1;
        Poly(Unchecked); Direct("tan"); Unavailable; false, false;
    Exp2 "exp2" 1..=1;
        Poly(Unchecked); Direct("exp2"); Unavailable; false, false;
    Log2 "log2" 1..=1;
        Poly(Unchecked); Direct("log2"); Unavailable; false, false;
    Round "round" 1..=1;
        Poly(Unchecked); Direct("round"); Native; false, false;

    Mix "mix" 3..=3;
        Poly(Mix); Direct("mix"); Native; true, true;
    Select "select" 3..=3;
        Poly(Select); Direct("select"); Native; false, true;
    Conditional "loft_conditional" 3..=3;
        Poly(Select); Direct("select"); Native; false, true;
    F32 "f32" 1..=1;
        Signature(&[ArgShape::Scalar], RetShape::F32); Direct("f32"); Native; true, true;
    I32 "i32" 1..=1;
        Signature(&[ArgShape::Scalar], RetShape::I32); Direct("i32"); Native; true, false;
    U32 "u32" 1..=1;
        Signature(&[ArgShape::Scalar], RetShape::U32); Direct("u32"); Native; true, false;
    Vec2 "vec2" 1..=2;
        Poly(VectorConstructor(2)); Direct("vec2<f32>"); Native; true, true;
    Vec2f "vec2f" 1..=2;
        Poly(VectorConstructor(2)); Direct("vec2<f32>"); Native; true, true;
    Vec3 "vec3" 1..=3;
        Poly(VectorConstructor(3)); Direct("vec3<f32>"); Native; true, true;
    Vec3f "vec3f" 1..=3;
        Poly(VectorConstructor(3)); Direct("vec3<f32>"); Native; true, true;
    Vec4 "vec4" 1..=4;
        Poly(VectorConstructor(4)); Direct("vec4<f32>"); Native; true, true;
    Vec4f "vec4f" 1..=4;
        Poly(VectorConstructor(4)); Direct("vec4<f32>"); Native; true, true;

    // Geometry, preserving the checker's space rules.
    Length "length" 1..=1;
        Poly(Length); Direct("length"); Native; true, true;
    Distance "distance" 2..=2;
        Poly(Distance); Direct("distance"); Native; true, true;
    Dist "dist" 2..=2;
        Poly(Distance); Direct("distance"); Native; false, true;
    Dot "dot" 2..=2;
        Poly(Dot); Direct("dot"); Native; true, true;
    Cross "cross" 2..=2;
        Poly(Cross); Direct("cross"); Native; true, true;
    Normalize "normalize" 1..=1;
        Poly(Normalize); Direct("normalize"); Native; true, true;
    Dir "dir" 1..=1;
        Poly(Direction); Direct("normalize"); Native; false, true;
    Coords "coords" 1..=1;
        Signature(&[ArgShape::Spatial], RetShape::Vec3); Custom; Native; false, true;
    LoftVec "loft_vec" 1..=1;
        Poly(Displacement); Custom; Native; true, true;
    LoftZero "loft_zero" 1..=1;
        Poly(Zero); Custom; Native; true, true;
    Place "place" 1..=2;
        Poly(Place); Custom; Native; false, false;
    Inv "inv" 1..=1;
        Poly(Inverse); Direct("loft_xf_inv"); Native; false, true;
    FrameAxes "frame_axes" 4..=4;
        Poly(FrameAxes); Direct("LoftFrame"); Native; false, true;

    // Immutable stage inputs and verdict/material values.
    Sample "sample" 2..=2;
        Poly(TextureSample); Custom; Native; false, true;
    SampleLod "sample_lod" 3..=3;
        Poly(TextureSample); Custom; Native; false, true;
    SampleC2 "sample_c2" 2..=2;
        Poly(ScanFrame); Custom; Native; true, true;
    DSample "loft_dsample" 3..=3;
        Poly(ScanFrame); Custom; Native; false, true;
    HasSky "has_sky" 0..=0;
        Signature(&[], RetShape::Bool); Custom; Native; false, true;
    SkyZenith "sky_zenith" 0..=0;
        Signature(&[], RetShape::Vec3); Custom; Native; false, true;
    SkyHorizon "sky_horizon" 0..=0;
        Signature(&[], RetShape::Vec3); Custom; Native; false, true;
    SunColor "sun_color" 0..=0;
        Signature(&[], RetShape::Vec3); Custom; Native; false, true;
    Held "held" 1..=1;
        Signature(&[ArgShape::Float], RetShape::Verdict); Custom; Native; false, false;
    Missed "missed" 1..=1;
        Signature(&[ArgShape::Float], RetShape::Verdict); Custom; Native; false, false;
    Unknown "unknown" 1..=1;
        Signature(&[ArgShape::Reason], RetShape::Verdict); Custom; Native; false, false;
    EvalBsdf "eval_bsdf" 4..=4;
        Signature(&[ArgShape::Bsdf, ArgShape::Vec3, ArgShape::Vec3, ArgShape::Vec3], RetShape::Vec3);
        Custom; Native; false, false;
    MixBsdf "mix_bsdf" 3..=3;
        Signature(&[ArgShape::Bsdf, ArgShape::Bsdf, ArgShape::Float], RetShape::Bsdf);
        Custom; Native; false, false;
    GgxEnergy "ggx_energy" 2..=2;
        Signature(&[ArgShape::F32; 2], RetShape::Vec2);
        Direct("loft_ggx_energy"); Native; false, false;

    // Packed numeric calls use the generated WGSL helpers in quant.
    Dot4I8 "dot4_i8" 2..=2;
        Signature(&[ArgShape::U32; 2], RetShape::I32);
        Direct("loft_quant_dot4_i8"); Native; false, false;
    Dot4U8 "dot4_u8" 2..=2;
        Signature(&[ArgShape::U32; 2], RetShape::U32);
        Direct("loft_quant_dot4_u8"); Native; false, false;
    UnpackBf16x2 "unpack_bf16x2" 1..=1;
        Signature(&[ArgShape::U32], RetShape::Vec2);
        Direct("loft_quant_unpack_bf16x2"); Native; false, false;
    PackBf16x2 "pack_bf16x2" 1..=1;
        Signature(&[ArgShape::Vec2], RetShape::U32);
        Direct("loft_quant_pack_bf16x2"); Native; false, false;
    UnpackU4Lo "unpack_u4_lo" 1..=1;
        Signature(&[ArgShape::U32], RetShape::Vec4);
        Direct("loft_quant_unpack_u4_lo"); Native; false, false;
    UnpackU4Hi "unpack_u4_hi" 1..=1;
        Signature(&[ArgShape::U32], RetShape::Vec4);
        Direct("loft_quant_unpack_u4_hi"); Native; false, false;
    UnpackI4Lo "unpack_i4_lo" 1..=1;
        Signature(&[ArgShape::U32], RetShape::Vec4);
        Direct("loft_quant_unpack_i4_lo"); Native; false, false;
    UnpackI4Hi "unpack_i4_hi" 1..=1;
        Signature(&[ArgShape::U32], RetShape::Vec4);
        Direct("loft_quant_unpack_i4_hi"); Native; false, false;
    UnpackU2 "unpack_u2" 2..=2;
        Signature(&[ArgShape::U32; 2], RetShape::F32);
        Direct("loft_quant_unpack_u2"); Native; false, false;
    UnpackFp8E4m3 "unpack_fp8_e4m3" 1..=1;
        Signature(&[ArgShape::U32], RetShape::Vec4);
        Direct("loft_quant_unpack_fp8_e4m3"); Native; false, false;
    UnpackFp8E5m2 "unpack_fp8_e5m2" 1..=1;
        Signature(&[ArgShape::U32], RetShape::Vec4);
        Direct("loft_quant_unpack_fp8_e5m2"); Native; false, false;
}
