## Task 1: hermit crab

```
space World Body Leg Claw Face Shell

-- Shell spine: a coiled spiral, metres
fn spine(u) = {0.14m * u * cos(22.0 * u), 0.10m * u, 0.14m * u * sin(22.0 * u)}@Shell

-- Rib modulation on the tube radius, v in 0..6.2832
fn rib(u, v) = 0.004m * (0.5 + 0.5 * cos(9.0 * v + 40.0 * u))

-- Frames transported along the spine so they do not twist
scan whorl u in 0..1 by 256 on bake_lut
  init = frame(spine(u), dir(spine'(u)), {0, 1, 0}@Shell)
  step prev = transport(prev, spine(u), dir(spine'(u)))
end

-- Shell as a ribbed tube whose radius grows with the spiral
surface(u, v) grid 64x32
  local fr = whorl(u)
  local r = 0.012m + 0.02m * u + rib(u, v)
  fr.o + r * (cos(v) * fr.n + sin(v) * fr.b)
end

-- Rounded palm, fixed finger, hinged moving finger with serrations
fn palm(p, s) = round_box(p, {s, s * 0.6, s * 0.5}, 0.2m * s)
fn fixed_finger(p, s) = round_box(p - {0, 0, s * 0.9}, {s * 0.25, s * 0.2, s * 0.8}, 0.1m * s)
fn teeth(p, s) = box(p - {0, 0, s * 1.4}, {s * 0.3, s * 0.1, s * 0.5})
fn moving_finger(p, s, open) = ssub(teeth(p, s), round_box(p |> move(-{0, 0, s * 0.9}) |> rot_z(open), {s * 0.25, s * 0.2, s * 0.8}, 0.1m * s))

kind Claw @Claw (open: rad, size: m, pose: xf@Claw>World)
  trace(p) = smin(palm(p, size), min(fixed_finger(p, size), moving_finger(p, size, open)))
end

kind CrabBody @Body (pose: xf@Body>World, head: p3@Body, com: p3@World, vel: v3@World)
  trace(p) = round_box(p, {0.12m, 0.04m, 0.10m}, 0.03m)
end

kind CrabLeg @Leg (parent: CrabBody, pose: xf@Leg>World, femur: m, tibia: m, foot: p3@World)
  trace(p) = smin(capsule(p, {0, 0, 0}, {femur, 0, 0}, 0.07m), capsule(p, {femur, 0, 0}, {femur + tibia * 0.5, -tibia, 0}, 0.05m))
end

-- Eyes on stalks: one definition, mirrored by side
kind Eye @Face (side: f, pose: xf@Face>World, look: v3@Face)
  trace(p) = capsule(p, {0, 0, 0}, {0, 0.30m, 0}, 0.02m)
end

fn socket(j, side) = move({0.10m * side, 0, 0.04m * (j - 2.5)}@Body)

fn crab(b, body)
  for j in 0..6, side in {-1, 1}
    CrabLeg{parent = b, pose = body * socket(j, side), femur = 0.40m + 0.05m * j,
            tibia = 0.30m - 0.02m * j, foot = body * {side, 0, 0.3 * j}@Body}
  end
end

-- Gaze target that drifts slowly in world space
fn gaze_at(t) = {0.3m * sin(0.2 * t), 0.2m, 1.2m}@World

law crab_step(leg: CrabLeg, dt, bodies: [CrabBody])
  local b = bodies[leg.parent]
  local cp = b.com + b.vel * 0.28s
  if dist(leg.foot, cp) > 0.5m then leg.foot = {cp.x, 0, cp.z} end
end

law aim_eyes(e: Eye, dt)
  local sym = e.pose |> mirror_x
  local eye_c = {0.03m, 0.30m, 0.07m}@sym
  local eye_w = e.pose * sym.out(eye_c)
  e.look = sym.into(inv(e.pose) * dir(gaze_at(time) - eye_w))
end

-- Layered wet chitin: red-orange base, subsurface, clear wet coat
material(p, n) = ggx({0.75, 0.22, 0.08}, 0.35 + 0.1 * fbm2(p.xz * 40), 0) + sss({0.9, 0.3, 0.1}, 0.08) + coat(0.7, 0.07)

-- Wet sand: a plain plane; noise lives in the material, not the distance field
fn ground(p) = p.y

-- Build: one crab, its rows, its eyes
local at = place({0, 0.4, 0}@World)
local body = CrabBody{pose = at, head = {0, 0.04, 0.10}@Body, com = {0, 0.4, 0}@World, vel = {0, 0, 0}@World}
crab(body, at)
Eye{side = 1, pose = at, look = {0, 0, 1}@Face}
Eye{side = -1, pose = at, look = {0, 0, 1}@Face}

-- Shell and claws as rows
Claw{open = 0.5rad, size = 0.09m, pose = at}
Claw{open = 0.3rad, size = 0.05m, pose = at}

schedule Main
  run_laws(crab_step)
  run_laws(aim_eyes)
  orbit({0, 0.4, 0}@World, 6m, 40deg)
  build_indirect(World, CrabLeg, Claw, Eye)
  render_bundle(Vis64) do
    draw_indirect(World, CrabLeg, Claw, Eye)
  end
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sun_sky)
  tonemap_aces(ColorHdr, sky_color)
end
```

