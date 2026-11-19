## 1. Forkable Example Engines (Unified E+ v1.0)

### 1.1 `engines/realtime.loft`

```loft
use std::light::*
use std::post::*
use std::sched::*

pub fn direct_plus_refl(w: World, h: Hit, wo: d3@World, ink: bool, px: v2) -> rgb {
  var rng = rng_key(px, Frame.idx, 0)
  let m = material(w, h)
  emit_of(m, wo) + direct_mis(w, h, wo, rng) + trace_refl(w, h, wo, rng)
}

pub schedule realtime(w: World, cam: Camera, sun_cam: Option, shade_fn = direct_plus_refl, history: bool = false) {
  if_some(sun_cam, find_shadow_map(w, sun_cam, 2048))
  cull_indirect(w, cam)
  find_vis(w, cam, 4)
  tile_classify(VisBuf, 8)
  shade_tiles(VisBuf, shade_fn)
  screen_pass(inscatter_pass(w, cam, DepthBuf, 24), PremulOver)
  if_true(history, temporal_accum(HdrBuf, VisBuf, PrevHdr, PrevVis))
  bloom_mip(HdrBuf, 1.2, 0.18, 3.0)
  tonemap_aces(HdrBuf, Sky)
}

```

### 1.2 `engines/reference.loft`

```loft
use std::light::*
use std::post::*

fn path_sample(w: World, cam: Camera, px: v2, bounces: u) -> v4 {
  var rng = rng_key(px, Frame.sample, 0)
  var ray = camera_ray(cam, px + rand2(rng), rand2(rng), rand1(rng))
  var l_acc = [0, 0, 0]
  var t_acc = [1, 1, 1]
  var allow_emit = true
  for b in 0..bounces {
    let h = trace(w, ray)
    let vol = inscatter_joint(w, ray, if h.t >= 0m { h.t } else { 1e6m }, 32, rng)
    l_acc = l_acc + t_acc * vol.l
    t_acc = t_acc * vol.t
    if h.t < 0m { l_acc = l_acc + t_acc * sky(w, ray.d); break }
    let m = material(w, h)
    if allow_emit { l_acc = l_acc + t_acc * emit_of(m, -ray.d) }
    let lt = pick_light(w, h.p, rng)
    let vis = visible(w, h.p + h.n * 0.0001m, lt.p)
    l_acc = l_acc + t_acc * eval_bsdf(m, h.n, -ray.d, lt.wi) * lt.l * vis / max(lt.pdf, 1e-6)
    let sc = sample_bsdf(m, h.n, -ray.d, rng)
    if sc.pdf <= 0 { break }
    t_acc = t_acc * sc.weight
    allow_emit = sc.delta
    ray = make_ray(h.p + h.n * (if dot(sc.wi, h.n) > 0 { 0.0001m } else { -0.0001m }), sc.wi)
  }
  [l_acc.x, l_acc.y, l_acc.z, 1]
}

pub schedule reference(w: World, cam: Camera, bounces: u = 16) {
  screen_pass(path_sample(w, cam, Frame.px, bounces), Additive)
  resolve_aces(SumF32)
}

```

### 1.3 `engines/woodblock.loft`

```loft
use std::light::*
use std::stylised::*
use std::post::*

pub fn woodblock_shade(w: World, shadow_w: World, h: Hit, wo: d3@World, ink: bool, px: v2) -> rgb {
  let m = material(w, h)
  let sun = light(w, 0, h.p, rng_key(px, 0, 0))
  let vis = visible(shadow_w, h.p + h.n * 0.001m, sun.p).r
  let ndl = max(dot(h.n, sun.wi), 0) * vis
  let band = cel_bands(ndl, [0.08, 0.42, 0.78], [0.22, 0.55, 0.85, 1.05], 0.03)
  let wash = bokashi(h.uv.y, 0.15, [0, 0.65], 0.35)
  let htch = cross_hatch(px, ndl, 5.0, 0.25)
  let col = overprint(bsdf_albedo(m, max(dot(h.n, wo), 0)) * band, [0.08, 0.12, 0.22], wash * 0.45 + htch * 0.30)
  if ink { [0.05, 0.05, 0.07] } else { col }
}

pub schedule woodblock(w: World, shadow_w: World, cam: Camera) {
  cull_indirect(w, cam)
  find_vis(w, cam, 4)
  tile_classify(VisBuf, 8)
  shade_tiles(VisBuf, woodblock_shade(w, shadow_w))
  outline_and_paper(PrintBuf, VisBuf, DepthBuf, [0.05, 0.05, 0.07], 1.5, 0.04, 0.03)
}

```

