## Task 1: a hermit crab

```
space World Body Leg Claw Shell Eye

sun = dir([0.35, 1.0, 0.25]@World)
chitin = [0.72, 0.22, 0.08]
sss_tint = [0.9, 0.3, 0.1]
sand = [0.42, 0.36, 0.28]

# ---- shell: spiral spine, transported frame, ribbed tube ----
fn spine(u: f) -> p3@Shell {
  a = 6.2831853rad * 3.0 * u
  r = 0.05m * (1.0 - 0.4 * u)
  [r * cos(a), 0.12m * u, r * sin(a)]
}

fn tube_r(u: f) -> m = 0.004m + 0.012m * u

fn rib(u: f) -> m = 0.0009m * sin(6.2831853rad * 14.0 * u)

scan whorl u in 0..1 by 256 on bake_lut {
  init = frame(spine(u), dir(spine'(u)), [0,1,0])
  step prev = transport(prev, spine(u), dir(spine'(u)))
}

kind Shell @Shell (pose: xf@Shell>World) {
  surface(u, v) grid 64x32 = whorl(u).o + (tube_r(u) + rib(u)) * (cos(v) * whorl(u).n + sin(v) * whorl(u).b)
  material(p, n) = ggx(chitin, 0.3, 0) + sss(sss_tint, 0.02m) + coat(0.7, 0.05)
}

# ---- claw: rounded palm, fixed finger, hinged serrated finger ----
fn palm(p: p3@Claw, size: m) -> m = round_box(p, [0.5 * size, 0.4 * size, 0.3 * size], 0.1 * size)

fn fixed_finger(p: p3@Claw, size: m) -> m =
  capsule(p, [0.2 * size, 0.3 * size, 0.0m], [1.1 * size, 0.1 * size, 0.0m], 0.08 * size)

fn moving_finger(p: p3@Claw, size: m, open: rad) -> m {
  q = p |> move([-0.2 * size, 0.15 * size, 0.0m]) |> rot_z(-open)
  teeth = 0.03 * size * max(0.0, sin(90.0 * q.x / size))
  capsule(q, [0.0m, 0.0m, 0.0m], [1.0 * size, 0.1 * size, 0.0m], 0.07 * size) - teeth
}

fn chitin_rough(p: p3@Claw) -> f = 0.22 + 0.15 * fbm2(p * 180.0)

kind Claw @Claw (pose: xf@Claw>World, open: rad, size: m) {
  trace(p) = min(smin(palm(p, size), fixed_finger(p, size), 0.02m), moving_finger(p, size, open))
  material(p, n) = ggx(chitin, chitin_rough(p), 0) + sss(sss_tint, 0.02m) + coat(0.7, 0.04)
}

# ---- legs: one kind, twelve rows ----
kind CrabLeg @Leg (parent: CrabBody, pose: xf@Leg>World, femur: m, tibia: m, foot: p3@World) {
  trace(p) = min(capsule(p, [0,0,0], [femur,0,0], 0.07m), capsule(p, [femur,0,0], [femur,-tibia,0], 0.04m))
  material(p, n) = ggx(chitin, 0.3, 0) + coat(0.7, 0.05)
}

# ---- body ----
kind CrabBody @Body (pose: xf@Body>World, head: p3@Body, vel: v3@World, com: p3@World) {
  trace(p) = round_box(p, [0.22m, 0.08m, 0.16m], 0.06m)
  material(p, n) = ggx(chitin, 0.3, 0) + coat(0.7, 0.05)
}

# ---- eyes: one mirrored definition ----
kind Eye @Eye (pose: xf@Eye>World) {
  trace(q) = {
    sym = q |> mirror_x |> move([-0.15,-0.72,-0.42])
    eye_c = [0.03,0.30,0.07]@sym
    globe = sphere(sym.p - eye_c, 0.085m)
    stalk = capsule(sym.p, [0.0m,0.0m,0.0m]@sym, [0.0m,0.1m,0.0m]@sym, 0.02m)
    min(globe, stalk)
  }
  material(q, n) = {
    sym = q |> mirror_x |> move([-0.15,-0.72,-0.42])
    eye_c = [0.03,0.30,0.07]@sym
    eye_w = pose * sym.out(eye_c)
    gaze = [0.3m * sin(0.2 * time), 0.4m, 1.0m]@World
    look = sym.in(inv(pose) * dir(gaze - eye_w))
    pupil = dot(normalize(sym.p - eye_c), look) > 0.93
    ink = if pupil { [0.02,0.02,0.02] } else { [0.9,0.85,0.8] }
    ggx(ink, 0.15, 0) + coat(0.8, 0.02)
  }
}

# ---- sand and sky ----
kind Sand @World (level: m) {
  trace(p) = p.y - level
  material(p, n) = ggx(sand, 0.12, 0) + coat(0.5, 0.02)
}

light(p, n) = [1.0, 0.94, 0.85] * max(0.0, dot(n, sun))
sky(rd) = [0.78, 0.84, 0.92]

# ---- laws and checks ----
law crab_step(leg: CrabLeg, dt, bodies: [CrabBody]) {
  b = bodies[leg.parent]
  cp = b.com + b.vel * 0.28s
  if dist(leg.foot, cp) > 0.5m { leg.foot = [cp.x,0,cp.z] }
}

check upright(b: CrabBody) {
  if !finite(b.pose) { return unknown(NonFinitePose) }
  margin = (b.pose * b.head).y - 0.15m
  if margin < 0m { missed(-margin) } else { held(margin) }
}

# ---- build ----
fn socket(j: f, side: f) -> xf@Leg>Body = frame([0.12m * side, 0.05m, 0.12m * j - 0.3m]@Body, [side, 0, 0]@Body, [0,1,0]@Body)

fn shell_socket() -> xf@Shell>Body = frame([0, 0.06m, -0.05m]@Body, [0,0,1]@Body, [0,1,0]@Body)

fn claw_socket(side: f) -> xf@Claw>Body = frame([0.25m * side, 0.05m, 0.4m]@Body, [0,0,1]@Body, [0,1,0]@Body)

fn eye_socket(side: f) -> xf@Eye>Body = frame([0.06m * side, 0.1m, 0.2m]@Body, [0,0,1]@Body, [0,1,0]@Body)

fn crab(b, body: xf@Body>World) {
  for j in 0..6, side in [-1, 1] {
    CrabLeg(parent: b, pose: body * socket(j, side), femur: 0.40m + 0.05m * j, tibia: 0.30m - 0.02m * j, foot: body * [side, 0, 0.3 * j]@Body)
  }
  Shell(pose: body * shell_socket())
  Claw(pose: body * claw_socket(1), open: 0.6rad, size: 0.5m)
  Claw(pose: body * claw_socket(-1), open: 0.4rad, size: 0.25m)
  for side in [-1, 1] {
    Eye(pose: body * eye_socket(side))
  }
}

Sand(level: 0m)
at = place([0,0.4,0]@World)
crab(CrabBody(pose: at, head: [0,0.1m,0.15m]@Body, vel: [0,0,0]@World, com: [0,0.4,0]@World), at)

# ---- schedule ----
schedule Main {
  run_laws(crab_step)
  check(upright)
  orbit([0,0.25,0]@World, 0.9m, 30deg)
  build_indirect(Sand, CrabLeg, CrabBody, Shell, Claw, Eye)
  render_bundle(Vis64) { draw_indirect(Sand, CrabLeg, CrabBody, Shell, Claw, Eye) }
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sky)
  tonemap_aces(ColorHdr, sky)
}
```

