# out_E_sonnet

## Task 1

```
space World Body Leg Claw Shell

# ---------- helpers ----------
fn clamp(x, lo, hi) = if x < lo { lo } else if x > hi { hi } else { x }
fn phase(rate) = rate * (time / 1s)
fn gaze() = [0.45m * sin(phase(0.31rad)), 0.10m, 0.55m + 0.20m * cos(phase(0.23rad))]@World
fn mount(x, y, z, yaw) = place([x, y, z]@Body) * rot_y(yaw)

# ---------- shell spiral ----------
fn growth(u) = exp(1.7 * u)
fn ang(u) = 6.2832rad * 3.5 * u
fn spine(u) = [0.016m * growth(u) * cos(ang(u)), 0.075m * u * growth(u), 0.016m * growth(u) * sin(ang(u))]@Shell
fn tube_r(u) = 0.010m * growth(u)

scan whorl u in 0..1 by 256 on bake_lut {
  init = frame(spine(u), dir(spine'(u)), [0,1,0])
  step prev = transport(prev, spine(u), dir(spine'(u)))
}

kind Shell @Shell (pose: xf@Shell>World) {
  surface(u, v) grid 64x32 = {
    fr = whorl(u)
    a = 6.2832rad * v
    rib = 1 + 0.07 * cos(14 * a) + 0.05 * sin(6.2832rad * 40 * u)
    fr.o + tube_r(u) * rib * (cos(a) * fr.n + sin(a) * fr.b)
  }
  material(p, n) = ggx([0.80,0.68,0.55], 0.55, 0) + coat(0.25, 0.30)
}

# ---------- claw ----------
kind Claw[serrated: bool] @Claw (pose: xf@Claw>World, open: rad, k: m) {
  trace(p) = {
    palm = round_box(p, [0.45 * k, 0.30 * k, 0.25 * k], 0.10 * k)
    fixed = capsule(p, [0.40 * k, -0.10 * k, 0], [1.35 * k, -0.20 * k, 0], 0.12 * k)
    q = p |> move([-0.45 * k, 0.20 * k, 0]) |> rot_z(-open)
    teeth = if serrated { 0.025 * k * abs(sin(6.2832rad * 7 * q.p.x / (0.9 * k))) } else { 0m }
    moving = capsule(q.p, [0,0,0], [0.9 * k, 0.1 * k, 0], 0.11 * k) + teeth
    smin(smin(palm, fixed, 0.05 * k), moving, 0.02 * k)
  }
  material(p, n) = chitin(p, n)
}

# ---------- legs ----------
kind CrabLeg @Leg (parent: CrabBody, pose: xf@Leg>World, femur: m, tibia: m, foot: p3@World) {
  trace(p) = {
    knee = [femur, 0, 0]@Leg
    tip = knee + tibia * dir(inv(pose) * foot - knee)
    min(capsule(p, [0,0,0]@Leg, knee, 0.006m), capsule(p, knee, tip, 0.005m))
  }
  material(p, n) = chitin(p, n)
}

# ---------- body and eyes ----------
kind CrabBody @Body (pose: xf@Body>World) {
  trace(p) = {
    sym = p |> mirror_x
    eye_c = [0.03m, 0.10m, 0.09m]@sym
    shell_c = capsule(p, [0, 0.02m, -0.02m], [0, 0.03m, 0.06m], 0.05m)
    stalk = capsule(sym.p, [0.03m, 0.045m, 0.07m]@sym, eye_c, 0.007m)
    globe = sphere(sym.p - eye_c, 0.014m)
    min(shell_c, min(stalk, globe))
  }
  material(p, n) = {
    sym = p |> mirror_x
    eye_c = [0.03m, 0.10m, 0.09m]@sym
    eye_w = pose * sym.out(eye_c)
    look = sym.in(inv(pose) * dir(gaze() - eye_w))
    if dist(sym.p, eye_c) < 0.02m && dot(sym.in(n), look) > 0.8 {
      ggx([0.02,0.02,0.02], 0.1, 0) + coat(1.0, 0.02)
    } else {
      chitin(p, n)
    }
  }
}

# ---------- materials ----------
fn chitin(p, n) = ggx([0.85,0.28,0.10], 0.18 + 0.12 * fbm2([p.x / 0.003m, (p.y + p.z) / 0.003m]), 0) + sss([0.9,0.3,0.1], 0.004m) + coat(0.7, 0.04)

kind Ground @World () {
  trace(p) = p.y
  material(p, n) = ggx([0.22,0.17,0.12], 0.25, 0) + coat(0.5, 0.05)
}

# ---------- build ----------
fn socket(j, side) = mount(side * 0.035m, 0.0m, -0.04m + 0.016m * j, side * 1.4rad)

fn crab(b, body: xf@Body>World) {
  for j in 0..6, side in [-1, 1] {
    CrabLeg(parent: b, pose: body * socket(j, side), femur: 0.05m + 0.004m * j, tibia: 0.06m + 0.003m * j, foot: body * [side * 0.11m, 0, -0.05m + 0.02m * j]@Body)
  }
  Shell(pose: body * mount(0.0m, 0.07m, -0.07m, 0rad))
  Claw[true](pose: body * mount(0.05m, 0.03m, 0.07m, 0.3rad), open: 0.5rad, k: 0.09m)
  Claw[false](pose: body * mount(-0.05m, 0.03m, 0.07m, -0.3rad), open: 0.2rad, k: 0.05m)
}

Ground()
at = place([0,0,0]@World)
crab(CrabBody(pose: at), at)

# ---------- light, camera, schedule ----------
light = sun(dir([-0.5,-1,-0.4]@World), [1.0,0.93,0.82] * 3)
sky(rd) = [0.55,0.70,0.90] * (1 - 0.4 * (1 - rd.y)) + [0.9,0.85,0.8] * 0.4 * (1 - rd.y)

schedule Main {
  orbit([0,0.05,0]@World, 0.8m, 0.45rad)
  build_indirect(Ground, CrabBody, CrabLeg, Claw, Shell)
  render_bundle(Vis64) { draw_indirect(Ground, CrabBody, CrabLeg, Claw, Shell) }
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sun_sky)
  tonemap_aces(ColorHdr, sky_color)
}
```

