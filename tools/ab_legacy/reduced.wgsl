const ab_patches = array(
  vec3f(0.3996526002883911, 0.9166666865348816, 0.0),
  vec3f(-0.48772361874580383, 0.75, 0.44679486751556396),
  vec3f(0.07100989669561386, 0.5833333730697632, -0.8091228604316711),
  vec3f(0.5531073808670044, 0.4166666865348816, 0.7214298248291016),
  vec3f(-0.9534448385238647, 0.25, -0.16865061223506927),
  vec3f(0.8408204317092896, 0.08333331346511841, -0.5348613262176514),
  vec3f(-0.25870034098625183, -0.08333337306976318, 0.9623563289642334),
  vec3f(-0.4462720453739166, -0.25, -0.8592678904533386),
  vec3f(0.8538990616798401, -0.4166666269302368, 0.3118418753147125),
  vec3f(-0.7507836222648621, -0.5833333730697632, 0.3099130690097809),
  vec3f(0.2803475260734558, -0.75, -0.5990869998931885),
  vec3f(0.11960970610380173, -0.9166666269302368, 0.38133421540260315)
);

fn ab_dome(d: vec3f) -> vec3f {
  if d.y >= 0.0 {
    return mix(vec3f(0.48, 0.55, 0.64), vec3f(0.16, 0.29, 0.55), sqrt(d.y));
  }
  return mix(vec3f(0.24, 0.21, 0.16), vec3f(0.12, 0.10, 0.08), sqrt(-d.y));
}

fn ab_directional(mu: f32) -> vec2f {
  let v = sqrt(clamp(mu, 0.0, 1.0)) * 32.0;
  let j = min(u32(v), 31u);
  let b = v - f32(j);
  let e0 = mix(ab_energy[j], ab_energy[j+1u], b);
  let e1 = mix(ab_energy[34u+j], ab_energy[35u+j], b);
  return mix(vec2f(e0, ab_energy[33u]), vec2f(e1, ab_energy[67u]), 0.55 * 32.0 - 17.0) / 65535.0;
}

fn ab_fresnel(mu: f32) -> f32 {
  let x = 1.0 - clamp(mu, 0.0, 1.0);
  let x2 = x * x;
  return 0.04 + 0.96 * (x2 * x2 * x);
}

fn ab_eval(base: vec3f, n: vec3f, v: vec3f, l: vec3f, nv: f32, eo: vec2f, fv: f32) -> vec3f {
  let nl = dot(n, l);
  if nl <= 0.0 { return vec3f(0.0); }
  var spec = 0.0;
  if nv > 0.0 {
    let h = normalize(v + l);
    let nh = clamp(dot(n, h), 0.0, 1.0);
    let a2 = (0.55 * 0.55) * (0.55 * 0.55);
    let q = (1.0 - nh * nh) + nh * nh * a2;
    let gv = nl * sqrt(a2 + (1.0 - a2) * nv * nv);
    let gl = nv * sqrt(a2 + (1.0 - a2) * nl * nl);
    let dv = a2 / (6.283185307 * q * q * max(gv + gl, 1e-8));
    let ei = ab_directional(nl).x;
    let lost = 1.0 - eo.y;
    let favg = 1.8 / 21.0;
    let tint = favg * favg * eo.y / max(1.0 - favg * lost, 1e-6);
    let ms = tint * ((1.0 - eo.x) * (1.0 - ei) * 0.318309886 / max(lost, 1e-6));
    spec = (ab_fresnel(dot(v, h)) * dv + ms) * nl;
  }
  return base * (1.0 - fv) * (0.318309886 * nl) + spec;
}

fn ab_field(c: Capsule, p: vec3f) -> f32 {
  let along = clamp(dot(c.axis, p), -c.half_height, c.half_height);
  return length(p - c.axis * along) - c.radius;
}

fn shade(x: Point, wo: vec3f, ink: bool, px: vec2f) -> vec4f {
  let c = load_Capsule(rows[x.thing].refs.y);
  let p = x.p - origin(x.thing);
  let offsets = array(vec3f(1,-1,-1), vec3f(-1,-1,1), vec3f(-1,1,-1), vec3f(1,1,1));
  let eps = 5e-4 * max(1.0, 0.25 * x.t);
  var g = vec3f(0.0);
  var n = vec3f(0,1,0);
  var occ = 0.0;
  var w = 1.0;
  // Keep the same tetrahedral normal and five local AO taps as native shade_tiles.
  for (var i = 0u; i < 9u; i++) {
    var q = p + offsets[min(i,3u)] * eps;
    var h = 0.0;
    if i >= 4u {
      if i == 4u { n = normalize(g); }
      h = 0.02 + 0.06 * f32(i - 4u);
      q = p + n * h;
    }
    let d = ab_field(c, q);
    if i < 4u { g += offsets[i] * d; } else { occ += (h - d) * w; w *= 0.8; }
  }
  let ao = clamp(1.0 - 2.0 * occ, 0.0, 1.0);
  let base = albedo(material(x));
  let nv = max(dot(n, wo), 0.0);
  let eo = ab_directional(nv);
  let fv = ab_fresnel(nv);
  let sun = emitters[0];
  var result = vec3f(0.0);
  for (var i = 0u; i < 13u; i++) {
    var l = normalize(sun.pos);
    var e = sun.L;
    if i > 0u {
      l = ab_patches[i - 1u];
      e = ab_dome(l) * (0.85 * 1.0471976 * ao);
    }
    result += ab_eval(base, n, wo, l, nv, eo, fv) * e;
  }
  return vec4f(result, 1);
}
