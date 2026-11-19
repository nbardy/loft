struct LoftVol {
    l: vec3<f32>,
    t: vec3<f32>,
}

fn loft_volume_step(vol: LoftVol, extinction: vec3<f32>, source: vec3<f32>, ds: f32) -> LoftVol {
    let tau = extinction * ds;
    let tr = exp(-tau);
    let thin = tau < vec3(0.001);
    let thin_weight = ds * (1.0 - tau * (0.5 - tau / 6.0));
    let thick_weight = (1.0 - tr) / select(extinction, vec3(1.0), thin);
    return LoftVol(vol.l + vol.t * source * select(thick_weight, thin_weight, thin), vol.t * tr);
}