## Task 2

```
space World Reed

paper = [0.93,0.88,0.76]
ink_dark = [0.17,0.12,0.16]
ink_faint = [0.86,0.80,0.70]
wind = dir([0.8,0,0.6]@World)

fn clamp(x, lo, hi) = if x < lo { lo } else if x > hi { hi } else { x }
fn rnd(i, k) = fract(sin((i * 12.9 + k * 78.2) * 1rad) * 43758.5)
fn gust(x, z) = 0.5 + 0.5 * sin(2.4rad * (time / 1s) - 1.3rad * ((0.8 * x + 0.6 * z) / 1m))

kind Reed @Reed (pose: xf@Reed>World, height: m, lean: rad, stiff: f, phase: rad) {
  trace(p) = {
    o = pose * [0,0,0]@Reed
    h = clamp(p.y / height, 0, 1)
    amp = 0.30m * gust(o.x, o.z) * (1 - 0.8 * stiff) + 0.03m * sin(1.7rad * (time / 1s) + phase)
    off = [sin(lean) * height * h + amp * h * h, 0m, 0m]@Reed + (inv(pose) * wind) * (amp * h * h)
    q = p - off
    s0 = capsule(q, [0,0,0], [0, 0.33 * height, 0], 0.012m)
    s1 = capsule(q, [0, 0.33 * height, 0], [0, 0.66 * height, 0], 0.009m)
    s2 = capsule(q, [0, 0.66 * height, 0], [0, 0.90 * height, 0], 0.006m)
    head = capsule(q, [0, 0.88 * height, 0], [0, height, 0], 0.022m)
    min(min(s0, s1), min(s2, head))
  }
  material(p, n) = ink(ink_dark, 0.0)
}

lobe ink(dark, cut, n, v, l) = if dot(n, l) < cut { dark } else { paper }

kind Water @World () {
  trace(p) = p.y
  material(p, n) = emit(if fract(p.z / 0.08m + 0.3 * sin((p.x / 0.4m) * 1rad)) < 0.12 { ink_faint } else { paper })
}

light = sun(dir([-0.5,-0.8,-0.3]@World), [1,1,1])
sky(rd) = if fract(rd.y * 18) < 0.06 { ink_faint } else { paper }

# 200 reeds, one kind, loose irregular cluster
for i in 0..200 {
  a = 6.2832rad * rnd(i, 1)
  r = 1.4m * rnd(i, 2) * rnd(i, 2)
  x = r * cos(a) + 0.25m * (rnd(i, 3) - 0.5)
  z = r * sin(a) + 0.25m * (rnd(i, 8) - 0.5)
  Reed(pose: place([x, 0, z]@World), height: 1.1m + 0.7m * rnd(i, 4), lean: 0.24rad * (rnd(i, 5) - 0.5), stiff: 0.2 + 0.7 * rnd(i, 6), phase: 6.2832rad * rnd(i, 7))
}
Water()

schedule Main {
  orbit([0,0.7,0]@World, 4m, 0.25rad)
  build_indirect(Water, Reed)
  render_bundle(Vis64) { draw_indirect(Water, Reed) }
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sun_sky)
  tonemap_aces(ColorHdr, sky_color)
}
```

## Task 3

