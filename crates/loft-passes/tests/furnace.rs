//! Phase 4 reference observations are linear RGB before exposure or presentation.
type Rgb = [f32; 3];

fn check_rgb(actual: Rgb, expected: Rgb, tolerance: f32) {
    for (got, want) in actual.into_iter().zip(expected) {
        assert!((got - want).abs() <= tolerance, "{actual:?} vs {expected:?}, tolerance {tolerance}");
    }
}

// Unit wall emission E, diffuse albedo 0.5, no external illumination.
fn closed(mut measure: impl FnMut(u32, u32) -> Rgb) {
    for (depth, expected) in [(1, 1.50), (2, 1.75), (16, 2.00)] {
        check_rgb(measure(depth, 1024), [expected; 3], expected * 0.01);
    }
}

// The white roughness-one metal sphere under a uniform 0.5 sky. Centre and
// rim refer to the 32x32 orthographic fixture in examples/slice/furnace.ts.
fn white(mut measure: impl FnMut([u32; 2], u32) -> Rgb) {
    for pixel in [[16, 16], [29, 16]] {
        check_rgb(measure(pixel, 1024), [0.50; 3], 0.50 * 0.02);
    }
}

// Unit path over black. Both media have scalar extinction ln(2); each has
// unit source function in its own channel. Measure joint radiance AND transmittance.
fn coincident(mut integrate: impl FnMut([Rgb; 2], f32) -> (Rgb, Rgb)) {
    let red = [1., 0., 0.];
    let blue = [0., 0., 1.];
    for order in [[red, blue], [blue, red]] {
        let (l, t) = integrate(order, std::f32::consts::LN_2);
        check_rgb(l, [0.375, 0., 0.375], 1e-4);
        check_rgb(t, [0.25; 3], 1e-4);
    }
}

#[test]
fn analytic_reference_vectors() {
    closed(|depth, _| [2. * (1. - 0.5_f32.powi(1 + depth as i32)); 3]);
    white(|_, _| [0.5; 3]); // unit directional albedo
    coincident(|sources, sigma| {
        let t = (-2. * sigma).exp();
        let l = std::array::from_fn(|i| (sources[0][i] + sources[1][i]) * (1. - t) / 2.);
        (l, [t; 3])
    });
}

#[path = "reference/engines.rs"]
mod engines;
#[path = "reference/bsdf_media.rs"]
mod bsdf_media;
