// Joint coefficients per metre, summed across all overlapping media.
struct LoftCoefficients {
    extinction: vec3<f32>,
    source: vec3<f32>,
}
struct LoftVol {
    l: vec3<f32>,
    t: vec3<f32>,
}

fn loft_inscatter_joint(o: vec3<f32>, d: vec3<f32>, begin: f32, end: f32, steps: u32, rng: vec4<u32>) -> LoftVol {
    var vol = LoftVol(vec3(0.0), vec3(1.0));
    if end <= begin {
        return vol;
    }
    let ds = (end - begin) / f32(steps);
    for (var k = 0u; k < steps; k++) {
        let medium = loft_sample_medium(o, d, begin + (f32(k) + 0.5) * ds, k, rng);
        let tau = medium.extinction * ds;
        let tr = exp(-tau);
        let thin = tau < vec3(0.001);
        let thin_weight = ds * (1.0 - tau * (0.5 - tau / 6.0));
        let thick_weight = (1.0 - tr) / select(medium.extinction, vec3(1.0), thin);
        let weight = select(thick_weight, thin_weight, thin);
        vol.l += vol.t * medium.source * weight;
        vol.t *= tr;
    }
    return vol;
}
