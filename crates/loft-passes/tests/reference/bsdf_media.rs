use loft_passes::source::Graph;
use std::path::Path;
use super::{check_rgb, coincident};
use loft_ir::{Code, DepthConvention, GpuFeatures, HostCaps, Span};
use loft_passes::{
    Prog,
    bsdf::{Coefficients, INSCATTER_WGSL, inscatter_joint},
    eval::{Eval, Val},
    front, quant,
};
use std::num::NonZeroU32;

#[test]
fn coincident_media_twin() {
    for count in [1, 24, 64] {
        coincident(|sources, extinction| {
            let [a, b] = sources.map(|rgb| Coefficients {
                extinction: [extinction; 3],
                source: rgb.map(|v| v * extinction),
            });
            let vol = inscatter_joint(0., 1., NonZeroU32::new(count).unwrap(), |_| a + b);
            (vol.l, vol.t)
        });
    }
}

#[test]
fn analytic_dye_slab_twin() {
    let ln2 = std::f32::consts::LN_2;
    for count in [1, 24, 64] {
        let steps = NonZeroU32::new(count).unwrap();
        let vol = inscatter_joint(2., 3., steps, |t| {
            assert!((2. ..3.).contains(&t));
            Coefficients {
                extinction: [ln2, 2. * ln2, 0.],
                source: [0.; 3],
            }
        });
        check_rgb(vol.l, [0.; 3], 1e-4);
        check_rgb(vol.t, [0.5, 0.25, 1.], 1e-4);

        let thin = inscatter_joint(2., 3., steps, |_| Coefficients {
            extinction: [0., 1e-7, 0.],
            source: [0.125, 0.25, 0.5],
        });
        check_rgb(thin.l, [0.125, 0.25, 0.5], 1e-6);
        let miss = inscatter_joint(2., 1., steps, |_| panic!("empty interval sampled"));
        assert_eq!((miss.l, miss.t), ([0.; 3], [1.; 3]));
    }
}

#[test]
fn joint_media_wgsl_validates() {
    let wgsl = format!(
        "{INSCATTER_WGSL}
        fn loft_sample_medium(o: vec3<f32>, d: vec3<f32>, t: f32, step: u32, rng: vec4<u32>) -> LoftCoefficients {{
            return LoftCoefficients(abs(o + d * t), vec3(f32(step + rng.x)));
        }}
        @group(0) @binding(0) var<storage, read_write> pixel: array<vec3<f32>, 2>;
        @compute @workgroup_size(1) fn march() {{
            let result = loft_inscatter_joint(vec3(0.0), vec3(0.0, 0.0, 1.0), 0.0, 1.0, 24u, vec4(0u));
            pixel[0] = result.l;
            pixel[1] = result.t;
        }}"
    );
    let module = naga::front::wgsl::parse_str(&wgsl)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&wgsl)));
    validate(&module);
}

fn validate(module: &naga::Module) {
    use naga::valid::{Capabilities, ValidationFlags, Validator};
    Validator::new(ValidationFlags::all(), Capabilities::empty())
        .validate(module)
        .unwrap();
}

fn v(rgb: [f32; 3]) -> Val<f32> {
    Val::V(rgb.to_vec(), 0.)
}

fn lanes(value: Val<f32>) -> Vec<f32> {
    let Val::V(lanes, _) = value else {
        panic!("expected vector, got {value:?}")
    };
    lanes
}