### 1.4 `engines/roam.loft` & `engines/act.loft` (Motion Engines)

```loft
# engines/roam.loft — Spore-like adaptive locomotion over slice-indexed BODY/STATE/POSE
use std::motion::*

pub fn default_turn(yaw: rad, lim: rad, tc: s, latch: bool) -> { rate: rad/s, latch: bool } {
  let r = abs(yaw) / max(lim, 0.1rad)
  let next_latch = if latch { r > 0.25 } else { r >= 0.85 }
  let rate = if next_latch { sign(yaw) * 3.8rad/s * clamp(r - 0.25, 0, 1) } else { 0rad/s }
  { rate: rate, latch: next_latch }
}

pub law roam_step(c: CreatureState, dt, bodies: [BodyRow], chains: [ChainRow], surfs: [SurfaceRow], inp: RoamInput)
  writes c.{trunk, feet, gaze}
{
  let b = bodies[c.body_id]
  if inp.hit.active { apply_impulse(c.trunk, b, inp.hit) }
  let g_eff = [0, -9.81, 0]@World - mean_pin_accel(c.feet, surfs)
  let hull = support_hull(c.feet, g_eff)
  let a_want = (inp.go - c.trunk.vel) / (b.tc * 0.5)
  let a_clmp = clamp_accel_support(c.trunk, hull, a_want, b, g_eff)
  let p_star = c.trunk.pos - a_clmp * (b.rest_h / 9.81)
  let shares = load_shares_nnls(c.feet, p_star)
  let cp = capture_point_ff(c.trunk, b.rest_h, a_clmp, mean_pin_vel(c.feet, surfs), mean_pin_accel(c.feet, surfs), g_eff)
  step_feet_and_slips(c.feet, chains, b, c.trunk, cp, p_star, shares, g_eff, dt)
  if inp.has_look { step_gaze_and_handoff(c.gaze, c.trunk, b, hull, inp.look, default_turn) }
  integrate_trunk_and_leash(c.trunk, c.feet, chains, a_clmp, dt, 4)
  if support_margin(hull, cp) > 0m { c.trunk.composure = saturate(c.trunk.composure + 0.55 * (dt / 1s)) }
}

# engines/act.loft — Disney-principled performance (composes roam + compliance-coupled priors + squash)
pub law act_step(c: CreatureState, dt, bodies: [BodyRow], chains: [ChainRow], surfs: [SurfaceRow], score: BeatScore, inp: RoamInput)
  writes c.{trunk, feet, gaze, manips}
{
  roam_step(c, dt, bodies, chains, surfs, inp)
  let alpha_t = score.trunk.compliance / max(c.trunk.composure * c.trunk.composure, 0.002)
  for it in 0..6 {
    solve_prior_xpbd(c.trunk, score.trunk, if c.trunk.composure < 0.15 { 1e6 } else { alpha_t })
    solve_reach_leash(c.trunk, c.feet, chains, 0)
  }
  apply_end_beats(c.manips, score.ends)
  step_volume_squash(c.trunk, dt, 3.2, 0.42)
}

```

## 2. Three Reference Scenes (Unified E+ v1.0)

### 2.1 Scene 1: Hermit Crab Boss (Hit 1 + Slip + Chained Hit 2)

