# Task 1

```
space World, Body, Leg, Claw, Face, Shell;

fn spine(u: f32) -> Pt[Shell] {
  let a = 14.0rad * u;
  let r = 0.02m * exp(1.6 * u);
  return Pt[Shell](r * cos(a), 0.05m * u * 8.0, r * sin(a));
}

scan whorl(u: f32 in 0.0..1.0, steps: 256u) @schedule(bake_lut) -> Table[Frame[Shell]] {
  init: frame_from_tangent(spine(u), normalize(diff(spine, u)(u)), Dir[Shell](0.0, 1.0, 0.0));
  step(prev, u_i): transport_frame(prev, spine(u_i), normalize(diff(spine, u)(u_i)));
}

kind ShellKind {
  row { pose: Xform[Shell -> World] }
  @grid(256u, 48u) query surface(self, u: f32, v: f32) -> Pt[Shell] {
    let f = sample_c2(whorl, u);
    let th = 6.2831853rad * v;
    let rad = 0.012m * exp(1.6 * u) * (1.0 + 0.12 * sin(24.0 * th));
    return f.origin + f.normal * (rad * cos(th)) + f.binormal * (rad * sin(th));
  }
  query material(self, p: Pt[Shell], n: Norm[Shell]) -> Bsdf {
    return ggx(vec3(0.85, 0.72, 0.6), 0.45, 0.0) + clearcoat(0.3, 0.2);
  }
}

kind Claw {
  row { pose: Xform[Claw -> World], open: Rad, size: Meters, palm: Meters }
  query trace(self, p: Pt[Claw]) -> Field[Claw] {
    let palm = sd_round_box(p, Vec[Claw](self.palm, self.palm * 0.6, self.palm * 0.5), 0.2 * self.palm);
    let fixed = sd_capsule(p, Pt[Claw](self.palm, 0.0, 0.0), Pt[Claw](self.palm + self.size, 0.0, 0.0), 0.3 * self.palm);
    let c = cos(self.open);
    let s = sin(self.open);
    let tip = Pt[Claw](self.palm + self.size * c, self.size * s, 0.0);
    var mv = sd_capsule(p, Pt[Claw](self.palm, 0.0, 0.0), tip, 0.28 * self.palm);
    let along = dot(p - Pt[Claw](self.palm, 0.0, 0.0), Vec[Claw](c, s, 0.0));
    let teeth = 0.02m * self.palm * abs(sin(along * 60.0));
    mv = mv + teeth * 0.5;
    return smin_poly(smin_poly(palm, fixed, 0.03m), mv, 0.03m);
  }
  query material(self, p: Pt[Claw], n: Norm[Claw]) -> Bsdf { return chitin(p); }
}

kind CrabBody {
  row { pose: Xform[Body -> World], com: Pt[World], vel: Vec[World], head: Pt[Body], gaze: Pt[World] }
  query trace(self, p: Pt[Body]) -> Field[Body] {
    return smin_poly(sd_round_box(p, Vec[Body](0.35, 0.12, 0.25), 0.05m),
                     sd_sphere(p - Vec[Body](0.3, 0.05, 0.0), 0.14m), 0.05m);
  }
  query material(self, p: Pt[Body], n: Norm[Body]) -> Bsdf { return chitin(p); }
}

kind CrabLeg {
  row { parent: RowId[CrabBody], pose: Xform[Leg -> World], femur: Meters, tibia: Meters, foot: Pt[World] }
  query trace(self, p: Pt[Leg]) -> Field[Leg] {
    let k = Pt[Leg](self.femur, 0.1m, 0.0);
    let a = sd_capsule(p, Pt[Leg](0.0, 0.0, 0.0), k, 0.035m);
    let b = sd_capsule(p, k, Pt[Leg](self.femur, -self.tibia, 0.0), 0.022m);
    return smin_poly(a, b, 0.02m);
  }
  query material(self, p: Pt[Leg], n: Norm[Leg]) -> Bsdf { return chitin(p); }
}

kind EyeStalk {
  row { parent: RowId[CrabBody], pose: Xform[Face -> World], gaze: Pt[World] }
  query trace(self, p: Pt[Face]) -> Field[Face] {
    map sym = mirror_x(p) |> translate(-Vec[Face](0.15, 0.72, 0.42));
    let eye_c = Pt['sym](0.03, 0.30, 0.07);
    let stalk = sd_capsule(sym - eye_c, Pt['sym](0.0, -0.2, 0.0), Pt['sym](0.0, 0.0, 0.0), 0.02m);
    let globe = sd_sphere(sym - eye_c, 0.085m);
    let eye_w = self.pose * (sym.to_parent * eye_c);
    let look = sym.from_parent * (inv(self.pose) * normalize(self.gaze - eye_w));
    let pupil = sd_sphere(sym - (eye_c + look * 0.08m), 0.035m);
    return min(smin_poly(stalk, globe, 0.02m), pupil);
  }
  query material(self, p: Pt[Face], n: Norm[Face]) -> Bsdf {
    return ggx(vec3(0.05, 0.04, 0.04), 0.1, 0.0) + clearcoat(0.9, 0.03);
  }
}

fn chitin(p: Pt[Leg]) -> Bsdf {
  let rough = 0.30 + 0.15 * fbm2(vec2(p.x, p.z) * 90.0);
  return ggx(vec3(0.85, 0.28, 0.1), rough, 0.0) + sss(vec3(0.9, 0.3, 0.1), 0.08) + clearcoat(0.7, 0.07);
}

kind Sand {
  row { pose: Xform[World -> World] }
  query trace(self, p: Pt[World]) -> Field[World] { return p.y + 0.004m * fbm2(vec2(p.x, p.z) * 30.0); }
  query material(self, p: Pt[World], n: Norm[World]) -> Bsdf {
    return ggx(vec3(0.35, 0.28, 0.2), 0.25, 0.0) + clearcoat(0.5, 0.05);
  }
}

kind Sun {
  row { dir: Dir[World], color: vec3<f32> }
  query light(self, p: Pt[World]) -> vec3<f32> { return self.color; }
  query sky(self, rd: Dir[World]) -> vec3<f32> { return mix(vec3(0.7, 0.8, 0.9), vec3(0.3, 0.5, 0.8), rd.y); }
}

comptime fn leg_socket(j: u32, side: f32) -> Xform[Leg -> Body] {
  return xform_translate(Vec[Body](0.12 * f32(j) - 0.25, 0.0, side * 0.22)) * xform_rot_y(side * 1.2rad);
}

comptime fn crab(origin: Pt[World], yaw: Rad) {
  let body = xform_translate(origin - Pt[World](0.0, 0.0, 0.0)) * xform_rot_y(yaw);
  let b = emit CrabBody { pose: body, com: origin, vel: Vec[World](0.0, 0.0, 0.0),
                          head: Pt[Body](0.4, 0.1, 0.0), gaze: Pt[World](1.0, 0.4, 1.0) };
  comptime for j in 0u..6u {
    comptime for side in [-1.0, 1.0] {
      emit CrabLeg { parent: b, pose: body * leg_socket(j, side), femur: 0.14m + 0.01m * f32(j),
                     tibia: 0.18m - 0.01m * f32(j), foot: body * Pt[Body](0.12 * f32(j) - 0.25, 0.0, side * 0.5) };
    }
  }
  emit Claw { pose: body * xform_translate(Vec[Body](0.4, 0.0, -0.25)), open: 0.5rad, size: 0.22m, palm: 0.1m };
  emit Claw { pose: body * xform_translate(Vec[Body](0.4, 0.0, 0.25)), open: 0.2rad, size: 0.1m, palm: 0.05m };
  emit EyeStalk { parent: b, pose: body * xform_translate(Vec[Body](0.4, 0.1, 0.0)), gaze: Pt[World](1.0, 0.4, 1.0) };
  emit ShellKind { pose: body * xform_translate(Vec[Body](-0.15, 0.15, 0.0)) };
}

comptime fn scene() {
  emit Sand { pose: xform_identity() };
  emit Sun { dir: normalize(Dir[World](-0.4, 0.8, 0.3)), color: vec3(1.0, 0.95, 0.85) };
  crab(Pt[World](0.0, 0.0, 0.0), 0.0rad);
}

law gaze_drift(e: EyeStalk, t: Sec) writes EyeStalk {
  e.gaze = Pt[World](1.0 + 0.5 * sin(0.2 * f32(t)), 0.4, 1.0 + 0.5 * cos(0.15 * f32(t)));
}

schedule Main {
  pass rows: scene();
  pass motion: run_laws(gaze_drift);
  pass camera: orbit(at: Pt[World](0.0, 0.15, 0.0), radius: 1.2m, fov: 40deg);
  pass cull: build_indirect(Sand, ShellKind, Claw, CrabBody, CrabLeg, EyeStalk);
  pass vis: render_bundle(Vis64) { draw_indirect(Sand, ShellKind, Claw, CrabBody, CrabLeg, EyeStalk); };
  pass classify: tile_bin_kinds(Vis64, tile: 8u);
  pass shade: dispatch_material_tiles(Vis64, light: sun_sky);
  pass present: tonemap_aces(ColorHdr, sky: sky_color);
}
```

