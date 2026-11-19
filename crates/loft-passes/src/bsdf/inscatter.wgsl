// Joint coefficients per metre, summed across all overlapping media.
struct LoftCoefficients {
    extinction: vec3<f32>,
    source: vec3<f32>,
}
fn loft_inscatter_joint(o: vec3<f32>, d: vec3<f32>, begin: f32, end: f32, steps: u32, rng: vec4<u32>) -> LoftVol {
    var vol = LoftVol(vec3(0.0), vec3(1.0));
    if end <= begin {
        return vol;
    }
    let ds = (end - begin) / f32(steps);
    for (var k = 0u; k < steps; k++) {
        let medium = loft_sample_medium(o, d, begin + (f32(k) + 0.5) * ds, k, rng);
        vol = loft_volume_step(vol, medium.extinction, medium.source, ds);
    }
    return vol;
}
