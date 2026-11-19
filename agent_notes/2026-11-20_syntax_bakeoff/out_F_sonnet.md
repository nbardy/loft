## Task 1

```
space World Body Leg Claw Shell Eye Sand

kind Sand @Sand (pose: xf@Sand>World)
  trace(p) = p.y + 0.01m * fbm2(p.xz * 6)
  material(p, n) = ggx({0.45, 0.36, 0.25}, 0.2, 0) + coat(0.5, 0.05)
end

kind CrabBody @Body (pose: xf@Body>World, vel: v3@World, com: p3@World, head: p3@Body, gaze: p3@World)
  trace(p) = round_box(p, {0.18m, 0.07m, 0.14m}, 0.04m)
  material(p, n) = chitin(p, n)
end

kind CrabLeg @Leg (parent: CrabBody, pose: xf@Leg>World, femur: m, tibia: m)
  trace(p) = smin(capsule(p, {0, 0, 0}, {femur, 0, 0}, 0.025m),
                  capsule(p, {femur, 0, 0}, {femur + tibia, -0.05m, 0}, 0.018m), 0.02m)
  material(p, n) = chitin(p, n)
end

kind Claw[serrated: bool] @Claw (parent: CrabBody, pose: xf@Claw>World, size: f, open: rad)
  trace(p) =
    local q = p / size
    local palm = round_box(q, {0.10, 0.05, 0.07}, 0.03)
    local fixed = capsule(q, {0.08, 0, 0.02}, {0.26, 0, 0.0}, 0.025)
    local qm = rot_z(q - {0.08, 0, -0.02}, open)
    local moving = capsule(qm, {0, 0, 0}, {0.18, 0, 0.0}, 0.022)
    local teeth = if serrated then 0.006 * sin(qm.x * 120) else 0 end
    smin(smin(palm, fixed, 0.02), moving + teeth, 0.01) * size
  end
  material(p, n) = chitin(p, n)
end

kind Eye @Eye (parent: CrabBody, pose: xf@Eye>World, pupil: d3@Eye)
  trace(p) = min(capsule(p, {0, 0, 0}, {0, 0.10m, 0}, 0.008m), length(p - {0, 0.11m, 0}) - 0.02m)
  material(p, n) = ggx({0.05, 0.05, 0.05}, 0.1, 0) + coat(0.9, 0.02)
end

kind Shell @Shell (parent: CrabBody, pose: xf@Shell>World)
  surface(u, v) grid 64x32 =
    local s = whorl(u)
    local th = v * 6.2832rad
    local rib = 1 + 0.08 * sin(th * 14 + u * 40)
    local r = 0.02m * exp(1.6 * u) * rib
    s.o + (s.n * cos(th) + s.b * sin(th)) * r
  end
  material(p, n) = ggx({0.85, 0.72, 0.55}, 0.4, 0) + coat(0.4, 0.1)
end

fn spine(u) = {0.12m * exp(1.1 * u) * cos(u * 18rad), 0.05m * u * 4, 0.12m * exp(1.1 * u) * sin(u * 18rad)}

scan whorl u in 0..1 by 256 on bake_lut
  init = frame(spine(u), dir(spine'(u)), {0, 1, 0})
  step prev = transport(prev, spine(u), dir(spine'(u)))
end

fn chitin(p, n) = ggx({0.85, 0.28, 0.08}, 0.35 + 0.1 * fbm2(p.xz * 40), 0) + sss({0.9, 0.3, 0.1}, 0.08) + coat(0.7, 0.07)

fn socket(j, side) = place({side * 0.12m, -0.02m, 0.05m * j - 0.12m}@Body)

fn crab(at)
  local body = CrabBody{pose = at, vel = {0, 0, 0}, com = at * {0, 0, 0}@Body, head = {0, 0.1m, 0.15m}@Body, gaze = {0.6m, 0.3m, 0.8m}@World}
  for j in 0..6, side in {-1, 1}
    CrabLeg{parent = body, pose = at * socket(j, side), femur = 0.10m + 0.01m * j, tibia = 0.09m + 0.008m * j}
  end
  Claw[true]{parent = body, pose = at * place({0.16m, 0, 0.16m}@Body), size = 1.0, open = 0.5rad + 0.2rad * sin(time * 1s)}
  Claw[false]{parent = body, pose = at * place({-0.16m, 0, 0.14m}@Body), size = 0.55, open = 0.3rad}
  local q = at |> mirror_x |> move({-0.05m, 0.06m, 0.14m})
  local gaze = {0.6m + 0.2m * sin(time * 0.3s), 0.3m, 0.8m}@World
  local eye_c = {0.03, 0.0, 0.0}@q
  local eye_w = at * q.out(eye_c)
  local look = q.into(inv(at) * dir(gaze - eye_w))
  Eye{parent = body, pose = at * q.out(eye_c), pupil = look}
  Shell{parent = body, pose = at * place({-0.05m, 0.10m, -0.05m}@Body)}
  body
end

local ground = Sand{pose = place({0, 0, 0}@World)}
local at = place({0, 0.07m, 0}@World)
crab(at)

light sun(p) = {3.2, 3.0, 2.6} * 1.0
sky = {0.55, 0.65, 0.8}

schedule Main
  orbit({0, 0.1m, 0}@World, 0.9m, 25deg)
  build_indirect(Sand, CrabLeg, Claw)
  render_bundle(Vis64) do
    draw_indirect(Sand, CrabLeg, Claw)
  end
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sun_sky)
  tonemap_aces(ColorHdr, sky)
end
```

