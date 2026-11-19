# Card A (Haiku 5.5) outputs

## Task 1: a hermit crab

```loft
space World, Body, Leg, Claw, Face, Shell;

// ---- sockets: where each part sits on the body (Body -> World is set per crab) ----
fn leg_socket(j: u32, side: f32) -> Xform[Leg -> Body] {
  return translate_xf(Vec[Body](side * 0.25m, 0.05m, 0.12m * f32(j) - 0.3m)) * rot_y(side * 0.8rad);
}
fn claw_socket(side: f32) -> Xform[Claw -> Body] {
  return translate_xf(Vec[Body](0.30m, 0.02m, 0.25m * side)) * rot_y(0.0rad);
}
fn eye_socket() -> Xform[Face -> Body] {
  return translate_xf(Vec[Body](0.0m, 0.0m, 0.30m));
}
fn shell_socket() -> Xform[Shell -> Body] {
  return translate_xf(Vec[Body](-0.10m, 0.10m, 0.0m));
}
fn body_pose() -> Xform[Body -> World] {
  return translate_xf(Vec[World](0.0m, 0.12m, 0.0m)) * rot_y(0.3rad);
}

// ---- wet chitin: one material, noise only in the material ----
fn wet_roughness(p: vec3<f32>) -> f32 {
  return 0.25 + 0.2 * fbm2(p.xz * 60.0);
}
fn wet_chitin(p: vec3<f32>) -> Bsdf {
  let r = wet_roughness(p);
  return ggx(vec3(0.75, 0.22, 0.08), r, 0.0)      // red-orange base, bottom
       + sss(vec3(0.9, 0.3, 0.1), 0.08)           // translucent subsurface
       + clearcoat(0.7, 0.07);                    // clear wet coat on top
}

// ---- shell: a coiled spiral spine with transported frames ----
fn spine(u: f32) -> Pt[Shell] {
  let turn = 6.2831853 * 2.5 * u;
  let r = 0.55 * (1.0 - 0.35 * u);
  return Pt[Shell](r * cos(turn), 0.9 * u, r * sin(turn));
}

scan whorl(u: f32 in 0.0..1.0, steps: 256u) @schedule(bake_lut) -> Table[Frame[Shell]] {
  init: frame_from_tangent(spine(u), normalize(diff(spine, u)(u)), Dir[Shell](0.0, 1.0, 0.0));
  step(prev, u_i): transport_frame(prev, spine(u_i), normalize(diff(spine, u)(u_i)));
}

kind Whorl {
  row { parent: RowId[CrabBody], pose: Xform[Shell -> World], tube: Meters, ribs: f32 }
  @grid(256u, 24u) query surface(self, u: f32, v: f32) -> Pt[Shell] {
    let f = sample_c2(whorl, u);
    let ang = 6.2831853 * v;
    let grow = 1.0 + 0.9 * u;                                      // radius grows with the spiral
    let rib = 1.0 + 0.12 * sin(self.ribs * 6.2831853 * 14.0 * u);  // ribs run along the whorl
    let r = self.tube * grow * rib;
    return f.origin + (f.normal * cos(ang) + f.binormal * sin(ang)) * r;
  }
}

// ---- claw: one definition; crusher and feeder differ only in row data ----
kind Claw {
  row { parent: RowId[CrabBody], pose: Xform[Claw -> World], size: Meters, open: Rad, teeth: f32 }
  query trace(self, p: Pt[Claw]) -> Field[Claw] {
    let palm  = sd_round_box(p - Pt[Claw](0.0m, 0.0m, 0.0m),
                             Vec[Claw](self.size, 0.8 * self.size, 0.7 * self.size), 0.25 * self.size);
    let hinge = Pt[Claw](self.size, 0.0m, 0.0m);
    let fixed = sd_capsule(p, Pt[Claw](self.size, -0.1 * self.size, 0.0m),
                              Pt[Claw](1.6 * self.size, -0.1 * self.size, 0.0m), 0.035 * self.size);
    // rotate the query point back about the hinge, so the moving finger stays a plain capsule
    let d = p - hinge;
    let c = cos(self.open);
    let s = sin(self.open);
    let q = hinge + Vec[Claw](c * d.x + s * d.y, -s * d.x + c * d.y, d.z);
    let tip = hinge + Vec[Claw](1.6 * self.size, 0.0m, 0.0m);
    let teeth = 0.006m * sin(self.teeth * 6.2831853 * (q.x - hinge.x) / self.size);   // serrations
    let mover = sd_capsule(q, hinge, tip, 0.03 * self.size) + teeth;
    return smin_poly(palm, min(fixed, mover), 0.02 * self.size);
  }
  query material(self, p: Pt[Claw], n: Norm[Claw]) -> Bsdf {
    return wet_chitin(vec3<f32>(p.x / 1.0m, p.y / 1.0m, p.z / 1.0m));
  }
}

// ---- legs: one kind, one row per leg ----
kind CrabLeg {
  row { parent: RowId[CrabBody], pose: Xform[Leg -> World], femur: Meters, tibia: Meters, foot: Pt[World] }
  query trace(self, p: Pt[Leg]) -> Field[Leg] {
    let knee  = Pt[Leg](self.femur, 0.0m, 0.0m);
    let ankle = knee + Vec[Leg](self.tibia * 0.6, -self.tibia * 0.8, 0.0m);
    let thigh = sd_capsule(p, Pt[Leg](0.0m, 0.0m, 0.0m), knee, 0.07m);
    let shank = sd_capsule(p, knee, ankle, 0.05m);
    return smin_poly(thigh, shank, 0.03m);
  }
  query material(self, p: Pt[Leg], n: Norm[Leg]) -> Bsdf {
    return wet_chitin(vec3<f32>(p.x / 1.0m, p.y / 1.0m, p.z / 1.0m));
  }
}

// ---- body ----
kind CrabBody {
  row { pose: Xform[Body -> World], com: Pt[World], vel: Vec[World] }
  query trace(self, p: Pt[Body]) -> Field[Body] {
    return sd_round_box(p - Pt[Body](0.0m, 0.0m, 0.0m), Vec[Body](0.35m, 0.12m, 0.45m), 0.10m);
  }
  query material(self, p: Pt[Body], n: Norm[Body]) -> Bsdf {
    return wet_chitin(vec3<f32>(p.x / 1.0m, p.y / 1.0m, p.z / 1.0m));
  }
}

// ---- eyes: one mirrored definition; the mirror cell gives the two eyes ----
map sym = mirror_x(q) |> translate(-Vec[Face](0.15, 0.72, 0.42));
let eye_c = Pt['sym](0.03, 0.30, 0.07);

fn gaze_target(time: Sec) -> Pt[World] {
  // a point in world space that drifts slowly
  return Pt[World](0.6 * sin(0.2 * f32(time)), 0.25, 1.4 + 0.3 * cos(0.13 * f32(time)));
}

kind Eye {
  row { parent: RowId[CrabBody], pose: Xform[Face -> World] }
  query trace(self, p: Pt[Face]) -> Field[Face] {
    let stalk = sd_capsule(sym, Pt['sym](0.0, -0.25, 0.0), Pt['sym](0.0, 0.0, 0.0), 0.03m);
    return smin_poly(sd_sphere(sym - eye_c, 0.085m), stalk, 0.02m);
  }
  query material(self, p: Pt[Face], n: Norm[Face], time: Sec) -> Bsdf {
    let target = gaze_target(time);
    let eye_w = self.pose * (sym.to_parent * eye_c);                  // each eye's own world position
    let look = sym.from_parent * (inv(self.pose) * normalize(target - eye_w));
    let pupil = select(vec3(0.02, 0.02, 0.03), vec3(0.9, 0.85, 0.2), dot(n, look) > 0.6);
    return ggx(vec3(0.85, 0.8, 0.7), 0.2, 0.0) + emission(pupil);
  }
}

// ---- sand, light, sky ----
kind Sand {
  row { wetness: f32 }
  query trace(self, p: Pt[World]) -> Field[World] { return p.y; }
  query material(self, p: Pt[World], n: Norm[World]) -> Bsdf {
    return ggx(vec3(0.42, 0.36, 0.30), mix(0.5, 0.15, self.wetness), 0.0);
  }
}

kind SunSky {
  row { dir: Dir[World], power: f32 }
  query light(self, p: Pt[World], n: Norm[World]) -> vec3<f32> {
    return vec3(self.power) * max(dot(n, self.dir), 0.0);
  }
}

kind Sky {
  row { zenith: vec3<f32> }
  query sky(self, rd: Dir[World]) -> vec3<f32> {
    return mix(vec3(0.7, 0.8, 0.9), self.zenith, rd.y);
  }
}

let sun_sky = SunSky { dir: normalize(Dir[World](-0.4, 0.8, -0.3)), power: 1.1 };
let sky_color = Sky { zenith: vec3(0.35, 0.55, 0.85) };

// ---- build: rows only; the crab is emitted by a loop over legs ----
comptime fn crab(b: RowId[CrabBody], body: Xform[Body -> World]) {
  comptime for j in 0u..6u {
    comptime for side in [-1.0, 1.0] {
      emit CrabLeg { parent: b, pose: body * leg_socket(j, side), femur: 0.40m + 0.05m * f32(j),
                     tibia: 0.30m - 0.02m * f32(j), foot: body * Pt[Body](side, 0.0, 0.3 * f32(j)) };
    }
  }
  // crusher on one side, feeder on the other: same kind, different data
  emit Claw { parent: b, pose: body * claw_socket(-1.0), size: 0.20m, open: 0.6rad, teeth: 5.0 };
  emit Claw { parent: b, pose: body * claw_socket(1.0),  size: 0.08m, open: 0.3rad, teeth: 3.0 };
  emit Whorl { parent: b, pose: body * shell_socket(), tube: 0.07m, ribs: 3.0 };
  emit Eye { parent: b, pose: body * eye_socket() };
}

comptime fn crabs() {
  let b = emit CrabBody { pose: body_pose(), com: Pt[World](0.0, 0.1, 0.0), vel: Vec[World](0.0, 0.0, 0.0) };
  crab(b, body_pose());
  emit Sand { wetness: 0.8 };
}

schedule Main {
  pass rows: crabs();
  pass camera: orbit(at: Pt[World](0.0, 0.1, 0.0), radius: 2.4m, fov: 40deg);
  pass cull: build_indirect(Sand, CrabBody, CrabLeg, Claw, Whorl, Eye);
  pass vis: render_bundle(Vis64) { draw_indirect(Sand, CrabBody, CrabLeg, Claw, Whorl, Eye); };
  pass classify: tile_bin_kinds(Vis64, tile: 8u);
  pass shade: dispatch_material_tiles(Vis64, light: sun_sky);
  pass present: tonemap_aces(ColorHdr, sky: sky_color);
}
```