# Task 2

```
space World, Reed;

kind Reed {
  row { pose: Xform[Reed -> World], height: Meters, lean: Rad, stiff: f32, phase: Rad }
  query trace(self, p: Pt[Reed], time: Sec) -> Field[Reed] {
    let h = clamp(p.y / self.height, 0.0, 1.0);
    let gust = sin(f32(time) * 1.7 - 0.8 * (self.pose * p).x + self.phase);
    let bend = (self.lean + 0.5rad * gust / self.stiff) * h * h;
    let q = Pt[Reed](p.x - bend * p.y, p.y, p.z);
    let r = 0.012m * (1.0 - 0.6 * h);
    let stalk = sd_capsule(q, Pt[Reed](0.0, 0.0, 0.0), Pt[Reed](0.0, self.height, 0.0), r);
    let head = sd_capsule(q, Pt[Reed](0.0, self.height - 0.12m, 0.0), Pt[Reed](0.0, self.height, 0.0), 0.025m);
    return min(stalk, head);
  }
  query material(self, p: Pt[Reed], n: Norm[Reed]) -> Bsdf { return ink(vec3(0.15, 0.1, 0.08), 0.0); }
}

@lobe fn ink(dark: vec3<f32>, cut: f32, n: vec3<f32>, v: vec3<f32>, l: vec3<f32>) -> vec4<f32> {
  return vec4(select(vec3(0.92, 0.85, 0.7), dark, dot(n, l) < cut), 0.0);
}

kind Water {
  row { level: Meters }
  query trace(self, p: Pt[World]) -> Field[World] { return p.y - self.level; }
  query material(self, p: Pt[World], n: Norm[World]) -> Bsdf {
    return ink(vec3(0.92, 0.85, 0.7) * 0.9, -2.0) + emission(vec3(0.02, 0.03, 0.04) * fbm2(vec2(p.x, p.z) * 4.0));
  }
}

kind Sun {
  row { dir: Dir[World] }
  query light(self, p: Pt[World]) -> vec3<f32> { return vec3(1.0, 1.0, 1.0); }
  query sky(self, rd: Dir[World]) -> vec3<f32> { return vec3(0.92, 0.85, 0.7) - 0.04 * fbm2(vec2(rd.x, rd.z) * 3.0); }
}

fn hash1(i: u32) -> f32 {
  raw wgsl (i: u32) -> f32 { return fract(sin(f32(i) * 12.9898) * 43758.5453); }
}

comptime fn bed() {
  comptime for i in 0u..200u {
    let a = 6.2831853 * hash1(i * 3u);
    let r = 1.5 * sqrt(hash1(i * 3u + 1u)) * (0.6 + 0.8 * hash1(i * 3u + 2u));
    emit Reed { pose: xform_translate(Vec[World](r * cos(a), 0.0, r * sin(a))),
                height: 1.1m + 0.5m * hash1(i * 7u), lean: 0.1rad * (hash1(i * 11u) - 0.5),
                stiff: 0.6 + 0.8 * hash1(i * 13u), phase: 6.2831853rad * hash1(i * 17u) };
  }
  emit Water { level: 0.0m };
  emit Sun { dir: normalize(Dir[World](-0.5, 0.8, 0.3)) };
}

schedule Main {
  pass rows: bed();
  pass camera: orbit(at: Pt[World](0.0, 0.7, 0.0), radius: 4m, fov: 35deg);
  pass cull: build_indirect(Water, Reed);
  pass vis: render_bundle(Vis64) { draw_indirect(Water, Reed); };
  pass classify: tile_bin_kinds(Vis64, tile: 8u);
  pass shade: dispatch_material_tiles(Vis64, light: sun_sky);
  pass present: tonemap_aces(ColorHdr, sky: sky_color);
}
```

