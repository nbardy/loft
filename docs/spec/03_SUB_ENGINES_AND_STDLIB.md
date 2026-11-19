## 1. Engine Composition Contract (Separate, Compatible Engines)

```text
  [Host Inputs] ──► [Motion Engine: roam | act | clip | custom]
                         │  writes POSE (32B/joint) OR custom row table + VerdictBuf
                         ▼
               [Plain GPU Storage Tables] ◄── (zero coupling; runnable headless)
                         │
                         ▼
  [World + Cam] ──► [Render Engine: realtime | woodblock | reference | custom]
                         │  reads POSE / rows; calls 6 World queries + Light Estimators
                         ▼
                    [Swapchain]

```

## 2. Render & Light Standard Library (`std::geom`, `std::light`, `std::stylised`, `std::post`, `std::sched`)

```loft
# --- std::geom: Analytic Kinds (exact hit + bound + L=1) ---
# Plane(n: n3@World, d: m), Sphere(pose, r: m), Box(pose, half: v3), Capsule(a, b, r: m),
# Cone(a, b, ra: m, rb: m), Washer(pose, r0: m, r1: m), Torus(pose, major: m, minor: m),
# Mesh(pose, mesh_id: u), BrickField(pose, brick_id: u), Specks(count: u, buf: u, size: m)

# --- std::geom: SDF Primitives & Ops (L derived, smoothness tagged) ---
fn sphere(p, r: m) -> m                                       # Cinf, L=1
fn box(p, half) -> m                                          # C0, L=1
fn round_box(p, half, r: m) -> m                              # C1, L=1
fn capsule(p, a, b, r: m) -> m                                # C1, L=1
fn round_cone(p, a, b, r1: m, r2: m) -> m                     # C1, L=1
fn torus(p, major: m, minor: m) -> m                          # C1, L=1
fn cylinder(p, r: m, half_h: m) -> m                          # C0, L=1
fn wedge(p, half) -> m                                        # C0, L=1
fn ellipsoid(p, r) -> m                                       # Cinf, L=max(r)/min(r)
fn smin(a: m, b: m, k: m) -> m                                # C1 quadratic smooth union
fn smin_cubic(a: m, b: m, k: m) -> m                          # C2 cubic smooth union
fn smin_exp(a: m, b: m, k: m) -> m                            # Cinf exponential smooth union
fn ssub(a: m, b: m, k: m) -> m                                # C1 smooth subtraction
fn shell(d: m, thick: m) -> m                                 # C0 hollow shell
fn ray_capsule(ro, rd, a, b, r: m) -> f                       # closed-form hit t (-1 miss)
fn ray_round_cone(ro, rd, a, b, ra: m, rb: m) -> f            # closed-form hit t (-1 miss)
fn hit_min(a: f, b: f) -> f                                   # nearest positive t

# --- std::geom: Domain Maps (introduce scoped value space) ---
# mirror_x(p), mirror_plane(p, n), move(p, delta), rot_x(p, a), rot_y(p, a), rot_z(p, a),
# repeat_grid(p, period, lim), repeat_polar(p, count), fold_kaleidoscope(p, planes, iters),
# fold_screw(p, pitch, count), fold_spiral(p, arms, rate), fold_kifs(p, iters, scale, off, rot),
# warp_twist(p, rate), warp_bend(p, rate), warp_custom(p, fwd, inv_opt)

# --- std::geom: Frames, Surfaces, Noise & Assets ---
fn frame(o, t, up) -> Frame                                   # orthonormal seed frame
fn transport(prev, p, t) -> Frame                             # double-reflection RMF step
fn grad_noise2(p: v2) -> f                                    # C2, L=2.0
fn grad_noise3(p: v3) -> f                                    # C2, L=2.45
fn wave_noise3(p: v3) -> f                                    # Cinf, L=1/sqrt(3) (exact certificate)
fn fbm2(p: v2) -> f                                           # C2 3-octave FBM
fn smooth_voronoi_cone(p: v3, k: f) -> f                      # C1, L=1.0
fn smooth_voronoi3(p: v3) -> f                                # C1, L=1.2
build fn import_gltf(path: str) -> MeshAsset
build fn export_gltf(path: str, pose: TableRef)
build fn bake_brick(field_fn, bounds, res: u) -> BrickAsset

# --- std::light: 6 World Queries, Open Bsdf & Transport ---
fn trace(w: World, r: Ray) -> Hit                             # nearest hit across all kinds in w
fn visible(w: World, a: p3@World, b: p3@World) -> rgb         # any-hit + RGB media transmittance
fn material(w: World, h: Hit) -> Bsdf                         # open surface BSDF
fn light(w: World, idx: u, p: p3@World, rng) -> Lit           # sample emitter idx (0 = sun)
fn pick_light(w: World, p: p3@World, rng) -> Lit              # power/RIS emitter pick
fn sky(w: World, rd: d3@World) -> rgb                         # environment radiance
fn medium(w: World, p: p3@World) -> Sigma                     # summed overlapping media

fn ggx(albedo: rgb, rough: f, metal: f) -> Bsdf               # Kulla-Conty energy-compensated GGX
fn coat(weight: f, rough: f) -> Bsdf                          # dielectric clearcoat (attenuates below)
fn film(weight: f, ior: f, nm: f) -> Bsdf                     # Belcour-Barla thin-film iridescence
fn sheen(color: rgb, rough: f) -> Bsdf                        # Charlie grazing microfiber sheen
fn sss(color: rgb, radius: m) -> Bsdf                         # Burley dipole (realtime) / random walk (ref)
fn transmission(ior: f, rough: f, tint: rgb) -> Bsdf          # dielectric refraction (delta if rough=0)
fn emit_lobe(radiance: rgb) -> Bsdf                           # emissive lobe (registers emitter)
fn direct_mis(w: World, h: Hit, wo: d3@World, rng) -> rgb     # 1-sample MIS direct light
fn trace_refl(w: World, h: Hit, wo: d3@World, rng) -> rgb     # 1-bounce glossy reflection
fn inscatter_joint(w: World, r: Ray, t_max: m, steps: u, rng) -> Vol # unified RGB (L, T)

# --- std::stylised & std::post ---
fn cel_bands(lit: f, cuts: v3, levels: v4, soft: f) -> f
fn bokashi(u: f, foot: f, under: v2, amt: f) -> f
fn cross_hatch(px: v2, lit: f, spacing: f, deep: f) -> f
fn overprint(base: rgb, ink: rgb, cover: f) -> rgb
fn outline_vis(vis, depth, px: v2, ink: rgb, width: f) -> v4
fn tonemap_aces(hdr, sky)                                     # ACES Filmic display pass
fn bloom_mip(hdr, thresh: f, gain: f, radius: f)              # dual-filter mip bloom
fn paper_grain(c: rgb, px: v2, fibre: f, tooth: f) -> rgb

```