## Task 2: a reed bed in wind

```
space World

paper = [0.95, 0.91, 0.82]
ink_dark = [0.12, 0.10, 0.09]
ink_faint = [0.78, 0.80, 0.86]
sun = dir([0.4, 1.0, 0.3]@World)

lobe flat_ink(dark, cut, n, v, l) = if dot(n, l) < cut { dark } else { paper }

# ---- one reed's centreline at parameter t in 0..1 ----
fn reed_point(base: p3@World, height: m, lean: rad, stiff: f, phase: rad, t: f, now: s) -> p3@World {
  sway = 0.35rad * sin(0.9/m * base.x - 2.2/s * now + phase) / (1.0 + 3.0 * stiff)
  dx = height * (t * sin(lean) + t * t * sin(sway))
  dy = height * t * cos(lean)
  base + [dx, dy, 0.0m]
}

kind Reed @World (base: p3@World, height: m, lean: rad, stiff: f, phase: rad) {
  trace(p) = {
    a = reed_point(base, height, lean, stiff, phase, 0.0, time)
    b = reed_point(base, height, lean, stiff, phase, 0.5, time)
    c = reed_point(base, height, lean, stiff, phase, 1.0, time)
    min(min(capsule(p, a, b, 0.008m), capsule(p, b, c, 0.005m)), capsule(p, c - [0,0.05m,0], c, 0.009m))
  }
  material(p, n) = flat_ink(ink_dark, 0.15, n, dir([0,0,1]@World), sun)
}

kind Water @World (level: m) {
  trace(p) = p.y - level
  material(p, n) = flat_ink(ink_faint, -0.2, n, dir([0,0,1]@World), sun)
}

sky(rd) = paper * 0.97 + ink_faint * 0.03

# ---- build: 200 reeds in a loose cluster ----
fn jit(i: f, k: f) -> f = fract(sin(i * 12.9898 + k * 78.233) * 43758.5453)

Water(level: 0m)

for i in 0..200 {
  r = 1.0m * sqrt(jit(i, 2))
  th = 6.2831853rad * jit(i, 3)
  Reed(
    base: [r * cos(th), -0.05m, r * sin(th)]@World,
    height: 0.6m + 0.4m * jit(i, 4),
    lean: 0.5rad * jit(i, 5) - 0.25rad,
    stiff: jit(i, 6),
    phase: 6.2831853rad * jit(i, 7)
  )
}

# ---- schedule ----
schedule Main {
  orbit([0,0.5,0]@World, 3m, 20deg)
  build_indirect(Water, Reed)
  render_bundle(Vis64) { draw_indirect(Water, Reed) }
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sky)
  tonemap_aces(ColorHdr, sky)
}
```