## Task 2

```
space World Reed Wet

kind Reed @Reed (base: p3@World, height: m, lean: rad, stiff: f, phase: f, seed: f)
  bend(h) = lean + (1 - stiff) * 0.35rad * h * h * (0.6 + 0.4 * sin(time * 1.3s + phase + base.x * 0.8 - base.z * 0.5))
  trace(p) =
    local q = p - base
    local h = clamp(q.y / height, 0, 1)
    local a = bend(h)
    local tip = {sin(a) * height * h, cos(a) * height * h, 0}
    local stalk = capsule(q, {0, 0, 0}, tip, 0.012m * (1 - 0.6 * h))
    local head = capsule(q, tip, tip + {sin(a) * 0.12m, cos(a) * 0.12m, 0}, 0.02m)
    min(stalk, head)
  end
  material(p, n) = ink(n)
end

kind Water @Wet (pose: xf@Wet>World)
  trace(p) = p.y
  material(p, n) = {0.93, 0.89, 0.8} + 0.04 * fbm2(p.xz * 3) * {0.3, 0.5, 0.6}
end

lobe ink(dark, cut, n, v, l) = if dot(n, l) < cut then dark else {0.93, 0.89, 0.78} end

fn hash1(x: f) -> f = wgsl[[ return fract(sin(x * 127.1) * 43758.5); ]]

fn bed()
  for i in 0..200
    local a = hash1(i * 1.0) * 6.2832rad
    local r = 1.2m * sqrt(hash1(i * 3.7)) * (0.7 + 0.5 * hash1(i * 9.1))
    Reed{base = {r * cos(a), 0, r * sin(a)}@World, height = 0.9m + 0.6m * hash1(i * 5.3),
         lean = (hash1(i * 7.7) - 0.5) * 0.3rad, stiff = hash1(i * 2.9) * 0.8,
         phase = hash1(i * 4.1) * 6.2832, seed = i * 1.0}
  end
end

Water{pose = place({0, 0, 0}@World)}
bed()

light sun(p) = {2.8, 2.6, 2.2}
sky = {0.93, 0.89, 0.8}

schedule Main
  orbit({0, 0.6m, 0}@World, 4m, 15deg)
  build_indirect(Water, Reed)
  render_bundle(Vis64) do
    draw_indirect(Water, Reed)
  end
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sun_sky)
  tonemap_aces(ColorHdr, sky)
end
```

## Task 3