```loft
use std::geom::*
use std::light::*
use std::motion::*
use engines::act::act_step
use engines::realtime::realtime

space World Body Shell Claw Face Leg

fn chitin(p, n) =
  ggx([0.68, 0.14, 0.07], 0.20 + 0.22 * grad_noise3(coords(p) * 6.5), 0.05)
  + sss([0.9, 0.25, 0.08], 0.08m) + coat(0.88, 0.06)

fn spine(t: rad) = { let e = exp(0.18 * (t / 1rad)); [e * 0.95 * cos(t), e * 0.95 * sin(t), -e * 0.72]@Shell * 1m }
scan whorl t in 0rad..28rad by 256 on bake_lut {
  init = frame(spine(t), dir(spine'(t)), [0, 0, 1])
  step prev = transport(prev, spine(t), dir(spine'(t)))
}

kind HermitShell @Shell (body: BossBody, pose: xf@Shell>World, aggro: f in 0..1) {
  fn macro_s(t: rad, a: rad) = {
    let f = whorl(t)
    let s = exp(0.18 * (t / 1rad)) * 0.62m
    let rib = 0.08 * pow(0.5 + 0.5 * cos(6 * t), 4) * (0.4 + 0.6 * sin(5 * a))
    f.o + s * (1.15 + rib) * cos(a) * f.n + s * (0.82 + rib) * sin(a) * f.b
  }
  fn barn_h(t: rad, a: rad) = 0.035m * smooth_voronoi_cone([t / 1rad * 2.1, a / 1rad * 3.0, 0], 0.12)
  surface(u, v) grid 96x32 = {
    let t = 28rad * u; let a = 6.2831853rad * v
    let p0 = macro_s(t, a); let n0 = dir(cross(diff(macro_s, t)(t, a), diff(macro_s, a)(t, a)))
    p0 + n0 * barn_h(t, a)
  }
  material(p, n) = {
    let grime = saturate(0.4 + 0.35 * grad_noise3(coords(p) * 4))
    ggx(mix([0.62, 0.48, 0.36], [0.09, 0.21, 0.14], grime), mix(0.28, 0.78, grime), 0)
    + coat(1 - 0.85 * grime, 0.08) + film(0.25 * (1 - grime), 1.42, 380)
  }
}

kind CrusherClaw @Claw (pose: xf@Claw>World, open: rad, scale: f in 0.3..1.5, aggro: f in 0..1) {
  trace(p) = {
    let q = p / scale
    let palm = ssub(round_box(q, [0.65m, 0.35m, 0.95m], 0.28m), box(q - [0.1m, -0.35m, 0.2m], [0.7m, 0.3m, 0.9m]), 0.06m)
    map ring = q |> repeat_polar(7)
    let studs = sphere(ring.p - ring.pt(0.75m, 0, 0), 0.11m)
    map jaw = q |> move([0, 0.22m, -0.85m]) |> rot_x(-open)
    let teeth = 0.045m * sin(26 * (jaw.p.z / 1m)) * smootherstep(0.1m, 0.9m, jaw.p.z)
    let dactyl = capsule(jaw.p, jaw.pt(0, 0, 0), jaw.pt(0, -0.15m, 1.15m), 0.24m) - teeth
    scale * smin(smin(palm, dactyl, 0.08m), studs, 0.04m)
  }
  material(p, n) = {
    let veins = smootherstep(0.62, 0.78, smooth_voronoi3(coords(p) * 5.2)) * aggro
    chitin(p, n) + emit_lobe([1.0, 0.32, 0.04] * veins * (3.5 + 1.5 * sin(8 * (time / 1s))))
  }
}

kind CrabFace @Face (pose: xf@Face>World, aggro: f in 0..1, gaze: target@World) {
  map sym(p) = p |> mirror_x |> move([-0.42m, -0.55m, -1.18m])
  trace(p) = {
    let sway = 0.08m * sin(2.1 * (time / 1s))
    let eye_c = sym.pt(sway, 0.24m, 0.16m)
    map brow = sym.p |> move([0, -0.28m, -0.05m]) |> rot_z(-0.35rad * aggro) |> rot_x(0.28rad * aggro)
    smin(smin(capsule(sym.p, sym.pt(0, -0.2m, 0), eye_c, 0.09m), sphere(sym.p - eye_c, 0.135m), 0.05m),
         wedge(brow.p, [0.32m, 0.10m, 0.38m]), 0.03m)
  }
  material(p, n) = {
    let eye_c = sym.pt(0.08m * sin(2.1 * (time / 1s)), 0.24m, 0.16m)
    let eye_w = pose * sym.out(eye_c)
    let look = sym.in(inv(pose) * dir(gaze.pos - eye_w))
    let is_pupil = dist(sym.p, eye_c) < 0.14m && dot(sym.in(n), look) > 0.90
    if is_pupil { ggx([0.02, 0.02, 0.02], 0.08, 0) + emit_lobe([1.0, 0.75, 0.10] * (1.2 + 3 * aggro)) }
    else { chitin(p, n) }
  }
}

kind CrabLeg @Leg (body: BossBody, chain_idx: u, r0: m, r1: m) {
  visible(ro, rd) = pose_chain_hit(body, chain_idx, ro, rd, r0, r1)
  material(p, n) = chitin(p, n)
}

body BossBody {
  trunk { mass: 420kg, inertia: [180, 240, 190], rest_h: 1.35m, mu: 0.65, strength: 2.2 }
  build for i in 0..8 {
    let s = if i >= 4 { 1 } else { -1 }; let z = (-0.75 + 0.5 * (i % 4)) * 1.4m
    chain leg[i]: support { socket: [s * 0.95m, -0.1m, z]@Body, lens: [0.55m, 1.25m, 1.35m, 0m],
                            rest: [s * 2.3m, -1.35m, z * 1.3]@Body, mu_s: 0.65, mu_k: 0.35 }
  }
  chain clawR: manip { socket: [0.8m, 0.1m, 1.1m]@Body, lens: [0.95m, 1.15m, 0m, 0m] }
}

state BossAnt checkpoint 60 { pos: array[p3@World, 64], prev: array[p3@World, 64] }

solver BossAntennae over BossAnt writes BossAnt.{pos, prev}
  config { strategy: colored_gs(2), substeps: 4, iters: 6, shared_mem: true }
{
  let root_w = pose_joint(body_id, 0).pos + [0.25m, 0.48m, 1.25m]@World
  pos[0] = root_w; prev[0] = root_w
  for i in 1..16 {
    let cur = [sin(time / 1s + 0.4 * i), 0.2, cos(0.7 * (time / 1s))]@World * 1.4
    let v = (pos[i] - prev[i]) / dt; prev[i] = pos[i]; pos[i] = pos[i] + (v * 0.96 + cur * dt) * dt
  }
  constrain edge(i, j) in 0..15 { solve_dist_edge(pos[i], pos[j], if i == 0 { 0 } else { 5 }, 5, 0.18m, 0.002) }
}

check boss_truth(c: CreatureState, surfs: [SurfaceRow]) -> m {
  for i in 0..8 { if c.feet[i].mode == 0 { yield check_pin_drift(pose_end(c.body_id, i), c.feet[i].pin, surfs, 0.5mm) } }
  yield check_posture(c.body_id, c.trunk)
}

build {
  emit Plane(n: [0, 1, 0]@World, d: 0m)
  emit Sun(dir: dir([-0.4, 0.85, 0.35]@World), color: [2.8, 2.6, 2.3])
  emit Sky(zenith: [0.35, 0.56, 0.88], horizon: [0.82, 0.86, 0.90])
  build for c in 0..1 {
    let bid = emit BossBody(origin: [c * 6m, 0m, 0m]@World)
    emit HermitShell(body: bid, pose: xform_id(), aggro: 0.85)
    emit CrusherClaw(pose: place([0.8m, 0.6m, 1.1m]@World), open: 0.45rad, scale: 1.0, aggro: 0.85)
    emit CrusherClaw(pose: place([-0.8m, 0.5m, 1.0m]@World), open: 0.20rad, scale: 0.62, aggro: 0.85)
    emit CrabFace(pose: xform_id(), aggro: 0.85, gaze: [0, 1.8m, 8m]@World)
    build for l in 0..8 { emit CrabLeg(body: bid, chain_idx: l, r0: 0.16m, r1: 0.05m) }
  }
}

schedule Main {
  act_step(hit_events: [hit(0.50s, [0, 1.1m, 0]@World, [-1800, 0, -900]),
                        hit(0.625s, [0.5m, 1.2m, 0]@World, [-1400, -300, -600])])
  run_solver(BossAntennae)
  check(boss_truth)
  realtime(SceneWorld, MainCam, SunCam)
}

```