## 3. Motion Formulation & Standard Library (`std::motion`)

**Formulation:** Compliance-coupled dynamics ($\alpha = 1/(k\Delta t^2)$) inside a fused 4-stage fixed-order tick (`dt = 1/120 s`), plus optional build-time horizon `plan` (effort + jerk + `prefer near`, fixing the `67°`/`4.8 g` least-jerk heron failure):

1. **Stage A (Arbiter):** Moving-surface pin frames (`SurfacePin`), friction-cone slip ($\|f_t\| > \mu_s f_n$), **feedforward-acceleration capture point** $p_{\text{cp}} = p_{\text{com}} + \tau(v - v_s) + \frac{1}{2}\tau^2(a_{\text{cmd}} - a_s)$ (cuts `0.36 s` crab reversal lag to ≤ `0.08 s` **predicted**), 2D support-polygon margin (`Flight|Point|Segment|Polygon`), continuous nonnegative least-norm load shares, greedy strain-ordered lifts, unwrapped Schmitt-trigger neck handoff (`0.85` latch / `0.25` release).
2. **Stage B (Coupled Trunk + Pin Core):** Fixed symmetric Gauss-Seidel XPBD (`4..8` iters in registers, zero atomics). Authored priors (keys, `plan`, MotionBricks `PosePacket`s) enter at $\alpha_{\text{prior}} / \max(\text{composure}^2, 0.002)$; planted pins enter at $\alpha = 0$. Hit 1 drops `composure` and triggers shear slip; Hit 2 compounds natively on the narrowed support hull.
3. **Stage C (Post-Trunk IK + `present`):** Constant-accel span at frame fraction $\Delta \in [0, dt)$, closed-form 2-bone / 3-bone / constant-length $C^1$ S-neck IK (or clamped FABRIK for arbitrary editor trees) in the gripped surface's frame $\to$ writes `POSE` (or custom output table) + post-IK `Verdict`s.
4. **Stage D (Secondary & Juice):** Exact 2×2 analytic springs or 2-color shared-memory XPBD chains (`checkpoint 60`) + volume-preserving squash ($\det S = 1$).

**Trade-offs given up:** Approximate simultaneous multi-contact load sharing (NNLS + sym-GS instead of full LCP); one-way secondary feedback; anticipation requires a future beat/target (instant player input uses frame-1 lean + feedforward lift).

