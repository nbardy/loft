# Loft v1.0 Final Specification Addendum

## A. Contradictions Settled

1. **Materials (`+` semantics).** `+` is an ordered physical layer stack written **bottom-to-top** (`base + sss + coat` puts `coat` on top), where each upper dielectric/microfacet layer attenuates everything below it by its directional transmittance $T = 1 - E_{\text{upper}}(\mu_o)\cdot F_{\text{upper}}$, every microfacet lobe includes Kulla–Conty multiple-scattering energy compensation, and `emit_lobe` is purely additive at the top.
2. **Reference pictures.** Keep R0g only as a one-time migration check when regenerating the legacy 295-scene corpus in Phase 5; for all new v1.0 scenes (Phases 1–4) and after corpus cutover, owner-approved v1.0 renders (geometry/look) and converged `reference` renders (light) are the sole Pictures gate references.
3. **`@waive` vs. "no `@attributes`".** Replace `@waive` with a plain keyword statement inside `body` or `schedule`: `waive NoPlantedSlide "reason"` (zero `@` tokens; `@` remains exclusively for spaces).
4. **Normativity of `loft.lark`.** `loft.lark` is **normative** for the surface grammar (modulo the lexer's newline-continuation filter and balanced `RAW_WGSL_BLOCK` scanner); apply the 8-line patch in §B.5/§B.9/§B.11 below so `waive`, `pass`, `plan` holds, and compound units (`m/s^2`) are accepted identically by `loft.lark` and `loft-parse`.

---

## B. Complete Specifications for Implementation

### 5. Schedule IR

Every schedule statement lowers to one or more `PassDecl` nodes in `Bundle.passes`. The compiler builds the pass DAG strictly from each statement's declared `reads` and `writes` resource sets and refuses read-before-write or cyclic dependencies (`E501`).

#### Built-In Schedule Statements

| Statement & Typed Signature | Reads | Writes | Lowers To |
| --- | --- | --- | --- |
| `orbit(at: p3@World, radius: m, fov: rad, height: m = 0.5m, period: s = 24s)` | `FrameUniform` | `CameraSlot` | Host/uniform camera update |
| `use_camera(cam: Camera)` | `FrameUniform` | `CameraSlot` | Host/uniform camera update |
| `run_laws(laws: LawRef...)` | Declared law read tables, `InputsBuf`, `BodyTables` | Declared law `writes` fields in `StateBuf`, `PoseBuf` | `ComputeFusedTicks` bucketed by `(wave, kind)` |
| `run_solver(s: SolverRef)` | `BodyTables`, `PoseBuf`, `StateBuf` | `s`'s `writes` table (`StateBuf` / custom table) | `Compute` (colored/Jacobi/shared-mem) |
| `check(checks: CheckRef...)` | `PoseBuf`, `StateBuf`, `RowTables` | `VerdictBuf` | `Compute` (lock-free fixed verdict slots) |
| `find_shadow_map(w: World, cam: Camera, res: u = 2048)` | `RowTables`, `PoseBuf`, `IndirectBuf` | `ShadowMap` (`depth32float`) | `RenderBundle` (casters only, depth-only) |
| `cull_indirect(w: World, cam: Camera)` or `cull_indirect(K...)` | `RowTables`, `PoseBuf`, `CameraSlot` | `IndirectBuf` (`visible_rows` + `DrawIndirectArgs`) | 1 `Compute` dispatch |
| `find_vis(w: World, cam: Camera, msaa: u = 4)` | `RowTables`, `PoseBuf`, `IndirectBuf`, `LutTables` | `VisBuf` (`rgba16uint`), `DepthBuf` (`depth32float`) | 1 `RenderBundle` (1 `drawIndirect` per kind) |
| `render_bundle(target: Target) { draw_indirect(K...) }` | `RowTables`, `PoseBuf`, `IndirectBuf` | `target`, `DepthBuf` | Explicit `RenderBundle` (low-level form of `find_vis`) |
| `tile_classify(vis: Target, tile: u = 8)` (alias `tile_bin`) | `vis` | `TileLists` (storage in `IndirectBuf`) | 1 `Compute` dispatch |
| `shade_tiles(vis: Target, shade: ShadeFn)` | `vis`, `DepthBuf`, `TileLists`, `RowTables`, `BvhNodes`, `ShadowMap`, `LutTables` | `HdrBuf` (`rgba16float`) | $K_{\text{active}}$ indirect `Compute` dispatches |
| `screen_pass(frag: fn(v2) -> v4, blend: BlendMode, out: Target = HdrBuf)` | Declared `reads` textures/tables | `out` | 1 full-screen `Render` draw (3-vert triangle) |
| `temporal_accum(curr: Target, vis: Target, prev: Target, prev_vis: Target)` | `curr`, `vis`, `Previous(prev)`, `Previous(prev_vis)` | `curr`, `prev`, `prev_vis` | 1 `Compute` dispatch (viewer-only) |
| `bloom_mip(hdr: Target, thresh: f = 1.2, gain: f = 0.18, radius: f = 3.0)` | `hdr` | `BloomMips`, `hdr` | Downsample + upsample mip chain (`Compute`) |
| `tonemap_aces(hdr: Target, sky: KindRef = Sky)` / `tonemap_agx(...)` / `present_ldr(...)` | `hdr`, `DepthBuf`, `sky` | `Swapchain` | 1 full-screen `Render` draw |
| `waive <InvariantName> "<reason>"` | None | Gate metadata | Marks invariant waived in `Bundle.verdicts` |
| `if_some(opt, stmt)` / `if_true(cond, stmt)` | Inner statement's | Inner statement's | Stage-0 conditional inclusion |

#### Custom Pass Declaration (Escaping Built-In Passes)

Any custom compute or render pass declares its resources explicitly inside a `schedule`:

```loft
# Custom compute pass
pass my_fluid reads [StateBuf, DepthBuf] writes [FluidTex] {
  compute(extent: [128, 128, 1], entry: step_fluid)
}

# Custom raster pass
pass my_overlay reads [HdrBuf] writes [Swapchain] {
  raster(verts: 3, instances: 1, blend: PremulOver, depth_write: false, vs: ov_vs, fs: ov_fs)
}

```

Add to `loft.lark`:

```lark
sched_stmt: "pass" LIDENT ("reads" "[" ident_list "]")? ("writes" "[" ident_list "]")? block
          | "waive" UIDENT STRING
          | expr block?

```

---

### 6. The `World` Queries' Types & `shade_fn` Contract

```loft
struct Ray     { o: p3@World, d: d3@World, t0: m, t1: m, w: v2 }        // w = (w0, dw/dt) ray footprint
struct Hit     { t: m, p: p3@World, n: n3@World, uv: v2,
                 local_p: p3@Local, kind_id: u, row_id: u, cell: v4 }    // t < 0m => miss
struct Scatter { wi: d3@World, weight: rgb, pdf: f, delta: bool }        // weight = f*cos/pdf
struct Lit     { wi: d3@World, l: rgb, pdf: f, p: p3@World, idx: u }     // solid-angle sample; pdf <= 0 => none
struct Sigma   { s: rgb, t: rgb, g: f, emit: rgb }                       // 1/m scatter & extinction, HG g, emission
struct Vol     { l: rgb, t: rgb }                                        // integrated in-scattered radiance & RGB transmittance
// Bsdf is a compile-time staged algebra (no runtime struct; lowers to per-kind K_eval/K_sample/K_emit/K_albedo).

// Six World Queries (closed over World w; w defaults to SceneWorld if omitted in single-world scenes)
pub fn trace(w: World, r: Ray) -> Hit                                    // nearest hit across all shapes in w
pub fn visible(w: World, a: p3@World, b: p3@World) -> rgb                // any-hit occlusion * RGB media transmittance in [0,1]^3
pub fn material(w: World, h: Hit) -> Bsdf                                // staged BSDF of (h.kind_id, h.row_id) at h
pub fn light(w: World, idx: u, p: p3@World, rng: RngKey) -> Lit          // emitter idx (0 = Sun) sampled toward p
pub fn pick_light(w: World, p: p3@World, rng: RngKey) -> Lit             // power-weighted / RIS emitter pick
pub fn sky(w: World, rd: d3@World) -> rgb                                // environment radiance along rd
pub fn medium(w: World, p: p3@World) -> Sigma                            // sum of all overlapping media at p

// Author-facing Bsdf operations (lowered statically inside shade_tiles, via kind switch inside reference)
pub fn eval_bsdf(b: Bsdf, n: n3@World, wo: d3@World, wi: d3@World) -> rgb
pub fn sample_bsdf(b: Bsdf, n: n3@World, wo: d3@World, rng: RngKey) -> Scatter
pub fn emit_of(b: Bsdf, wo: d3@World) -> rgb
pub fn bsdf_albedo(b: Bsdf, n_dot_v: f) -> rgb

```

#### `shade_fn` Contract (For Forked Light Transport)

`shade_tiles(vis, shade_fn)` invokes `shade_fn` **once per distinct `(kind_id, row_id)` present in a pixel's 4 MSAA samples**, reconstructs `h: Hit` at the pixel centre, weights the returned linear `rgb` by `sample_count / 4.0`, and writes `HdrBuf`:

```loft
// Normative signature of any function passed to `shade_tiles`:
type ShadeFn = fn(w: World, h: Hit, wo: d3@World, ink: bool, px: v2) -> rgb
// In scope inside shade_fn: `Frame.{time, dt, idx, sample, size}`, `ShadowMap`, and any captured World (e.g. `shadow_w`).
// Contract: returns finite, scene-referred linear radiance (nonnegative for PBR; unrestricted for stylised looks).

```

---

### 7. Complete `std` Typed Signatures (With Units & Defaults)

#### `std::geom`

```loft
// Analytic Kinds (exact hit + bound + L=1 axiom)
pub kind Plane        @World (n: n3@World = [0,1,0]@World, d: m = 0m)
pub kind Sphere       @Local (pose: xf@Local>World, r: m > 0m)
pub kind Box          @Local (pose: xf@Local>World, half: v3@Local)
pub kind Capsule      @World (a: p3@World, b: p3@World, r: m > 0m)
pub kind Cone         @World (a: p3@World, b: p3@World, ra: m > 0m, rb: m > 0m)
pub kind Washer       @Local (pose: xf@Local>World, r0: m >= 0m, r1: m > 0m)
pub kind Torus        @Local (pose: xf@Local>World, major: m > 0m, minor: m > 0m)
pub kind Mesh         @Local (pose: xf@Local>World, asset: MeshAsset, lod: u = 0)
pub kind BrickField   @Local (pose: xf@Local>World, asset: BrickAsset)
pub kind Specks       @World (buf: u, count: u, size: m > 0m)

// SDF Primitives (space-polymorphic S)
pub fn sphere[S](p: p3@S, r: m) -> m                                                    // Cinf, L=1
pub fn box[S](p: p3@S, half: v3@S) -> m                                                 // C0, L=1
pub fn round_box[S](p: p3@S, half: v3@S, r: m) -> m                                     // C1, L=1
pub fn capsule[S](p: p3@S, a: p3@S, b: p3@S, r: m) -> m                                 // C1, L=1
pub fn round_cone[S](p: p3@S, a: p3@S, b: p3@S, r1: m, r2: m) -> m                      // C1, L=1
pub fn torus[S](p: p3@S, major: m, minor: m) -> m                                       // C1, L=1
pub fn cylinder[S](p: p3@S, r: m, half_h: m) -> m                                       // C0, L=1
pub fn wedge[S](p: p3@S, half: v3@S) -> m                                               // C0, L=1
pub fn ellipsoid[S](p: p3@S, r: v3@S) -> m                                              // Cinf, L=max(r)/min(r)
pub fn plane_sdf[S](p: p3@S, n: n3@S, d: m = 0m) -> m                                   // Cinf, L=1

// SDF Operators & Closed-Form Intersectors
pub fn smin(a: m, b: m, k: m > 0m) -> m                                                 // C1 quadratic
pub fn smin_cubic(a: m, b: m, k: m > 0m) -> m                                           // C2 cubic
pub fn smin_exp(a: m, b: m, k: m > 0m) -> m                                             // Cinf exponential
pub fn smax(a: m, b: m, k: m > 0m) -> m                                                 // C1 smooth max
pub fn ssub(a: m, b: m, k: m > 0m) -> m                                                 // C1 smooth subtract
pub fn shell(d: m, thick: m > 0m) -> m                                                  // C0 abs(d) - thick*0.5
pub fn bound_step(d: m, max_step: m > 0m) -> m                                          // conservative Unbounded wrapper
pub fn ray_sphere[S](ro: p3@S, rd: d3@S, c: p3@S, r: m) -> f                            // hit t or -1
pub fn ray_capsule[S](ro: p3@S, rd: d3@S, a: p3@S, b: p3@S, r: m) -> f                  // hit t or -1
pub fn ray_round_cone[S](ro: p3@S, rd: d3@S, a: p3@S, b: p3@S, ra: m, rb: m) -> f       // hit t or -1
pub fn ray_box[S](ro: p3@S, rd: d3@S, lo: p3@S, hi: p3@S) -> f                          // hit t or -1
pub fn hit_min(a: f, b: f) -> f                                                         // nearest positive t, else -1

// Domain Maps (introduce scoped space 'm)
pub fn move[S](p: p3@S, delta: v3@S) -> MapRes['m, S]
pub fn rot_x[S](p: p3@S, a: rad) -> MapRes['m, S]
pub fn rot_y[S](p: p3@S, a: rad) -> MapRes['m, S]
pub fn rot_z[S](p: p3@S, a: rad) -> MapRes['m, S]
pub fn mirror_x[S](p: p3@S) -> MapRes['m, S]
pub fn mirror_plane[S](p: p3@S, n: n3@S) -> MapRes['m, S]
pub fn repeat_grid[S](p: p3@S, period: v3@S, lim: v3 = [1e6, 1e6, 1e6]) -> MapRes['m, S]
pub fn repeat_polar[S](p: p3@S, count: u, axis: d3@S = [0,1,0]@S) -> MapRes['m, S]
pub fn fold_screw[S](p: p3@S, pitch: m, count: u = 1) -> MapRes['m, S]
pub fn fold_spiral[S](p: p3@S, arms: f = 1, rate: f = 0.2) -> MapRes['m, S]
pub fn fold_kifs[S](p: p3@S, iters: u, scale: f, off: v3@S, rot: xf@S>S) -> MapRes['m, S]
pub fn warp_twist[S](p: p3@S, rate: rad/m) -> MapRes['m, S]
pub fn warp_bend[S](p: p3@S, rate: rad/m) -> MapRes['m, S]

// Transforms, Frames, Curves, Noise & Assets
pub fn place[S](p: p3@S) -> xf@S>S
pub fn xform_id[S]() -> xf@S>S
pub fn inv[A, B](x: xf@A>B) -> xf@B>A
pub fn coords[S](p: p3@S) -> v3                                                         // dimensionless metres xyz
pub fn dir[S](v: v3@S) -> d3@S                                                          // checked unit direction
pub fn dist[S](a: p3@S, b: p3@S) -> m
pub fn frame[S](o: p3@S, t: d3@S, up: d3@S) -> frame@S
pub fn transport[S](prev: frame@S, p: p3@S, t: d3@S) -> frame@S
pub fn smootherstep(lo: f, hi: f, x: f) -> f                                            // C2 quintic (polymorphic in unit)
pub fn grad_noise2(p: v2) -> f                                                          // C2, L=2.0
pub fn grad_noise3(p: v3) -> f                                                          // C2, L=2.45
pub fn wave_noise3(p: v3) -> f                                                          // Cinf, L=1/sqrt(3)
pub fn fbm2(p: v2, octaves: u = 3, lac: f = 2.0, gain: f = 0.5) -> f                    // C2
pub fn fbm3(p: v3, octaves: u = 3, lac: f = 2.0, gain: f = 0.5) -> f                    // C2
pub fn smooth_voronoi_cone(p: v3, k: f = 0.12) -> f                                     // C1, L=1.0
pub fn smooth_voronoi3(p: v3, k: f = 0.12) -> f                                         // C1, L=1.2
pub build fn import_gltf(path: str) -> GltfScene
pub build fn export_gltf(path: str, pose: TableRef)
pub build fn bake_brick[S](f: fn(p3@S) -> m, lo: p3@S, hi: p3@S, res: u = 64) -> BrickAsset

```

#### `std::light`

```loft
pub kind Sun         @World (dir: d3@World = dir([-0.4,0.85,0.35]@World), color: rgb = [2.8,2.6,2.3], angular_r: rad = 0.0046rad)
pub kind Sky         @World (zenith: rgb = [0.35,0.56,0.88], horizon: rgb = [0.82,0.86,0.90])
pub kind PointLamp   @World (pos: p3@World, color: rgb, radius: m = 0.05m, reach: m = 10m)
pub kind FieldMedium @World (lo: p3@World, hi: p3@World, sigma_t: rgb, albedo: rgb, g: f = 0.0)
pub kind GaussMedium @World (lo: p3@World, hi: p3@World, blobs: u, count: u)

pub fn ggx(albedo: rgb, rough: f, metal: f = 0.0, f0: rgb = [0.04,0.04,0.04]) -> Bsdf
pub fn ggx_aniso(albedo: rgb, rx: f, ry: f, t: d3@World, metal: f = 0.0) -> Bsdf
pub fn lambert(albedo: rgb) -> Bsdf
pub fn coat(weight: f, rough: f = 0.08, ior: f = 1.5) -> Bsdf
pub fn film(weight: f, ior: f = 1.42, nm: f = 380.0) -> Bsdf
pub fn sheen(color: rgb, rough: f = 0.5) -> Bsdf
pub fn sss(color: rgb, radius: m = 0.05m) -> Bsdf
pub fn transmission(ior: f = 1.5, rough: f = 0.0, tint: rgb = [1,1,1]) -> Bsdf
pub fn emit_lobe(radiance: rgb) -> Bsdf
pub fn directional(d: d3@World, color: rgb) -> Lit
pub fn direct_mis(w: World, h: Hit, wo: d3@World, rng: RngKey) -> rgb
pub fn restir_di(w: World, h: Hit, wo: d3@World, prev: u, rng: RngKey) -> rgb
pub fn trace_refl(w: World, h: Hit, wo: d3@World, rng: RngKey) -> rgb
pub fn inscatter_joint(w: World, r: Ray, t_max: m, steps: u = 24, rng: RngKey) -> Vol

```

#### `std::stylised` & `std::post`

```loft
pub fn cel_bands(lit: f, cuts: v3, levels: v4, soft: f = 0.02) -> f
pub fn bokashi(u: f, foot: f = 0.15, under: v2 = [0, 0.65], amt: f = 0.35) -> f
pub fn cross_hatch(px: v2, lit: f, spacing: f = 5.0, deep: f = 0.25) -> f
pub fn overprint(base: rgb, ink: rgb, cover: f) -> rgb
pub fn outline_vis(vis: Target, depth: Target, px: v2, ink: rgb, width: f = 1.5) -> v4
pub fn camera_ray(cam: Camera, px: v2, lens_xi: v2 = [0,0], time_xi: f = 0.0) -> Ray
pub fn exposure_ev(c: rgb, ev100: f = 0.0) -> rgb
pub fn aces(c: rgb) -> rgb
pub fn agx(c: rgb) -> rgb
pub fn film_grain(c: rgb, px: v2, frame: u, amount: f = 0.045) -> rgb
pub fn paper_grain(c: rgb, px: v2, fibre: f = 0.04, tooth: f = 0.03) -> rgb
pub fn color_grade(c: rgb, lift: rgb = [0,0,0], gamma: rgb = [1,1,1], gain: rgb = [1,1,1], sat: f = 1.0) -> rgb

```

#### `std::motion`

```loft
pub fn body_com(parts: [MassPart]) -> { mass: kg, com: p3@Body, inv_i: mat3 }
pub fn world_inv_inertia(q: quat, inv_i: mat3) -> mat3
pub fn capture_point_ff(t: TrunkState, h: m, a_cmd: v3@World, v_s: v3@World, a_s: v3@World, g_eff: v3@World) -> p3@World
pub fn support_hull(feet: [FootPin], g_eff: v3@World) -> SupportHull
pub fn support_margin(hull: SupportHull, pt: p3@World) -> m
pub fn support_margin_without(feet: [FootPin], skip: u, pt: p3@World, g_eff: v3@World) -> m
pub fn clamp_accel_support(t: TrunkState, hull: SupportHull, a_want: v3@World, b: BodyRow, g_eff: v3@World) -> v3@World
pub fn load_shares_nnls(feet: [FootPin], p_star: p3@World) -> array[f, 16]
pub fn quintic_plan(d: v3@World, v1: v3@World, dur: s, t: s) -> { p: v3@World, v: v3@World, a: v3@World }
pub fn solve_reach_leash(t: TrunkState, feet: [FootPin], chains: [ChainRow], alpha: f = 0.0) -> f
pub fn solve_prior_xpbd(t: TrunkState, prior: PriorSample, alpha: f) -> f
pub fn solve_dist_edge(a: p3@World, b: p3@World, im_a: 1/kg, im_b: 1/kg, rest: m, alpha: f = 0.0) -> f
pub fn solve_2bone_ik(root: p3@World, tgt: p3@World, l1: m, l2: m, pole: d3@World) -> array[PoseJoint, 3]
pub fn solve_3bone_ik(root: p3@World, tgt: p3@World, lens: v3, pole: d3@World) -> array[PoseJoint, 4]
pub fn solve_s_neck_ik(sh: frame@World, head: frame@World, len: m, segs: u = 6) -> { ok: bool, err: m }
pub fn solve_fabrik_clamped(pts: array[p3@World, 16], n: u, tgt: p3@World, lim: SwingTwist) -> f
pub fn step_spring_exact(x: f, v: f, drive_a: f, hz: f, zeta: f, dt: s) -> v2
pub fn volume_squash(q: f, axis: d3@Body = [0,1,0]@Body) -> mat3
pub fn look_level_clamped(head: p3@World, tgt: p3@World, trunk_q: quat, yaw_lim: v2, glance: rad = 0.26rad, prev: rad = 0rad) -> { q: quat, yaw: rad, err: rad }
pub fn sample_prior(tbl: PriorTable, t: s) -> PriorSample
pub fn pose_joint(body: u, joint: u) -> PoseJoint
pub fn pose_end(body: u, chain: u) -> p3@World
pub fn check_pin_drift(drawn: p3@World, pin: SurfacePin, surfs: [SurfaceRow], tol: m = 0.5mm) -> Verdict[m]
pub fn check_s_neck_length(body: u, expected: m, tol: m = 0.2mm) -> Verdict[m]
pub fn check_gaze_honest(body: u, tgt: target@World, g: GazePin, airborne: bool) -> Verdict[rad]
pub fn check_posture(body: u, t: TrunkState, b: BodyRow) -> Verdict[m]

```

---

### 8. Motion Tables (`std430` Exact Layouts), `body` Syntax & Custom Output Tables

Every struct below is strict `std430` (every `vec3f` padded to 16 B, every struct size a multiple of 16 B).

#### `BODY` Tables (Immutable / Editor-Updated, `@group(1)` Binding `1`)

| Struct | Byte Offset | Field | WGSL Type | Unit | Size |
| --- | --- | --- | --- | --- | --- |
| **`BodyRow`** *(112 B)* | `0` | `com_local` / `mass` | `vec3f` / `f32` | `m` / `kg` | 16 B |
|  | `16` | `inv_inertia` (`mat3x3f`, 3 padded cols) | `array<vec4f, 3>` | `1/(kg·m²)` | 48 B |
|  | `64` | `inv_mass`, `rest_h`, `tc`, `mu` | `vec4f` | `1/kg`, `m`, `s`, `1` | 16 B |
|  | `80` | `strength`, `chain_start`, `chain_count`, `seg_start` | `f32`, `u32`, `u32`, `u32` | `g`, idx | 16 B |
|  | `96` | `seg_count`, `eye_chain`, `flags`, `pad` | `vec4u` | count / idx | 16 B |
| **`ChainRow`** *(112 B)* | `0` | `socket_pos` / `role` (`0=sup,1=gaze,2=manip,3=trail`) | `vec3f` / `u32` | `m` / enum | 16 B |
|  | `16` | `socket_rot` | `vec4f` (quat) | `1` | 16 B |
|  | `32` | `rest_end` / `ik_kind` (`0=2b,1=3b,2=sneck,3=fabrik`) | `vec3f` / `u32` | `m` / enum | 16 B |
|  | `48` | `lens` (up to 4 segment lengths) | `vec4f` | `m` | 16 B |
|  | `64` | `pitch_lo`, `pitch_hi`, `yaw_lo`, `yaw_hi` | `vec4f` | `rad` | 16 B |
|  | `80` | `twist_lo`, `twist_hi`, `mu_s`, `mu_k` | `vec4f` | `rad`, `1` | 16 B |
|  | `96` | `max_load`, `seg_count`, `pose_offset`, `neighbours` | `f32`, `u32`, `u32`, `u32` | `N`, count, mask | 16 B |
| **`SegmentRow`** *(48 B)* | `0` | `rest_pos` / `mass` | `vec3f` / `f32` | `m` / `kg` | 16 B |
|  | `16` | `rest_rot` | `vec4f` (quat) | `1` | 16 B |
|  | `32` | `half_extents` / `parent_seg` (`-1` = root) | `vec3f` / `i32` | `m` / idx | 16 B |

#### `STATE` & `POSE` Tables (`@group(2)` Bindings `0` and `1`)

| Struct | Byte Offset | Field | WGSL Type | Unit | Size |
| --- | --- | --- | --- | --- | --- |
| **`CreatureHeader`** *(176 B)* | `0` | `pos` / `composure` | `vec3f` / `f32` | `m` / `1` | 16 B |
| *(Trunk 96 B + Gaze 48 B + Meta 32 B)* | `16` | `vel` / `squash_q` | `vec3f` / `f32` | `m/s` / `1` | 16 B |
|  | `32` | `acc` / `squash_v` | `vec3f` / `f32` | `m/s²` / `1/s` | 16 B |
|  | `48` | `rot` (current tick end quat) | `vec4f` | `1` | 16 B |
|  | `64` | `omega` / `tick` | `vec3f` / `u32` | `rad/s` / tick | 16 B |
|  | `80` | `prev_rot` (tick start quat for sub-tick slerp) | `vec4f` | `1` | 16 B |
|  | `96` | `head_world` / `yaw_unwrapped` | `vec3f` / `f32` | `m` / `rad` | 16 B |
|  | `112` | `skull_q` | `vec4f` | `1` | 16 B |
|  | `128` | `bob_phase`, `handoff`, `glance_err`, `pad` | `f32`, `u32`, `f32`, `u32` | `1`, bool, `rad` | 16 B |
|  | `144` | `body_id`, `feet_start`, `feet_count`, `manip_start` | `vec4u` | idx / count | 16 B |
|  | `160` | `manip_count`, `trail_start`, `hit_window`, `chained_hits` | `u32`, `u32`, `f32`, `u32` | count, `s` | 16 B |
| **`FootPin`** *(80 B)* | `0` | `local_p` / `lift` | `vec3f` / `f32` | `m` / `m` | 16 B |
|  | `16` | `local_n` / `f_n` | `vec3f` / `f32` | `1` / `N` | 16 B |
|  | `32` | `from_p` / `f_t` | `vec3f` / `f32` | `m` / `N` | 16 B |
|  | `48` | `aim_p` / `phase` | `vec3f` / `f32` | `m` / `1` | 16 B |
|  | `64` | `surface_id`, `mode` (`0=Planted,1=Swing,2=Slip`), `dur`, `down_t` | `u32`, `u32`, `f32`, `f32` | idx, enum, `s`, `s` | 16 B |
| **`ManipState`** *(32 B)* | `0` | `target` / `authority` | `vec3f` / `f32` | `m` / `1` | 16 B |
|  | `16` | `vel` / `active` | `vec3f` / `u32` | `m/s` / bool | 16 B |
| **`TrailNode`** *(32 B)* | `0` | `pos` / `inv_m` | `vec3f` / `f32` | `m` / `1/kg` | 16 B |
|  | `16` | `prev` / `pad` | `vec3f` / `f32` | `m` | 16 B |
| **`PoseJoint`** *(32 B)* | `0` | `pos` / `scale` | `vec3f` / `f32` | `m` / `1` | 16 B |
|  | `16` | `rot` (world-space rigid joint quat) | `vec4f` | `1` | 16 B |

**Exact Per-Creature `STATE` & `POSE` Footprints (strict `f32` `std430`):**

* **Heron** (2 feet): `176 + 2×80 =` **336 B** `STATE`; `16×32 =` **512 B** `POSE`.
* **Crab** (8 feet): `176 + 8×80 =` **816 B** `STATE`; `25×32 =` **800 B** `POSE`.
* **Boss Crab** (8 feet, 2 claws, 4×16 antennae): `176 + 8×80 + 2×32 + 64×32 =` **2,928 B** `STATE` *(replaces the earlier unpadded `2,368 B` estimate because `std430` aligns `pos`/`prev` `vec3f` to 16 B each)*; `96×32 =` **3,072 B** `POSE`.

#### Full `body` Declaration Example (Segments, Joints, Limits, Masses, Ends, Eyes)

```loft
body HeronBody {
  trunk { mass: 2.2kg, inertia: [0.18, 0.12, 0.22], rest_h: 0.85m, mu: 0.9, strength: 2.02 }
  segment pelvis { parent: none, pos: [0, 0, 0]@Body, mass: 1.4kg, half: [0.12m, 0.10m, 0.18m] }
  segment chest  { parent: pelvis, pos: [0, 0.12m, 0.14m]@Body, mass: 0.8kg, half: [0.10m, 0.09m, 0.14m] }
  build for i in 0..2 {
    let s = if i == 1 { 1 } else { -1 }
    chain leg[i]: support {
      socket: [s * 0.18m, -0.15m, -0.08m]@Body, lens: [0.45m, 0.48m], ik: two_bone,
      limits: { pitch: -40deg..85deg, yaw: -25deg..25deg, twist: -15deg..15deg },
      rest: [s * 0.18m, -0.85m, 0m]@Body, mu_s: 0.9, mu_k: 0.6, max_load: 80N,
    }
  }
  chain neck: gaze {
    socket: [0, 0.18m, 0.24m]@Body, lens: [1.55m], ik: s_neck,
    limits: { pitch: -45deg..55deg, yaw: -110deg..110deg, twist: -20deg..20deg },
    eye: { offset: [0, 0.03m, 0.06m]@Head, fwd: [0, 0, 1]@Head, glance: 15deg },
  }
}

```

#### Replacing `POSE` With a Custom Motion Output Table (Non-Rigid / Continuum)

```loft
state TentacleMesh checkpoint 60 { ctrl: array[p3@World, 64], prev: array[p3@World, 64] }

solver SoftWave over TentacleMesh writes TentacleMesh.{ctrl, prev}
  config { strategy: colored_gs(2), substeps: 4, iters: 4, shared_mem: true }
{ /* updates ctrl directly; never touches POSE */ }

kind SoftTentacle @World (state_row: TentacleMesh, radius: m) {
  surface(u, v) grid 64x16 = eval_tube_from_table(TentacleMesh[state_row].ctrl, u, v, radius)
  material(p, n) = ggx([0.5, 0.2, 0.4], 0.3, 0) + coat(0.8, 0.05)
}

```

---

### 9. `plan` (Build-Time Horizon Solver)

```loft
plan HeronHop(keys: PriorSpline, rail: Box, block: SurfaceRow, dur: s = 2.4s) -> PriorTable {
  knots: 24, duration: dur
  minimize effort(1.0) + jerk(1e-7) + near(keys, pos: 8.0, vel: 0.5)
  subject_to {
    pos_at(0.0s, [0, 0.85m, 0]@World)
    vel_at(0.0s, [0, 0, 0]@World)
    clear(rail, margin: 0.08m)
    land_on(block, at: dur)
    pitch_in(-15deg, 15deg)
    flight_ballistic(from: 0.77s, to: 1.55s)
  }
}

```

* **Accepted objectives (weighted sum over `M = 4·N` uniform collocation samples):**
* `effort(w)` $\equiv w \int \Vert{}a(t) - g\Vert{}^2\,dt$
* `jerk(w)` $\equiv w \int \Vert{}\dddot{x}(t)\Vert{}^2\,dt$
* `near(spline, pos: wp, vel: wv = 0, acc: wa = 0)` $\equiv \int (w_p\Vert{}x - \hat{x}\Vert{}^2 + w_v\Vert{}v - \hat{v}\Vert{}^2 + w_a\Vert{}a - \hat{a}\Vert{}^2)\,dt$


* **Accepted holds (`subject_to`):**
* `pos_at(t, p: p3@World, tol: m = 1mm)`
* `vel_at(t, v: v3@World, tol: m/s = 1mm/s)`
* `acc_at(t, a: v3@World)`
* `arrive_at(t, p: p3@World, v: v3@World)` (triggers quintic boundary-condition anticipation)
* `clear(b: Box, margin: m)` (signed box clearance $\ge \text{margin}$ at every collocation point)
* `land_on(surf: SurfaceRow, at: s)` (terminal height + zero vertical/tangential slip)
* `pitch_in(lo: rad, hi: rad)` (posture box constraint)
* `flight_ballistic(from: s, to: s)` (eliminates flight knots into an exact 3-DOF parabola $a \equiv g$)


* **Storage & runtime read:** Solves once at Stage 0 on CPU (analytical sparse Hessian, augmented Lagrangian, max `40` iters, `tol = 1e-5`). Stored in `LutTables` as `PriorHeader` (`32 B`: `duration, n_samples, table_offset, status_tag`) + `n_samples` uniform `PriorSample` rows (`48 B` each: `(pos.xyz, compliance)`, `(vel.xyz, active)`, `(rot.xyzw)`). Read at runtime via `sample_prior(HeronHop, t)` with $C^1$ Hermite interpolation. On non-convergence, sets `status_tag = Unknown(SolveFailed)` so the Truth gate fails honestly.

---

### 10. Determinism Rules for Motion Code

| Category | Rule | Enforcement |
| --- | --- | --- |
| **Allowed** | IEEE-754 `f32` `+ - * /`, explicit `fma(a, b, c)`, `sqrt`, `min`/`max`, `clamp`, `select`, `u32`/`i32` bit ops, fixed-bound `for` loops, `workgroupBarrier()` between colored phases | Checked in `loft-passes/src/det_check.rs` |
| **Wrapped** | `sin`, `cos`, `atan2`, `acos`, `exp`, `log` inside `law`/`solver` emit Loft's fixed minimax polynomials (`det_sin`, `det_cos`, etc.) built exclusively from explicit `fma()`—never hardware fast-math transcendentals | Emitter pass rewrites calls automatically inside `law`/`solver` |
| **Banned** | Float atomics (`atomicAdd` on floats), `wgsl { ... }` without `det_verified`, dynamic `while` loops, unseeded RNG, cross-creature `StateBuf` writes outside `solver` (`colored_gs`/`jacobi`), `f16`, fast-math contractions (`@diagnostic(off)` / Metal fast-math disabled on motion pipelines) | Hard compile error `E504` |
| **Order & Parity** | Any reduction/gather (e.g. Jacobi) sums in ascending `row_id` order. Live and baked playback invoke the **single compiled entry** `advance(from_tick, to_tick, keep_stride)` with fixed `dt = 1/120 s` accumulator. | Guaranteed `0`-bit live-vs-bake parity per device; audited by Twin gate across 30/60/120 Hz + 6-tick stall |

---

### 11. Units & Numeric Promotion

* **Base units (4D rational exponent vector `(L, T, M, A)`):** `m` `(1,0,0,0)`, `s` `(0,1,0,0)`, `kg` `(0,0,1,0)`, `rad` `(0,0,0,1)`.
* **Derived aliases:** `mm = 0.001 m`, `deg = (π/180) rad`, `Hz = s^-1`, `N = kg*m/s^2`, `J = kg*m^2/s^2`.
* **Compound unit grammar (both in types and literal suffixes):**
```lark
unit_expr: unit_atom ( ("*" | "/") unit_atom )*
unit_atom: UNIT_SYM ("^" "-"? INT)?
// Valid examples: rad/s, m/s^2, N/m, 1/kg, kg*m^2

```


*(In numeric literals, `NUM_UNIT` lexes `0.5m/s^2` or `3.8rad/s` as one token using regex `/[0-9]+(\.[0-9]+)?(m|mm|rad|deg|s|kg|N|Hz)([*\/](m|mm|rad|deg|s|kg|N|Hz)(\^-?[0-9]+)?)*/`.)*
* **Numeric promotion rules:**
1. Untyped integer/float literals (`0`, `1`, `0.5`) have type `LitNum`: they unify with `f`, `u`, `i`, or any dimensioned unit **only** when constrained by a binary operand (`x + 0`, `x < 0`), parameter type, or column type.
2. `0` unifies with any dimension (`0 == 0m == 0rad == 0s`). Non-zero unitless literals cannot be added to a dimensioned value (`height + 1` is `E110`; write `height + 1m` or `height * 2`).
3. Multiplying/dividing a dimensioned value by a dimensionless `f` preserves the dimension (`2 * 0.5m : m`).
4. `sin`/`cos`/`tan` take `rad` and return `f`; `atan2(y, x)` requires equal units on `y, x` and returns `rad`; `exp`/`log`/`pow` require dimensionless `f`. Strip units explicitly via `x / 1m` or `num(x)`.



---

### 12. Randomness (Build-Time & Runtime GPU Hash)

#### Build-Time Deterministic API (`std::rand`, Philox-4x32-7, CPU Stage 0)

```loft
pub build fn rng(seed: u) -> Rng                                         // deterministic seeded stream
pub build fn next_f(r: Rng) -> f                                         // uniform [0, 1)
pub build fn next_range(r: Rng, lo: f, hi: f) -> f                       // uniform [lo, hi) (unit-polymorphic)
pub build fn next_gauss(r: Rng, mean: f = 0, std: f = 1) -> f            // Box-Muller deterministic normal
pub build fn next_disk(r: Rng, radius: m = 1m) -> v2                     // concentric Shirley disk sample
pub build fn next_dir(r: Rng) -> d3@World                                // uniform sphere direction
pub build fn hash_u(i: u, salt: u = 0) -> f                              // stateless build hash in [0, 1)

```

#### Runtime GPU Counter-Based Hash & RNG (`std::rand`, Philox-4x32-7, GPU + Twin)

```loft
pub fn hash1(x: u) -> f                                                  // stateless u32 -> [0, 1)
pub fn hash2(p: v2) -> f                                                 // stateless quantized v2 -> [0, 1)
pub fn hash3(p: v3) -> f                                                 // stateless quantized v3 -> [0, 1)
pub fn rng_key(px: v2, sample: u, bounce: u = 0) -> RngKey               // order-independent path RNG
pub fn rand1(r: RngKey) -> f                                             // advances counter, returns [0, 1)
pub fn rand2(r: RngKey) -> v2                                            // advances counter, returns [0, 1)^2

```

---

### 13. Tier 0 Opcode Table (56 Fixed 32-Bit Ops)

Encoding: `[op:6 (31..26)][dst:5 (25..21)][a:5 (20..16)][b:5 (15..11)][imm11:11 (10..0)]`, operating over `var reg: array<vec4f, 32>` (512 B/thread). `imm11` indexes `const_pool[0..2047]` (vec4f), `row` vec4 offsets, LUT slots, or loop target PCs.

| Range | Opcodes (`id: mnemonic — semantics`) |
| --- | --- |
| **0–7** *(Loads & Moves)* | `0: NOP`, `1: LOAD_CONST(dst=C[imm11])`, `2: LOAD_ROW(dst=Row[imm11])`, `3: LOAD_INPUT(dst=In[imm11])`, `4: LOAD_FRAME(dst=Frame[imm11])`, `5: MOV(dst=a)`, `6: SWIZZLE(dst=swiz(a, imm11))`, `7: SPLAT(dst=vec4(a[imm11&3]))` |
| **8–19** *(Arithmetic)* | `8: ADD(a+b)`, `9: SUB(a-b)`, `10: MUL(a*b)`, `11: DIV(a/b)`, `12: FMA(fma(a,b,reg[imm11&31]))`, `13: NEG(-a)`, `14: ABS(abs(a))`, `15: MIN(min(a,b))`, `16: MAX(max(a,b))`, `17: CLAMP(clamp(a,b,reg[imm11&31]))`, `18: MIX(mix(a,b,reg[imm11&31]))`, `19: SELECT(select(a,b,reg[imm11&31].x!=0))` |
| **20–27** *(Compare & Step)* | `20: LT(a<b)`, `21: LE(a<=b)`, `22: EQ(a==b)`, `23: STEP(step(a,b))`, `24: SMOOTHSTEP(smoothstep(a,b,reg[imm11&31]))`, `25: SMOOTHERSTEP(smootherstep(a,b,reg[imm11&31]))`, `26: FLOOR(floor(a))`, `27: FRACT(fract(a))` |
| **28–37** *(Math & Vector)* | `28: SQRT(sqrt(a))`, `29: SIN(sin(a))`, `30: COS(cos(a))`, `31: ATAN2(atan2(a,b))`, `32: EXP(exp(a))`, `33: LOG(log(a))`, `34: POW(pow(a,b))`, `35: DOT3(dot(a.xyz,b.xyz))`, `36: CROSS(cross(a.xyz,b.xyz))`, `37: LEN_NORM(vec4(norm(a.xyz), len(a.xyz)))` |
| **38–47** *(3D & SDF)* | `38: XFORM_PT(M[imm11]*a.xyz1)`, `39: XFORM_DIR(M[imm11]*a.xyz0)`, `40: MIRROR_X(vec4(abs(a.x),a.y,a.z,sign(a.x)))`, `41: REPEAT_CELL(repeat(a,b))`, `42: SD_SPHERE(len(a)-b.x)`, `43: SD_BOX(sd_box(a,b))`, `44: SD_ROUND_BOX(sd_rbox(a,b,reg[imm11&31].x))`, `45: SD_CAPSULE(sd_cap(a,b,reg[imm11&31]))`, `46: SMIN_POLY(smin(a.x,b.x,C[imm11].x))`, `47: SSUB_POLY(ssub(a.x,b.x,C[imm11].x))` |
| **48–55** *(Noise, Tables & Flow)* | `48: GRAD_NOISE3(grad_noise3(a.xyz))`, `49: SMOOTH_VOR3(smooth_voronoi3(a.xyz))`, `50: FBM2(fbm2(a.xy, imm11))`, `51: SAMPLE_LUT_C2(lut(imm11, a.x))`, `52: SAMPLE_TEX2D(tex(imm11, a.xy))`, `53: LOOP_BEGIN(trip=a.x, end_pc=imm11)`, `54: LOOP_END(start_pc=imm11)`, `55: RET(out=a)` |

*(Opcodes `56..63` reserved for future Tier 0 intrinsics.)*

---

### 14. The Bundle on Disk (`.loftb`) & Caching

Single-file binary container (little-endian, 16-byte aligned sections, zero-copy mmap-able):

```rust
#[repr(C)]
pub struct BundleHeader {
    pub magic: [u8; 8],           // b"LOFTB001"
    pub format_version: u32,      // 1
    pub flags: u32,               // bit 0: has_naga_ir, bit 1: has_wgsl, bit 2: has_debug_spans
    pub bundle_hash: [u8; 32],    // BLAKE3 of all sections below
    pub compiler_abi: u32,        // bumps on IR/ABI change
    pub section_count: u32,
} // 56 bytes, followed by SectionEntry[section_count] { tag: u32, offset: u64, len: u64, hash: [u8; 32] }

```

* **Sections read by `Runner::load_bundle`:** `0x01 MANIFEST` (bincode `Bundle` metadata: kinds, tables, passes, checkpoints, verdicts), `0x02 ROWS` (initial std430 `RowTables`, `BodyTables`, `BvhNodes`, `LutTables`), `0x03 MODULES` (per-module `ModuleId`, canonical IR hash, validated WGSL UTF-8 + optional serialized `naga::Module`), `0x04 ASSETS` (mesh vertex/index buffers, compressed textures), `0x05 SPANS` (optional file/line table for `loft show`).
* **What is cached & what invalidates it:**
1. **GPU Pipelines:** Vulkan persists `wgpu::PipelineCache` at `target/loft_cache/vk_<adapter_id>.bin` (invalidated by driver version or `module_hash`); Metal relies on the OS MSL shader cache keyed by emitted MSL text (invalidated only when `module_hash` changes).
2. **Bound / Lipschitz Derivations & `plan` Solves:** Cached at `target/loft_cache/deriv_<blake3>.bin`, keyed by `BLAKE3(compiler_abi, canonical_kind_ir_hash, shape_row_bytes)`. Any geometry edit or shape-row edit misses cleanly; pose-only edits always hit (`warm load ≤ 60 ms` **predicted**).



---

### 15. Comments & Lossless Edits

* **`loft-parse` builds a lossless Concrete Syntax Tree (CST)** (tokens carry leading/trailing `Trivia { Whitespace | Newline | Comment }` spans, ~150 LOC overhead over a bare AST). `loft fmt` and automated agent source rewrites operate on the CST and round-trip comments verbatim.
* **`loft ir --sexp` does NOT carry comments or whitespace.** It is a canonical, hash-stable serialization of Layer 1 semantic SSA IR; nodes carry only `(span "file.loft" line col)` for diagnostic provenance.

---

### 16. `loft fmt` Canonical Formatting

* **Indentation & width:** 2 spaces (never tabs); soft max line width `100` chars; LF line endings; single trailing newline.
* **Trailing commas:** Required on multiline parameter lists, row column lists, `emit` args, and array literals; omitted on single-line lists.
* **Spacing:** 1 space around binary operators, `:`, `=`, `|>`; no space before `:` in `x: m`; no space around `@` (`p3@World`, `xf@Leg>World`) or unit suffixes (`0.5m`).
* **Blank lines:** 1 blank line between top-level items; 0 blank lines inside single-expression blocks; 1 blank line separating `map` / helper `fn` / queries inside a `kind`.
* **Canonical ordering:**
* Top-level: `use` (sorted alphabetically) → `space` → `const` / `input` / `param` / `camera` / `target` → helper `fn` / `lobe` → `scan` → `kind` / `body` / `state` → `plan` → `law` / `solver` → `check` → `world` → `build` → `schedule`.
* Inside `kind`: `map` → private `fn` → queries in fixed order `bound`, `visible`, `trace`, `surface`, `material`, `light`, `sky`, `medium`.



---

### 17. CLI & Creative Training Contract

#### Commands & Flags

```text
loft build   <file.loft> [-o out.loftb] [--target native|wasm] [--check-budget]
loft render  <file.loft> --out <img.webp|png> [--size WxH] [--views v1,v2] [--time T] [--samples N] [--engine realtime|reference|woodblock]
loft clip    <file.loft> --out <dir|mp4> --size WxH --frames N --fps F [--start T] [--max-build-ms 2000] [--max-frame-ms 50]
loft profile <file.loft> --size WxH [--frames 60] [--nonce N] [--json out.json]
loft check   <file.loft> [--only pictures|twin|speed|load|truth] [--json out.json]
loft gate    [corpus_dir] [--only ...] [--json out.json]
loft show    <file.loft> [--kind K] [--plan] [--pixel x,y] [--json]
loft fmt     <path...> [--check]
loft ir      <file.loft> --sexp [-o out.sexp]
loft fork    <std_or_engine_path> <dest.loft>

```

#### Exit Codes

| Code | Meaning |
| --- | --- |
| `0` | Success (all requested checks/gates passed within budget) |
| `1` | Compile error (`E001`–`E601`: syntax, space, unit, Lipschitz, smoothness, staging, write conflict) |
| `2` | Truth check failed (`Missed` or `Unknown` verdict) |
| `3` | Picture or Twin parity gate failed (or missing frame) |
| `4` | Load gate failed (module/pipeline/cold-load cap exceeded or 1-vs-20 hash mismatch) |
| `5` | Invalid CLI arguments or missing file |
| `6` | Environment / GPU device error (not the scene's fault) |
| `7` | **Over budget** (CPU build > `--max-build-ms` or GPU p95 frame > `--max-frame-ms`) |

#### Creative Training Pipeline Interface

Training invokes:

```sh
loft clip <scene.loft> --size 512x512 --frames 16 --fps 16 --max-build-ms 2000 --max-frame-ms 50 --out <dir> --json <dir>/meta.json

```

* Exit `0`: writes `frame_000.png..frame_015.png` (never partial; missing frame returns `3`) and `meta.json` containing `{ "build_cpu_ms", "load_gpu_ms", "frame_gpu_p50_ms", "frame_gpu_p95_ms", "modules", "pipelines", "commands", "verdicts", "identity_hash" }`.
* Exit `7` immediately aborts rollout scoring when CPU build > `2,000 ms` or GPU frame > `50 ms` (**predicted** budget enforcement overhead < `1 ms`).

---

### 18. Asset Import (glTF 2.0 Subset — Phase 5)

* **Supported in Phase 5:**
* `.glb` and `.gltf` (+ external `.bin`/PNG/JPEG/WebP).
* Triangle meshes (`TRIANGLES` mode), `POSITION`, `NORMAL`, `TANGENT`, `TEXCOORD_0`, `JOINTS_0`, `WEIGHTS_0`.
* Standard PBR metallic-roughness (`baseColor`, `metallicRoughness`, `normal`, `emissive` textures & factors) lowered directly into `ggx(...) + emit_lobe(...)`.
* Skins (`inverseBindMatrices` + node hierarchy $\to$ `BodyRow`/`SegmentRow`) and animation channels (translation/rotation/scale $\to$ `PriorSpline` / `retarget` clips).


* **Explicitly excluded in Phase 5 (typed refusal `E602`):** Morph targets, sparse accessors, Draco/Meshopt compression, KHR transmission/sheen/clearcoat extensions (author those directly in `.loft`).
* **How an imported mesh becomes a kind with rows:**
`import_gltf("crab_prop.glb")` runs at Stage 0 (`build`), concatenates all primitives' vertex/index data into `Bundle`'s `ASSETS` buffers, builds a `Bvh2` BLAS per mesh, and returns a `GltfScene` handle with named `MeshAsset` IDs. Every instance emits a row of the single built-in `Mesh` kind (`0` extra shaders per mesh asset):
```loft
build {
  let glb = import_gltf("assets/anchor.glb")
  build for i in 0..20 {
    emit Mesh(pose: place([i * 2m, 0m, 0m]@World), asset: glb.mesh("Anchor"))
  }
}

```



---

## C. Open Questions

All five defaults in `05_NUMBERS_AND_BUILD_ORDER.md` stand unchanged. Proceed with Phase 1.5 and Phases 2–5 as specified.