### 2.2 Scene 2: Heron Hunting (Effort+Jerk Plan, Gaze Lock, Neck Limit → Hop)

```loft
use std::geom::*
use std::motion::*
use engines::act::act_step
use engines::realtime::realtime

space World Body

body HeronBody {
  trunk { mass: 2.2kg, inertia: [0.18, 0.12, 0.22], rest_h: 0.85m, mu: 0.9, strength: 2.02 }
  build for i in 0..2 {
    let s = if i == 1 { 1 } else { -1 }
    chain leg[i]: support { socket: [s * 0.18m, -0.15m, -0.08m]@Body, lens: [0.45m, 0.48m, 0m, 0m],
                            rest: [s * 0.18m, -0.85m, 0m]@Body, mu_s: 0.9, mu_k: 0.6 }
  }
  chain neck: gaze { socket: [0, 0.18m, 0.24m]@Body, lens: [1.55m, 0m, 0m, 0m], yaw_lim: [-1.9rad, 1.9rad] }
}

plan HeronHop(keys: PriorSpline, rail: Box, block: SurfaceRow) -> PriorTable {
  minimize integral(1.0 * sq_norm(acc - [0, -9.81, 0]) + 1e-7 * sq_norm(jerk) + 8.0 * sq_norm(pos - keys.pos))
  subject_to { clear(rail, 0.08m); land_on(block); pitch_in(-15deg, 15deg); flight_ballistic() }
}

fn fish_at(t: s) -> p3@World = [2.4m * cos(1.0rad/s * t), 0.05m, 2.4m * sin(1.0rad/s * t)]@World

check heron_truth(c: CreatureState, surfs: [SurfaceRow]) -> m {
  for i in 0..2 { if c.feet[i].mode == 0 { yield check_pin_drift(pose_end(c.body_id, i), c.feet[i].pin, surfs, 0.5mm) } }
  yield check_s_neck_length(c.body_id, 1.55m, 0.2mm)
  yield check_posture(c.body_id, c.trunk)
}

check heron_gaze(c: CreatureState) -> rad {
  check_gaze_honest(c.body_id, fish_at(time), c.gaze, !any_planted(c.feet))
}

schedule Main {
  act_step(prior: HeronHop, look: fish_at(time))
  check(heron_truth, heron_gaze)
  realtime(HeronWorld, MainCam, SunCam)
}

```