```loft
# --- std::motion: Stateless Biomechanics Primitives (CPU + GPU) ---
fn body_com(parts) -> MassProps                               # parallel-axis CoM & inv inertia
fn capture_point_ff(t, h: m, a_cmd, v_s, a_s, g_eff) -> p3@World
fn support_hull(feet, g_eff) -> SupportHull                   # Flight | Point | Segment | Polygon (<=16)
fn support_margin(hull, pt: p3@World) -> m
fn support_margin_without(feet, skip: u, pt: p3@World, g_eff) -> m
fn clamp_accel_support(t, hull, a_want, b, g_eff) -> v3@World
fn load_shares_nnls(feet, p_star: p3@World) -> array[f, 16]   # continuous >= 0 load shares
fn quintic_plan(d, v1, dur: s, t: s) -> KinSample             # boundary-condition anticipation (V > 2.5D)
fn solve_reach_leash(t, feet, chains, alpha: f) -> f          # XPBD hard pin reach leash
fn solve_prior_xpbd(t, prior, alpha: f) -> f                  # compliance-coupled prior pull
fn solve_dist_edge(a, b, im_a, im_b, rest: m, alpha: f) -> f  # XPBD chain edge constraint
fn solve_2bone_ik(root, tgt, l1: m, l2: m, pole) -> Pose3
fn solve_3bone_ik(root, tgt, lens: v3, pole) -> Pose4
fn solve_s_neck_ik(sh, head, len: m, segs: u) -> NeckRes      # constant-length 2-arc S-curve
fn solve_fabrik_clamped(pts, n: u, tgt, lim) -> f             # arbitrary N-link editor chains
fn step_spring_exact(x: f, v: f, drive_a: f, hz: f, zeta: f, dt: s) -> v2
fn look_level_clamped(head, tgt, trunk_q, yaw_lim: v2, glance: rad, prev_yaw: rad) -> GazeRes
fn check_pin_drift(drawn: p3@World, pin, surfs, tol: m) -> Verdict
fn check_joint_limits(pose, chains) -> VerdictStream
fn check_posture(pose, trunk, body) -> Verdict

```

**Six Default Creature Invariants (active unless `@waive`d):** `NoPlantedSlide` (≤ `0.5 mm` in surface frame), `JointLimits` (≤ `0.1°`), `NoGroundPenetration` (≤ `2.0 mm`), `PostureSanity` (`head.y > hip.y` while `composure > 0.3`), `SolverConverged` (residual ≤ `0.2 mm`, finite), `LiveBakeParity` (`0`-bit diff across 30/60/120 Hz + 6-tick stall).

## 4. The Escape Table

| Forkable Level | Author Writes | Keeps for Free | Loses |
| --- | --- | --- | --- |
| **1. Single BSDF lobe** | One `lobe` fn | Vis-buffer, all shapes, shadows, MIS, media, post, furnace checks | Multi-scatter LUT until re-integrated on twin (automatic for continuous lobes) |
| **2. Whole material model** | Custom `lobe`s + `shade_fn` | Exact depth, 4× MSAA silhouettes, true shadows, `World` overrides, motion | Furnace energy test (replaced by aesthetic/reference-input checks) |
| **3. Light transport** | New `shade_fn` using 6 `World` queries | All shape kinds, BSDFs, `RenderBundle`, media, post, R0g gate | Default unbiased MIS (scored against `reference`) |
| **4. One query of one kind** | `wgsl(...) -> T { ... }` inside that query | Typed bindings, all other kinds, vis-buffer, motion, speed/load gates | Twin, AD, derived $L$, Tier 0 tape **for that function only** |
| **5. Visibility record** | New record layout + `find_vis` + `shade_tiles` | All shapes, culling, `RenderBundle`, `drawIndirect`, motion, `reference` | Default 12 B MSAA layout & built-in `outline_vis` |
| **6. Schedule** | Custom `schedule Main { ... }` | All kinds, rows, queries, motion, `(wave, law)` bucketing, 5 gates | Default frame-budget constants until profiled |
| **7. One motion law** | One `fn` passed to `roam`/`act` or custom `law` | `BODY`/`STATE`/`POSE`, fused ticks, checkpoints, live/bake parity, 6 verdicts | Prior law's cached verdict hashes (auto re-checked) |
| **8. Motion solver** | New `solver` writing `POSE` or custom rows | Render binding, glTF export, checkpoints, replay identity, post-IK verdicts | Default XPBD compliance & built-in residual hook |
| **9. Whole engine** | Custom `schedule` + `wgsl` passes on `Runner` | Native/WASM `wgpu` runner, arenas, pipeline cache, input log, profiler, gates | Nothing forced; zero framework overhead |

---