## Task 3: a heron's hop

```
space World Body Head

sun = dir([0.4, 1.0, 0.3]@World)
neck_top = [0.0m, 0.95m, 0.10m]@Body
neck_limit = 0.9rad

# ---- fish: a curve in world space, its position is the gaze target ----
fn fish_path(t: s) -> p3@World =
  [1.2m * cos(0.3rad/s * t), -0.1m, 1.2m + 0.4m * sin(0.5rad/s * t)]

kind Fish @World (pos: p3@World, t: s) {
  trace(p) = sphere(p - pos, 0.06m)
  material(p, n) = ggx([0.6, 0.7, 0.8], 0.2, 0) + coat(0.5, 0.03)
}

law fish_swim(f: Fish, dt) {
  f.t = f.t + dt
  f.pos = fish_path(f.t)
}

# ---- how far the head is inside its neck limit (rad, negative when past it) ----
fn neck_room(pose: xf@Body>World, fish: p3@World) -> rad {
  head_at = pose * neck_top
  trunk = pose * dir([0,0,1]@Body)
  want = dir(fish - head_at)
  neck_limit - acos(clamp(dot(want, trunk), -1.0, 1.0))
}

# ---- body: turns and hops while the neck is at its limit ----
kind Heron @Body (anchor: p3@World, yaw: rad, air: m, phase: f, pose: xf@Body>World) {
  trace(p) = {
    trunk = capsule(p, [0,0.35m,0]@Body, [0,0.6m,0]@Body, 0.12m)
    leg_l = capsule(p, [0.08m,0.35m,0]@Body, [0.08m,0.0m,0]@Body, 0.012m)
    leg_r = capsule(p, [-0.08m,0.35m,0]@Body, [-0.08m,0.0m,0]@Body, 0.012m)
    n1 = capsule(p, [0,0.6m,0]@Body, [0.0m,0.72m,-0.06m]@Body, 0.03m)
    n2 = capsule(p, [0.0m,0.72m,-0.06m]@Body, [0.0m,0.84m,0.06m]@Body, 0.025m)
    n3 = capsule(p, [0.0m,0.84m,0.06m]@Body, neck_top, 0.02m)
    min(min(min(trunk, leg_l), min(leg_r, n1)), min(n2, n3))
  }
  material(p, n) = ggx([0.55,0.58,0.6], 0.4, 0) + coat(0.3, 0.1)
}

law heron_body(b: Heron, dt, fish: Fish) {
  stuck = neck_room(b.pose, fish.pos) < 0rad
  b.phase = if stuck { fract(b.phase + dt * 0.8/s) } else { 0.0 }
  b.air = 0.05m * sin(3.14159rad * b.phase)
  b.yaw = if stuck { b.yaw + 0.6rad/s * dt } else { b.yaw }
  b.pose = frame(b.anchor + [0, b.air, 0]@World, dir([sin(b.yaw), 0, cos(b.yaw)]@World), [0,1,0]@World)
}

# ---- neck and head: aim at the fish, clamped to the neck limit ----
kind Head @Head (parent: Heron, pose: xf@Head>World, aim: d3@World, room: rad) {
  trace(p) = min(sphere(p, 0.05m), capsule(p, [0,0,0.02m], [0,0,0.22m], 0.012m))
  material(p, n) = ggx([0.55,0.58,0.6], 0.4, 0) + coat(0.3, 0.1)
}

law head_aim(h: Head, dt, bodies: [Heron], fish: Fish) {
  b = bodies[h.parent]
  head_at = b.pose * neck_top
  trunk = b.pose * dir([0,0,1]@Body)
  want = dir(fish.pos - head_at)
  room = neck_room(b.pose, fish.pos)
  perp = dir(want - trunk * dot(want, trunk))
  limit_dir = dir(trunk * cos(neck_limit) + perp * sin(neck_limit))
  h.room = room
  h.aim = if room < 0rad { limit_dir } else { want }
  h.pose = frame(head_at, h.aim, [0,1,0]@World)
}

# ---- checks ----
check gaze_held(h: Head) {
  if !finite(h.pose) { return unknown(NonFinitePose) }
  if h.room < 0rad { missed(-h.room) } else { held(h.room) }
}

check above_hips(h: Head, bodies: [Heron]) {
  b = bodies[h.parent]
  hip = (b.pose * [0.0m, 0.45m, 0.0m]@Body).y
  margin = (b.pose * neck_top).y - hip
  if margin < 0m { missed(-margin) } else { held(margin) }
}

# ---- build ----
at = [0,0,0]@World
h = Heron(anchor: at, yaw: 0rad, air: 0m, phase: 0.0, pose: frame(at, [0,0,1]@World, [0,1,0]@World))
Head(parent: h, pose: frame(at + neck_top, [0,0,1]@World, [0,1,0]@World), aim: [0,0,1]@World, room: 0rad)
Fish(pos: fish_path(0s), t: 0s)

# ---- schedule: body, then neck and head, then checks ----
schedule Main {
  run_laws(fish_swim, heron_body, head_aim)
  check(gaze_held)
  check(above_hips)
  orbit([0,0.5,0]@World, 3m, 35deg)
  build_indirect(Heron, Head, Fish)
  render_bundle(Vis64) { draw_indirect(Heron, Head, Fish) }
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sky)
  tonemap_aces(ColorHdr, sky)
}
```