## Task 2: reed bed in wind

```
space World

kind Reed @World (root: p3@World, height: m, taper: f, lean: rad, stiff: f, phase: f, bend: rad)
  trace(p) = smin(seg(p, 0.0, 1.0 / 3.0), smin(seg(p, 1.0 / 3.0, 2.0 / 3.0), seg(p, 2.0 / 3.0, 1.0)))
end

-- One stalk segment, from fraction a to b of the height; radius tapers toward the tip
fn seg(p, a, b) = capsule(p, pt(a), pt(b), 0.012m * (1.0 - 0.5 * a))

-- Point on the stalk at fraction s: lean at base, bend grows toward the tip
fn pt(s) = root_at + {0, height * s, 0} + {height * lean_at(s), 0, 0}

-- Build-time pseudo-random, deterministic per (i, k)
fn rnd(i, k) = fract(sin(i * 12.9898 + k * 78.233) * 43758.5453)

-- Two hundred reeds in a loose irregular cluster
for i in 0..200
  local ang = 6.2832 * rnd(i, 1)
  local rad = 3.0m * sqrt(rnd(i, 2))
  Reed{root = {rad * cos(ang), 0, rad * sin(ang)}@World,
       height = 0.8m + 0.5m * rnd(i, 3),
       taper = 0.6 + 0.3 * rnd(i, 4),
       lean = 0.05rad * (rnd(i, 5) - 0.5),
       stiff = rnd(i, 6),
       phase = 6.2832 * rnd(i, 7),
       bend = 0rad}
end

-- Wind: a gust travels across the bed; the bend grows toward the tip; stiff reeds bend less
law sway(r: Reed, dt)
  local gust = sin(1.5 * time - 0.6 * r.root.x + r.phase)
  local amp = 0.35rad * (0.6 + 0.4 * gust) / (1.0 + 3.0 * r.stiff)
  r.bend = amp
end

-- Look: two flat inks on warm paper, hard shadow edge
material(p, n) = lobe ink(paper_dark, 0.0, n, v, l)
lobe ink(dark, cut, n, v, l) = if dot(n, l) < cut then dark else {0.96, 0.92, 0.82} end

-- Water and sky: plain paper with a faint second ink
fn water(p) = p.y
material_water(p, n) = {0.96, 0.92, 0.82} + 0.03 * {0.2, 0.2, 0.3}

schedule Main
  run_laws(sway)
  build_indirect(World, Reed)
  render_bundle(Vis64) do
    draw_indirect(World, Reed)
  end
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, paper_ink)
  tonemap_aces(ColorHdr, sky_color)
end
```

## Task 3: heron's hop