#[test]
fn layered_bsdf_and_white_furnace_twin() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = Graph::text(&root, Path::new("furnace.loft"), "
        use stdlib::materials::bsdf::*
        fn white_metal() -> Bsdf = ggx(rgb(1), 1, 1)
        fn base() -> Bsdf = lambert([0.5, 0.25, 0])
        fn top() -> Bsdf = coat(1, 1)
        fn layered() -> Bsdf = lambert([0.5, 0.25, 0]) + coat(1, 1)
        fn reversed() -> Bsdf = coat(1, 1) + lambert([0.5, 0.25, 0])
        fn emission() -> Bsdf = emit_lobe([0.125, 0.25, 0.5]) + coat(1, 1)
    ",
    );
    let (modules, types) = front(&src).unwrap_or_else(|d| panic!("{}", d.render(&src)));
    let p = Prog::index(&modules).unwrap();
    let mut cpu = Eval::<f32>::new(&p, &types, vec![], &[]);
    let mut material = |name| cpu.call_fn(p.fns[name], vec![], Span::NONE).unwrap();
    let metal = material("white_metal");
    let base = material("base");
    let top = material("top");
    let layered = material("layered");
    let reversed = material("reversed");
    let emission = material("emission");
    let normal = v([0., 0., 1.]);
    let mut reflect = |b, wo: [f32; 3], wi: [f32; 3]| -> [f32; 3] {
        lanes(
            cpu.eval_bsdf(b, &normal, &v(wo), &v(wi), Span::NONE)
                .unwrap(),
        )
        .try_into()
        .unwrap()
    };
    for mu in [1.0_f32, 0.3] {
        let wo = [(1. - mu * mu).sqrt(), 0., mu];
        let mut energy = [0.; 3];
        // At roughness one and unit Fresnel the GGX integrand is independent of azimuth.
        for i in 0..1024 {
            let nl = (i as f32 + 0.5) / 1024.;
            let rgb = reflect(&metal, wo, [(1. - nl * nl).sqrt(), 0., nl]);
            for (out, value) in energy.iter_mut().zip(rgb) {
                *out += value * (std::f32::consts::PI / 1024.);
            }
        }
        check_rgb(energy, [0.5; 3], 0.50 * 0.02);
    }
    let direction = [0., 0., 1.];
    let base = reflect(&base, direction, direction);
    let coat = reflect(&top, direction, direction);
    let transfer = 1. - (1. - std::f32::consts::LN_2) * 0.04;
    check_rgb(
        reflect(&layered, direction, direction),
        std::array::from_fn(|i| coat[i] + base[i] * transfer),
        1e-5,
    );
    check_rgb(
        reflect(&reversed, direction, direction),
        std::array::from_fn(|i| coat[i] + base[i]),
        1e-6,
    );
    check_rgb(
        reflect(&emission, direction, [0.; 3]),
        [0.125, 0.25, 0.5],
        1e-6,
    );
}

fn quant_sources() -> Graph {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let scene = include_str!("../../../../stdlib/engines/capsules.loft").replace(
        "ggx(albedo, 0.55, 0)",
        "
        ggx(albedo, 0.55, 0) + lambert(demo(0x7f00ff80, 0x7f80fd02) * 1e-6)",
    );
    let quant = "
use stdlib::materials::bsdf::{lambert}
        fn signed(a: u, b: u) -> i = dot4_i8(a, b)
        fn unsigned(a: u, b: u) -> u = dot4_u8(a, b)
        fn bf(a: u) -> v2 = unpack_bf16x2(a)
        fn pack_bf(a: v2) -> u = pack_bf16x2(a)
        fn u4lo(a: u) -> v4 = unpack_u4_lo(a)
        fn u4hi(a: u) -> v4 = unpack_u4_hi(a)
        fn i4lo(a: u) -> v4 = unpack_i4_lo(a)
        fn i4hi(a: u) -> v4 = unpack_i4_hi(a)
        fn u2lane(a: u, k: u) -> f = unpack_u2(a, k)
        fn e4(a: u) -> v4 = unpack_fp8_e4m3(a)
        fn e5(a: u) -> v4 = unpack_fp8_e5m2(a)
        fn demo(a: u, b: u) -> rgb {
            let dot_sum = f32(signed(a,b)) + f32(unsigned(a,b))
            let bf_val = bf(pack_bf(bf(a)))
            let q = u4lo(a) + u4hi(a) + i4lo(a) + i4hi(a)
            let e = e4(b & 0x7e7e7e7e) + e5(b & 0x7b7b7b7b)
            [dot_sum, bf_val.x + q.x, u2lane(a,1) + e.y]
        }
    ";
    Graph::text(&root, Path::new("quant.loft"), &format!("{scene}\n{quant}"))
}