## Task 2: a reed bed in wind

```loft
space World;

// ---- ink: two flat inks, a hard edge between lit and shadowed ----
@lobe fn ink(dark: vec3<f32>, cut: f32, n: vec3<f32>, v: vec3<f32>, l: vec3<f32>) -> vec4<f32> {
  return vec4(select(vec3(0.92, 0.85, 0.7), dark, dot(n, l) < cut), 0.0);
}

let sun_dir = normalize(Dir[World](-0.5, 0.8, -0.3));
let view_dir = Dir[World](0.0, 0.0, 1.0);

// ---- placement: 200 reeds in a loose, irregular cluster; built once ----
comptime fn hash01(n: u32) -> f32 {
  return fract(sin(f32(n) * 12.9898) * 43758.5453);
}

comptime fn reeds() {
  comptime for i in 0u..200u {
    let k = 7u * i;
    let a = 6.2831853 * hash01(k);
    let r = 3.0 * sqrt(hash01(k + 1u));
    emit Reed {
      root: Pt[World](r * cos(a), 0.0, r * sin(a)),
      height: 0.9m + 0.5m * hash01(k + 2u),
      lean: 0.4rad * (hash01(k + 3u) - 0.5),
      stiff: 0.2 + 0.8 * hash01(k + 4u),
      phase: 6.2831853 * hash01(k + 5u),
      base_r: 0.012m + 0.004m * hash01(k + 6u),
      seed_r: 0.035m,
    };
  }
}

// ---- reed: a tapered stalk with a seed head; sway from its own data and time ----
kind Reed {
  row { root: Pt[World], height: Meters, lean: Rad, stiff: f32, phase: f32, base_r: Meters, seed_r: Meters }

  query trace(self, p: Pt[World], time: Sec) -> Field[World] {
    let t = f32(time);
    let k = 1.0 / (1.0 + 2.0 * self.stiff);                      // stiffer reeds bend less
    let gust = sin(0.9 * f32(self.root.x) - 2.4 * t) * 0.5 + 0.5; // a gust travels across the bed
    let sway = f32(self.lean) + 0.5 * sin(self.phase + 1.7 * t) + 0.6 * gust;
    let bend = sway * k * f32(self.height);                      // displacement scale, metres
    var d = 1.0e3m;
    var a = self.root;
    for s in 1u..7u {
      let v = f32(s) / 6.0;
      // the bend grows toward the tip: offset is quadratic in height
      let b = Pt[World](self.root.x + bend * v * v, self.root.y + v * self.height, self.root.z);
      let r = self.base_r * (1.0 - 0.6 * v);
      d = smin_poly(d, sd_capsule(p, a, b, r), 0.01m);
      a = b;
    }
    return smin_poly(d, sd_sphere(p - a, self.seed_r), 0.02m);
  }

  query material(self, p: Pt[World], n: Norm[World]) -> Bsdf {
    return ink(vec3(0.10, 0.08, 0.07), 0.0, n, view_dir, sun_dir);
  }
}

// ---- water and sky: plain paper with a faint second ink ----
kind Water {
  row { level: Meters }
  query trace(self, p: Pt[World]) -> Field[World] { return p.y - self.level; }
  query material(self, p: Pt[World], n: Norm[World]) -> Bsdf {
    return ink(vec3(0.80, 0.76, 0.66), -0.6, n, view_dir, sun_dir);
  }
}

kind Sky {
  row { paper: vec3<f32> }
  query sky(self, rd: Dir[World]) -> vec3<f32> { return self.paper; }
}

kind SunSky {
  row { dir: Dir[World], power: f32 }
  query light(self, p: Pt[World], n: Norm[World]) -> vec3<f32> {
    return vec3(self.power) * max(dot(n, self.dir), 0.0);
  }
}

let sun_sky = SunSky { dir: sun_dir, power: 1.0 };
let sky_color = Sky { paper: vec3(0.96, 0.92, 0.84) };

comptime fn reeds_scene() {
  reeds();
  emit Water { level: 0.0m };
}

schedule Main {
  pass rows: reeds_scene();
  pass camera: orbit(at: Pt[World](0.0, 0.5, 0.0), radius: 7m, fov: 40deg);
  pass cull: build_indirect(Water, Reed, Sky);
  pass vis: render_bundle(Vis64) { draw_indirect(Water, Reed); };
  pass classify: tile_bin_kinds(Vis64, tile: 8u);
  pass shade: dispatch_material_tiles(Vis64, light: sun_sky);
  pass present: clamp_ldr(ColorHdr, sky: sky_color);
}
```