# Task 3

```
space World, Body, Neck, Head;

kind Heron {
  row { pose: Xform[Body -> World], hips: Pt[Body], head_pos: Pt[World], yaw: Rad, neck_yaw: Rad,
        limit: Rad, hop_t: Sec, hopping: bool, vel: Vec[World], gaze: Pt[World], head: Xform[Head -> World] }
  query trace(self, p: Pt[Body]) -> Field[Body] {
    let trunk = sd_capsule(p, Pt[Body](-0.15, 0.55, 0.0), Pt[Body](0.1, 0.6, 0.0), 0.11m);
    let legs = min(sd_capsule(p, Pt[Body](0.0, 0.0, 0.05), Pt[Body](0.0, 0.5, 0.05), 0.012m),
                   sd_capsule(p, Pt[Body](0.0, 0.0, -0.05), Pt[Body](0.0, 0.5, -0.05), 0.012m));
    let neck = min(sd_capsule(p, Pt[Body](0.1, 0.6, 0.0), Pt[Body](0.2, 0.75, 0.0), 0.03m),
                   min(sd_capsule(p, Pt[Body](0.2, 0.75, 0.0), Pt[Body](0.15, 0.9, 0.0), 0.025m),
                       sd_capsule(p, Pt[Body](0.15, 0.9, 0.0), Pt[Body](0.25, 1.0, 0.0), 0.025m)));
    let head = sd_sphere(p - Vec[Body](0.28, 1.02, 0.0), 0.04m);
    let beak = sd_capsule(p, Pt[Body](0.3, 1.02, 0.0), Pt[Body](0.45, 1.02, 0.0), 0.012m);
    return min(min(smin_poly(trunk, legs, 0.03m), neck), min(head, beak));
  }
  query material(self, p: Pt[Body], n: Norm[Body]) -> Bsdf { return ggx(vec3(0.55, 0.6, 0.65), 0.6, 0.0); }
}

kind Fish {
  row { pos: Pt[World] }
  query trace(self, p: Pt[World]) -> Field[World] { return sd_sphere(p - (self.pos - Pt[World](0.0, 0.0, 0.0)), 0.05m); }
  query material(self, p: Pt[World], n: Norm[World]) -> Bsdf { return ggx(vec3(0.8, 0.8, 0.85), 0.2, 1.0); }
}

kind Ground {
  row { level: Meters }
  query trace(self, p: Pt[World]) -> Field[World] { return p.y - self.level; }
  query material(self, p: Pt[World], n: Norm[World]) -> Bsdf { return ggx(vec3(0.3, 0.35, 0.3), 0.5, 0.0); }
}

kind Sun {
  row { dir: Dir[World] }
  query light(self, p: Pt[World]) -> vec3<f32> { return vec3(1.0, 0.95, 0.9); }
  query sky(self, rd: Dir[World]) -> vec3<f32> { return vec3(0.6, 0.75, 0.9); }
}

fn fish_curve(t: Sec) -> Pt[World] {
  return Pt[World](1.5 * cos(0.4 * f32(t)) , 0.1, 1.0 + 1.5 * sin(0.4 * f32(t)));
}

fn wrap_pi(a: Rad) -> Rad {
  return atan2(sin(a), cos(a));
}

law fish_swim(f: Fish, h: Table[Heron], t: Sec) writes Fish {
  f.pos = fish_curve(t);
}

law heron_gaze(h: Heron, dt: Sec, fish: Table[Fish]) writes Heron {
  let target = fish[0u].pos;
  h.gaze = target;
  let to = target - h.pose * Pt[Body](0.0, 0.0, 0.0);
  let want = wrap_pi(atan2(to.z, to.x) - h.yaw);
  h.neck_yaw = clamp(want, -h.limit, h.limit);
}

law heron_hop(h: Heron, dt: Sec, fish: Table[Fish]) writes Heron {
  let to = h.gaze - h.pose * Pt[Body](0.0, 0.0, 0.0);
  let want = wrap_pi(atan2(to.z, to.x) - h.yaw);
  if abs(want) > h.limit && !h.hopping { h.hopping = true; h.hop_t = 0.0s; }
  if h.hopping {
    h.hop_t = h.hop_t + dt;
    let u = h.hop_t / 0.4s;
    let y = 0.12m * 4.0 * u * (1.0 - u);
    h.vel = Vec[World](0.0, y, 0.0);
    h.yaw = h.yaw + wrap_pi(want - h.limit * 0.5) * clamp(f32(dt) / 0.4, 0.0, 1.0);
    if u >= 1.0 { h.hopping = false; }
  }
  h.pose = xform_translate(h.vel) * xform_rot_y(h.yaw);
  h.head = h.pose * xform_rot_y(h.neck_yaw) * xform_translate(Vec[Body](0.28, 1.02, 0.0));
  h.head_pos = h.head * Pt[Head](0.0, 0.0, 0.0);
}

constraint gaze_locked(h: Heron) -> Verdict {
  if !is_finite(h.pose) { return Verdict::Unknown(NonFinitePose); }
  let to = h.gaze - h.pose * Pt[Body](0.0, 0.0, 0.0);
  let want = wrap_pi(atan2(to.z, to.x) - h.yaw);
  let over = abs(want) - h.limit;
  if over > 0.0rad { return Verdict::Missed(over); }
  return Verdict::Held(-over);
}

constraint head_above_hips(h: Heron) -> Verdict {
  if !is_finite(h.pose) { return Verdict::Unknown(NonFinitePose); }
  let margin = h.head_pos.y - (h.pose * h.hips).y;
  if margin < 0.0m { return Verdict::Missed(-margin); }
  return Verdict::Held(margin);
}

comptime fn scene() {
  emit Ground { level: 0.0m };
  emit Sun { dir: normalize(Dir[World](-0.4, 0.8, 0.3)) };
  emit Fish { pos: fish_curve(0.0s) };
  emit Heron { pose: xform_identity(), hips: Pt[Body](0.0, 0.5, 0.0), head_pos: Pt[World](0.28, 1.02, 0.0),
               yaw: 0.0rad, neck_yaw: 0.0rad, limit: 1.1rad, hop_t: 0.0s, hopping: false,
               vel: Vec[World](0.0, 0.0, 0.0), gaze: fish_curve(0.0s), head: xform_identity() };
}

schedule Main {
  pass rows: scene();
  pass fish: run_laws(fish_swim);
  pass body: run_laws(heron_hop);
  pass neck: run_laws(heron_gaze);
  pass check: check(gaze_locked, head_above_hips);
  pass camera: orbit(at: Pt[World](0.0, 0.6, 0.0), radius: 4m, fov: 40deg);
  pass cull: build_indirect(Ground, Heron, Fish);
  pass vis: render_bundle(Vis64) { draw_indirect(Ground, Heron, Fish); };
  pass classify: tile_bin_kinds(Vis64, tile: 8u);
  pass shade: dispatch_material_tiles(Vis64, light: sun_sky);
  pass present: tonemap_aces(ColorHdr, sky: sky_color);
}
```