#[test]
fn quant_packed_and_portable_wgsl_and_twin() {
    let src = quant_sources();
    for features in [GpuFeatures::empty(), GpuFeatures::PACKED_INT8_DOT] {
        let caps = HostCaps {
            depth_convention: DepthConvention::StandardZeroToOne,
            msaa_samples: 4,
            features,
        };
        let (program, _, compiled) = quant::compile(&src, &[("count".into(), 1.)], caps)
            .unwrap_or_else(|d| panic!("{}", d.render(&src)));
        let shader = &program.modules[0];
        for module in &program.modules {
            validate(&module.naga);
        }
        let contains_dot = features.contains(GpuFeatures::PACKED_INT8_DOT);
        assert_eq!(shader.wgsl.contains("dot4I8Packed(a, b)"), contains_dot);
        assert_eq!(shader.wgsl.contains("dot4U8Packed(a, b)"), contains_dot);
        assert_eq!(
            shader
                .wgsl
                .contains("extractBits(bitcast<i32>(a), 24u, 8u)"),
            !contains_dot
        );
        assert_eq!(
            shader.wgsl.contains("extractBits(a, 24u, 8u)"),
            !contains_dot
        );
        assert!(shader.wgsl.contains("loft_ggx_energy("));

        let p = compiled.prog();
        let mut twin = Eval::<f32>::new(&p, &compiled.types, vec![], &[]);
        let mut call = |name, args| twin.call_fn(p.fns[name], args, Span::NONE).unwrap();
        let words = || vec![Val::U(0x7f00ff80), Val::U(0x7f80fd02)];
        assert!(matches!(call("signed", words()), Val::I(15876)));
        assert!(matches!(call("unsigned", words()), Val::U(80900)));
        assert!(matches!(
            call("signed", vec![Val::U(u32::MAX), Val::U(0x01010101)]),
            Val::I(-4)
        ));
        assert!(matches!(
            call("unsigned", vec![Val::U(u32::MAX), Val::U(u32::MAX)]),
            Val::U(260100)
        ));
        assert_eq!(lanes(call("bf", vec![Val::U(0xc0203f80)])), [1., -2.5]);
        let zeros = lanes(call("bf", vec![Val::U(0x80000000)]));
        assert_eq!(
            zeros.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
            [0, 0x80000000]
        );
        assert!(matches!(
            call(
                "pack_bf",
                vec![Val::V(
                    vec![f32::from_bits(0x3f808000), f32::from_bits(0x3f818000)],
                    0.
                )]
            ),
            Val::U(0x3f823f80)
        ));
        assert_eq!(
            lanes(call("u4lo", vec![Val::U(0xfedcba98)])),
            [8., 9., 10., 11.]
        );
        assert_eq!(
            lanes(call("u4hi", vec![Val::U(0xfedcba98)])),
            [12., 13., 14., 15.]
        );
        assert_eq!(
            lanes(call("i4lo", vec![Val::U(0xfedcba98)])),
            [-8., -7., -6., -5.]
        );
        assert_eq!(
            lanes(call("i4hi", vec![Val::U(0xfedcba98)])),
            [-4., -3., -2., -1.]
        );
        assert!(matches!(
            call("u2lane", vec![Val::U(0xc0000000), Val::U(15)]),
            Val::S(3.)
        ));
        let e4 = lanes(call("e4", vec![Val::U(0x7f7e0138)]));
        assert_eq!(&e4[..3], &[1., 2_f32.powi(-9), 448.]);
        assert!(e4[3].is_nan());
        let e5 = lanes(call("e5", vec![Val::U(0x7d7c013c)]));
        assert_eq!(&e5[..3], &[1., 2_f32.powi(-16), f32::INFINITY]);
        assert!(e5[3].is_nan());
    }
}

#[test]
fn quant_rejects_typed_signed_word() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = Graph::text(&root, Path::new("bad.loft"), "fn bad(a: i) -> i = dot4_i8(a, 1)");
    assert_eq!(front(&src).unwrap_err().code, Code::E003);
}

#[test]
fn joint_media_surface_types() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = Graph::text(&root, Path::new("march.loft"), "
        use stdlib::lighting::world::*
        space World Local
        fn bounded(w: World, r: Ray, limit: m, rng: RngKey) -> Vol = inscatter_joint(w, r, limit, 64, rng)
        fn default_steps(w: World, r: Ray, limit: m, rng: RngKey) -> Vol = inscatter_joint(w, r, limit, rng)
    ");
    front(&src).unwrap_or_else(|d| panic!("{}", d.render(&src)));
}