```
space World Body Neck Head Ground

kind Heron @Body (pose: xf@Body>World, heading: rad, yaw: rad, head: rad, hop: f, air: m, hips: p3@Body,
                  eye: p3@Head, fwd: d3@Head, neck_limit: rad, turn_rate: rad, hop_t: f)
  trace(p) = smin(round_box(p, {0.10m, 0.07m, 0.22m}, 0.03m),
                  capsule(p, {0, 0.1m, 0.2m}, {0, 0.35m, 0.28m}, 0.02m), 0.04m)
  material(p, n) = ggx({0.55, 0.6, 0.65}, 0.5, 0)
end

kind Leg @Body (parent: Heron, side: f)
  trace(p) = capsule(p, {side * 0.05m, -0.05m, 0}, {side * 0.05m, -0.5m, 0.02m}, 0.012m)
  material(p, n) = ggx({0.8, 0.7, 0.3}, 0.5, 0)
end

kind HeadPart @Head (parent: Heron)
  trace(p) = min(length(p) - 0.04m, capsule(p, {0, 0, 0.03m}, {0, 0, 0.18m}, 0.012m))
  material(p, n) = ggx({0.9, 0.75, 0.2}, 0.4, 0)
end

kind Fish @World (t: f)
  path(t) = {1.5m * cos(t * 0.4), 0.0, 1.2m * sin(t * 0.4) + 1.0m}@World
  trace(p) = length(p - path(t)) - 0.04m
  material(p, n) = ggx({0.7, 0.75, 0.8}, 0.2, 0) + coat(0.6, 0.05)
end

fn wrap(a: rad) -> rad = a - 6.2832rad * floor((a + 3.1416rad) / 6.2832rad)

fn gaze_target(f: Fish) -> p3@World = {1.5m * cos(f.t * 0.4), 0.0, 1.2m * sin(f.t * 0.4) + 1.0m}@World

fn bearing(h: Heron, tgt: p3@World) -> rad =
  local d = inv(h.pose) * tgt
  atan2(d.x, d.z)
end

law neck_head(h: Heron, dt, fish: [Fish])
  local tgt = gaze_target(fish[0])
  local want = bearing(h, tgt)
  local off = wrap(want - h.yaw)
  h.head = clamp(off, -h.neck_limit, h.neck_limit)
end

law body_turn(h: Heron, dt, fish: [Fish])
  local tgt = gaze_target(fish[0])
  local want = wrap(bearing(h, tgt))
  local over = abs(want) - h.neck_limit
  if over > 0rad and h.hop_t <= 0 then h.hop_t = 1 end
  if h.hop_t > 0 then
    h.hop_t = h.hop_t - dt / 0.5s
    h.air = 0.12m * sin(clamp(1 - h.hop_t, 0, 1) * 3.1416)
    h.heading = h.heading + sign(want) * min(abs(over), h.turn_rate * dt)
    if h.hop_t <= 0 then h.air = 0m; h.hop_t = 0 end
  end
  h.pose = place({0, 0.5m + h.air, 0}@World) * rot_y(h.heading)
end

check gaze(h: Heron, fish: [Fish])
  if not finite(h.pose) then return unknown(NonFinitePose) end
  local want = bearing(h, gaze_target(fish[0]))
  local left = abs(wrap(want - h.head)) 
  local over = abs(wrap(want)) - h.neck_limit
  if over > 0rad then missed(over) else held(h.neck_limit - abs(wrap(want))) end
end

check head_above_hips(h: Heron)
  local margin = (h.pose * h.eye).y - (h.pose * h.hips).y
  if margin < 0m then missed(-margin) else held(margin) end
end

local bird = Heron{pose = place({0, 0.5m, 0}@World), heading = 0rad, yaw = 0rad, head = 0rad, hop = 0, air = 0m,
                   hips = {0, 0, -0.1m}@Body, eye = {0, 0.02m, 0.03m}@Head, fwd = dir({0, 0, 1}),
                   neck_limit = 70deg, turn_rate = 4rad, hop_t = 0}
Leg{parent = bird, side = -1}
Leg{parent = bird, side = 1}
HeadPart{parent = bird}
Fish{t = 0}

light sun(p) = {3.0, 2.8, 2.4}
sky = {0.6, 0.7, 0.85}

schedule Main
  run_laws(body_law, body_turn)
  run_laws(neck_law, neck_head)
  check(gaze)
  check(head_above_hips)
  orbit({0, 0.5m, 0.5m}@World, 4m, 30deg)
  build_indirect(Ground, Heron, Leg, HeadPart, Fish)
  render_bundle(Vis64) do
    draw_indirect(Ground, Heron, Leg, HeadPart, Fish)
  end
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sun_sky)
  tonemap_aces(ColorHdr, sky)
end
```