### 2.3 Scene 3: Non-PBR Coral Shrine (Woodblock + Dual Shadow World)

```loft
use std::geom::*
use std::stylised::*
use engines::woodblock::woodblock

space World Shrine

kind FoldedLantern @Shrine (pose: xf@Shrine>World) {
  trace(p) = {
    map k = p |> fold_kifs(4, 1.45, [0.42m, 0.28m, 0.35m], rot_y(0.55rad))
    max(round_box(k.p, [0.32m, 0.45m, 0.32m], 0.04m) * k.stretch, -cylinder(p, 0.28m, 1.2m))
  }
  material(p, n) = emit_lobe(mix([0.86, 0.24, 0.14], [0.95, 0.88, 0.72], smootherstep(0m, 1.8m, p.y)))
}

kind CoralBranch @World (a: p3@World, b: p3@World, ra: m, rb: m, level: f) {
  visible(ro, rd) = ray_round_cone(ro, rd, a, b, ra, rb)
  material(p, n) = emit_lobe(mix([0.14, 0.26, 0.44], [0.88, 0.32, 0.22], level))
}

world PrimaryWorld { things: [Plane, FoldedLantern, CoralBranch] }
world SolidShadow  { things: [Plane, SolidLanternProxy, CoralBranch] }

schedule Main { woodblock(PrimaryWorld, SolidShadow, MainCam) }

```

---
