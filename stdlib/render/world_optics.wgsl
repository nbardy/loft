
struct Optics { base: f32, dispersion: f32, absorb: vec3<f32> }
fn world_optics(h: PathHit) -> Optics {
  switch h.kind { {{OPTICS}} default: { return Optics(0.0, 0.0, vec3<f32>(0.0)); } }
}