```
space World Body Neck Head Leg

-- Fish: a row whose position is the gaze target, moving on a curve in world space
kind Fish @World (pos: p3@World)
  trace(p) = sphere(p, 0.05m)
end

fn fish_path(t) = {0.8m * cos(0.3 * t), 0.15m, 0.6m * sin(0.3 * t)}@World

-- Trunk: the body, with its heading and hip height
kind Heron @Body (pose: xf@Body>World, heading: rad, hips: p3@Body, head: K, hop: f, hopping: bool)
  trace(p) = round_box(p, {0.25m, 0.10m, 0.18m}, 0.06m)
end

kind HeronLeg @Leg (parent: Heron, pose: xf@Leg>World, femur: m, tibia: m, foot: p3@World)
  trace(p) = smin(capsule(p, {0, 0, 0}, {0, -femur, 0}, 0.02m), capsule(p, {0, -femur, 0}, {0, -femur - tibia, 0}, 0.015m))
end

-- Neck: an S-shaped chain, its curve in neck space
fn neck_pt(s) = {0.10m * sin(3.0 * s), 0.35m * s, 0.02m * (1.0 - s)}@Neck

kind Neck @Neck (parent: Heron, pose: xf@Neck>World)
  trace(p) = smin(capsule(p, neck_pt(0.0), neck_pt(0.5), 0.03m), capsule(p, neck_pt(0.5), neck_pt(1.0), 0.025m))
end

-- Head with eye, forward axis, and the neck's yaw limit
kind Head @Head (parent: Heron, pose: xf@Head>World, eye: p3@Head, fwd: d3@Head, yaw: rad, limit: rad, leftover: rad)
  trace(p) = smin(sphere(p, 0.06m), capsule(p, {0, 0, 0.04m}, {0, 0, 0.22m}, 0.012m))
end

-- Build: one heron, its legs, neck and head
local hd = Head{parent = K0, pose = place({0, 0.6m, 0}@World), eye = {0, 0.02m, 0.03m}@Head,
                fwd = {0, 0, 1}@Head, yaw = 0rad, limit = 1.1rad, leftover = 0rad}
local hr = Heron{pose = place({0, 0.6m, 0}@World), heading = 0rad, hips = {0, 0, 0}@Body,
                 head = hd, hop = 0.0, hopping = false}
HeronLeg{parent = hr, pose = place({0.08m, 0.6m, 0}@World), femur = 0.35m, tibia = 0.30m, foot = {0.08m, 0, 0}@World}
HeronLeg{parent = hr, pose = place({-0.08m, 0.6m, 0}@World), femur = 0.35m, tibia = 0.30m, foot = {-0.08m, 0, 0}@World}
Neck{parent = hr, pose = place({0, 0.6m, 0}@World)}
Fish{pos = fish_path(0)}

-- Law 1, the body: while the neck is at its limit, hop and turn the trunk toward the fish
law body_hop(b: Heron, dt, heads: [Head], fish: [Fish])
  local h = heads[b.head]
  local target = fish[0].pos
  if h.leftover > 0rad and not b.hopping then
    b.hopping = true
    b.hop = 0.0
  end
  if b.hopping then
    b.hop = b.hop + dt / 0.6s
    b.heading = b.heading + (dir(target - b.pose.pos).x) * 0.2 * dt / 0.6s
    if b.hop >= 1.0 then b.hopping = false end
  end
end

-- Law 2, the neck and head: the gaze lock holds each tick in world space; the head stops at its limit
law head_gaze(h: Head, dt, fish: [Fish], bodies: [Heron])
  local b = bodies[h.parent]
  local eye_w = h.pose * h.eye
  local aim = inv(h.pose) * dir(fish[0].pos - eye_w)
  local rel = dot(aim, h.fwd)
  local angle = acos(clamp(rel, -1.0, 1.0))
  local over = max(angle - h.limit, 0rad)
  h.leftover = over
  h.yaw = min(angle, h.limit)
end

-- Check 1: gaze is missed by the leftover angle when beyond the limit, else held
check gaze(h: Head)
  if h.leftover > 0rad then missed(h.leftover) else held(h.limit - h.yaw) end
end

-- Check 2: the head stays above the hips throughout
check head_above_hips(b: Heron, heads: [Head])
  local hd = heads[b.head]
  local margin = (hd.pose * hd.eye).y - (b.pose * b.hips).y
  if margin < 0 then missed(-margin) else held(margin) end
end

schedule Main
  run_laws(body_hop)
  run_laws(head_gaze)
  check(gaze)
  check(head_above_hips)
  orbit({0, 0.4, 0}@World, 3m, 30deg)
  build_indirect(World, Heron, HeronLeg, Neck, Head)
  render_bundle(Vis64) do
    draw_indirect(World, Heron, HeronLeg, Neck, Head)
  end
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sun_sky)
  tonemap_aces(ColorHdr, sky_color)
end
```