## Task 3: a heron's hop

```loft
space World, Body, Fish;

// ---- fish: moves along a curve in world space; its position is the gaze target ----
fn fish_path(t: Sec) -> Pt[World] {
  let s = f32(t);
  return Pt[World](1.4 * sin(0.25 * s), 0.02, 1.6 + 0.4 * cos(0.18 * s));
}

kind Fish {
  row { clock: Sec, pos: Pt[World] }
  query trace(self, p: Pt[World]) -> Field[World] { return sd_sphere(p - self.pos, 0.06m); }
  query material(self, p: Pt[World], n: Norm[World]) -> Bsdf {
    return ggx(vec3(0.9, 0.5, 0.2), 0.3, 0.0) + emission(vec3(0.3, 0.12, 0.02));
  }
}

law swim(f: Fish, dt: Sec) writes Fish {
  f.clock = f.clock + dt;
  f.pos = fish_path(f.clock);
}

// ---- heron: trunk, two legs, S-shaped neck, head and beak, in Body space ----
// the neck pivots at the top of the trunk; its S-curve is laid out in Body space
fn neck_pt(s: f32) -> Pt[Body] {
  return Pt[Body](0.0, 0.62 + 0.55 * s, 0.15 + 0.2 * sin(6.2831853 * s));
}

fn pose_at(origin: Pt[World], yaw: Rad) -> Xform[Body -> World] {
  return translate_xf(Vec[World](origin.x, origin.y, origin.z)) * rot_y(yaw);
}

fn wrap_angle(a: f32) -> Rad {
  return Rad(atan2(sin(a), cos(a)));
}

kind Heron {
  row {
    base: Pt[World], pose: Xform[Body -> World], yaw: Rad, lift: Meters,
    hop: Sec, hopping: f32, target: RowId[Fish],
    neck: Rad, limit: Rad, want: Rad, gaze_err: Rad, at_limit: f32,
    head_y: Meters, hip_y: Meters
  }

  query trace(self, p: Pt[Body]) -> Field[Body] {
    // trunk, fixed in the body
    var d = sd_capsule(p, Pt[Body](0.0m, 0.45m, -0.15m), Pt[Body](0.0m, 0.62m, 0.15m), 0.12m);

    // neck and head turn about the trunk top by the neck angle: evaluate in the neck's frame
    let pivot = neck_pt(0.0);
    let c = cos(self.neck);
    let s = sin(self.neck);
    let r = p - pivot;
    let q = pivot + Vec[Body](c * r.x - s * r.z, r.y, s * r.x + c * r.z);
    for i in 0u..8u {
      let a = neck_pt(f32(i) / 8.0);
      let b = neck_pt(f32(i + 1u) / 8.0);
      d = smin_poly(d, sd_capsule(q, a, b, 0.035m), 0.02m);
    }
    let head_c = neck_pt(1.0) + Vec[Body](0.0m, 0.04m, 0.0m);
    let skull = sd_sphere(q - head_c, 0.07m);
    // beak runs along the head's forward axis (+z in the neck frame)
    let beak = sd_capsule(q, head_c + Vec[Body](0.0m, 0.0m, 0.05m), head_c + Vec[Body](0.0m, -0.01m, 0.30m), 0.02m);
    d = smin_poly(d, smin_poly(skull, beak, 0.01m), 0.02m);

    // two legs
    for k in 0u..2u {
      let side = f32(k) * 2.0 - 1.0;
      let hip  = Pt[Body](side * 0.08m, 0.45m, 0.0m);
      let knee = Pt[Body](side * 0.10m, 0.22m, 0.10m);
      let foot = Pt[Body](side * 0.08m, 0.0m, 0.0m);
      let leg = smin_poly(sd_capsule(p, hip, knee, 0.02m), sd_capsule(p, knee, foot, 0.012m), 0.01m);
      d = smin_poly(d, leg, 0.01m);
    }
    return d;
  }

  query material(self, p: Pt[Body], n: Norm[Body]) -> Bsdf {
    return ggx(vec3(0.62, 0.66, 0.68), 0.5, 0.0) + clearcoat(0.1, 0.2);
  }
}

// ---- laws, in order: body first ----
// the body hops while the neck is at its limit, turns the trunk toward the fish, and lands
law body_law(b: Heron, dt: Sec) writes Heron {
  if b.hopping < 0.5 && b.at_limit > 0.5 {
    b.hopping = 1.0;
    b.hop = 0.0s;
  }
  if b.hopping > 0.5 {
    b.hop = b.hop + dt;
    let phase = b.hop / 0.6s;
    b.lift = 0.22m * sin(3.14159265 * phase);
    // trunk turns toward the fish's world heading, at most 1.5 rad/s
    let step = 1.5rad * dt / 1.0s;
    b.yaw = b.yaw + clamp(b.want - b.yaw, -step, step);
    if phase >= 1.0 {
      b.hopping = 0.0;
      b.lift = 0.0m;
    }
  }
  b.pose = pose_at(b.base + Vec[World](0.0m, b.lift, 0.0m), b.yaw);
}

// then the neck and head: gaze lock first (aim at the fish in world space), then clamp the neck to its limit
law neck_law(b: Heron, dt: Sec, fish: Table[Fish]) writes Heron {
  let f = fish[b.target];
  let head = b.pose * neck_pt(1.0);
  let rel = f.pos - head;
  let want_abs = Rad(atan2(rel.x, rel.z));
  let rel_yaw = wrap_angle(f32(want_abs - b.yaw));
  let kept = clamp(rel_yaw, -b.limit, b.limit);
  b.neck = kept;
  b.gaze_err = rel_yaw - kept;               // the part of the gaze the neck cannot reach
  b.at_limit = f32(abs(rel_yaw) > b.limit);
  b.want = want_abs;
  b.head_y = head.y;
  b.hip_y = (b.pose * Pt[Body](0.0m, 0.45m, 0.0m)).y;
}

// checks: the gaze is held only when the neck reaches the target; otherwise it is missed by the leftover angle
constraint gaze_lock(b: Heron) -> Verdict {
  if !is_finite(b.pose) { return Verdict::Unknown(NonFinitePose); }
  let miss = abs(b.gaze_err);
  if miss > 0.01rad { return Verdict::Missed(miss); }
  return Verdict::Held(0.01rad - miss);
}

constraint head_above_hips(b: Heron) -> Verdict {
  if !is_finite(b.pose) { return Verdict::Unknown(NonFinitePose); }
  let margin = b.head_y - b.hip_y;
  if margin < 0.0m { return Verdict::Missed(-margin); }
  return Verdict::Held(margin);
}

// ---- build, light, camera, water ----
comptime fn heron_scene() {
  let fish = emit Fish { clock: 0.0s, pos: fish_path(0.0s) };
  emit Heron {
    base: Pt[World](0.0, 0.0, 0.0), pose: pose_at(Pt[World](0.0, 0.0, 0.0), 0.0rad),
    yaw: 0.0rad, lift: 0.0m, hop: 0.0s, hopping: 0.0, target: fish,
    neck: 0.0rad, limit: 0.6rad, want: 0.0rad, gaze_err: 0.0rad, at_limit: 0.0,
    head_y: 1.17m, hip_y: 0.45m
  };
  emit Water { level: 0.0m };
}

kind Water {
  row { level: Meters }
  query trace(self, p: Pt[World]) -> Field[World] { return p.y - self.level; }
  query material(self, p: Pt[World], n: Norm[World]) -> Bsdf {
    return ggx(vec3(0.35, 0.42, 0.45), 0.08, 0.0);
  }
}

kind SunSky {
  row { dir: Dir[World], power: f32 }
  query light(self, p: Pt[World], n: Norm[World]) -> vec3<f32> {
    return vec3(self.power) * max(dot(n, self.dir), 0.0);
  }
}

kind Sky {
  row { zenith: vec3<f32> }
  query sky(self, rd: Dir[World]) -> vec3<f32> {
    return mix(vec3(0.7, 0.8, 0.9), self.zenith, rd.y);
  }
}

let sun_sky = SunSky { dir: normalize(Dir[World](-0.4, 0.8, -0.3)), power: 1.1 };
let sky_color = Sky { zenith: vec3(0.35, 0.55, 0.85) };

schedule Main {
  pass rows: heron_scene();
  pass fish: run_laws(Fish -> swim);
  pass body: run_laws(Heron -> body_law);
  pass neck: run_laws(Heron -> neck_law);
  pass check: check(gaze_lock);
  pass check2: check(head_above_hips);
  pass camera: orbit(at: Pt[World](0.0, 0.6, 0.6), radius: 4.0m, fov: 40deg);
  pass cull: build_indirect(Water, Heron, Fish);
  pass vis: render_bundle(Vis64) { draw_indirect(Water, Heron, Fish); };
  pass classify: tile_bin_kinds(Vis64, tile: 8u);
  pass shade: dispatch_material_tiles(Vis64, light: sun_sky);
  pass present: tonemap_aces(ColorHdr, sky: sky_color);
}
```