```
space World Body Head

neck_limit = 1.0rad
hop_len = 0.5s
hop_h = 0.18m
turn_rate = 2.4rad

fn clamp(x, lo, hi) = if x < lo { lo } else if x > hi { hi } else { x }
fn wrap_pi(a) = a - 6.2832rad * floor(a / 6.2832rad + 0.5)
fn bearing(a, b) = atan2(b.x - a.x, b.z - a.z)
fn fish_curve(u) = [2.0m * sin(u * 1rad), 0.03m, 0.8m + 0.6m * cos(2 * u * 1rad)]@World

kind Fish @World (u: f, pos: p3@World) {
  trace(p) = sphere(p - pos, 0.05m)
  material(p, n) = ggx([0.7,0.75,0.8], 0.2, 0.6)
}

kind Heron @Body (pose: xf@Body>World, pos: p3@World, heading: rad, hop: s, lift: m, target: Fish) {
  trace(p) = {
    sym = p |> mirror_x
    trunk = capsule(p, [0, 0.04m, -0.20m], [0, 0.10m, 0.12m], 0.10m)
    legs = capsule(sym.p, [0.05m, -0.05m, -0.08m]@sym, [0.05m, -0.72m, -0.05m]@sym, 0.012m)
    min(trunk, legs)
  }
  material(p, n) = ggx([0.55,0.6,0.65], 0.5, 0)
}

kind Head @Head (parent: Heron, target: Fish, pose: xf@Head>World, yaw: rad, pitch: rad, leftover: rad, eye: p3@World, hips: p3@World) {
  trace(p) = {
    neck = min(capsule(p, [0, -0.30m, -0.12m], [0, -0.18m, 0.08m], 0.02m),
           min(capsule(p, [0, -0.18m, 0.08m], [0, -0.08m, -0.04m], 0.02m),
               capsule(p, [0, -0.08m, -0.04m], [0, 0, 0.02m], 0.02m)))
    skull = sphere(p - [0,0,0]@Head, 0.035m)
    beak = capsule(p, [0, -0.005m, 0.03m], [0, -0.01m, 0.25m], 0.012m)
    min(neck, min(skull, beak))
  }
  material(p, n) = ggx([0.9,0.85,0.6], 0.4, 0)
}

kind Water @World () {
  trace(p) = p.y
  material(p, n) = ggx([0.1,0.2,0.25], 0.1, 0) + coat(0.6, 0.02)
}

law fish_swim(f: Fish, dt) {
  f.u = f.u + 0.5 * (dt / 1s)
  f.pos = fish_curve(f.u)
}

# 1. body: hop and turn while the neck is at its limit
law heron_body(b: Heron, dt, fish: [Fish]) {
  want = wrap_pi(bearing(b.pos, fish[b.target].pos) - b.heading)
  if b.hop < 0s && abs(want) > neck_limit { b.hop = 0s }
  if b.hop >= 0s {
    k = b.hop / hop_len
    step = turn_rate * (dt / 1s)
    b.heading = b.heading + clamp(want, -step, step)
    b.lift = 4 * hop_h * k * (1 - k)
    b.hop = b.hop + dt
    if b.hop >= hop_len {
      b.hop = -1s
      b.lift = 0m
    }
  }
  b.pose = place([b.pos.x, 0.75m + b.lift, b.pos.z]@World) * rot_y(b.heading)
}

# 2. neck and head: gaze lock first, then the limit
law neck_head(h: Head, dt, bodies: [Heron], fish: [Fish]) {
  b = bodies[h.parent]
  anchor = b.pose * [0, 0.5m, 0.28m]@Body
  to = fish[h.target].pos
  want = wrap_pi(bearing(anchor, to) - b.heading)
  h.yaw = clamp(want, -neck_limit, neck_limit)
  h.leftover = abs(want - h.yaw)
  h.pitch = clamp(atan2(to.y - anchor.y, dist(anchor, [to.x, anchor.y, to.z]@World)), -0.9rad, 0.5rad)
  h.pose = b.pose * place([0, 0.5m, 0.28m]@Body) * rot_y(h.yaw) * rot_x(-h.pitch)
  h.eye = h.pose * [0, 0.025m, 0.05m]@Head
  h.hips = b.pose * [0, -0.05m, -0.08m]@Body
}

# 3. checks
check gaze(h: Head) {
  if !finite(h.pose) { return unknown(NonFinitePose) }
  if h.leftover > 0.001rad { missed(h.leftover) } else { held(neck_limit - abs(h.yaw)) }
}

check head_above_hips(h: Head) {
  if !finite(h.eye) { return unknown(NonFiniteEye) }
  margin = h.eye.y - h.hips.y - 0.2m
  if margin < 0m { missed(-margin) } else { held(margin) }
}

# build
fish = Fish(u: 0.0, pos: fish_curve(0.0))
body = Heron(pose: place([0, 0.75m, 0]@World), pos: [0,0,0]@World, heading: 0rad, hop: -1s, lift: 0m, target: fish)
Head(parent: body, target: fish, pose: place([0, 1.25m, 0.28m]@World), yaw: 0rad, pitch: 0rad, leftover: 0rad, eye: [0, 1.27m, 0.33m]@World, hips: [0, 0.7m, -0.08m]@World)
Water()

light = sun(dir([-0.4,-0.8,-0.3]@World), [1.0,0.95,0.85] * 3)
sky(rd) = [0.6,0.72,0.88]

schedule Main {
  run_laws(fish_swim, heron_body, neck_head)
  check(gaze, head_above_hips)
  orbit([0,0.8m,0.5m]@World, 3.5m, 0.35rad)
  build_indirect(Water, Heron, Head, Fish)
  render_bundle(Vis64) { draw_indirect(Water, Heron, Head, Fish) }
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sun_sky)
  tonemap_aces(ColorHdr, sky_color)
}
```
