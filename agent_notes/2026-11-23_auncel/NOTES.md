# The Auncel run, 23 Nov 2026: what was built, what it showed about Loft, what is pending

Written by the coordinating agent of the Auncel thread (session `loft-9c`). The worker logs hold the
attempts and measurements; this note holds the conclusions and the pending list so that neither is lost.

**Takeover notice.** At about 16:30 on 23 Nov the owner asked this session to merge and finish the
meta-engine redesign lanes (`probe-door`, `probe-rows`, `cut-tape`, `cut-unused`) and lean2. Those lanes
were started by another coordinating session and were all committed clean at 16:07. A coordinator that
resumes should read §6 before touching them.

## 1. What was built

The owner's character, "the Auncel" (an inverted chalk cone four metres tall on a two-centimetre point,
pleated, with a level black dish, threads and weights on the rim, a groove trail of arcs, and three boss
phases), was built four times by four independent workers, each from sketches to a moving model.

| Worker | Lens | Branch | Scene | Log |
|---|---|---|---|---|
| A | surface first | `auncel/a` | `scenes/auncel_a/` | `out/auncel/a/index.html` |
| B | balance first | `auncel/b` | `scenes/auncel_b/` | `out/auncel/b/index.html` |
| C | dish first | `auncel/c` | `scenes/auncel_c/` | `out/auncel/c/index.html` |
| D | boss first | `auncel/d` | `scenes/auncel_d/` | `out/auncel/d/index.html` |

`out/` is ignored by git; the logs, clips and stills live in the primary checkout at `out/auncel/`, with
the hub page `out/auncel/index.html` and the shared brief `out/auncel/BRIEF.md`. The scenes were built
with a binary pinned at `c3660843` (`~/git/_wt/loft-bin/loft-c3660843-auncel`). None of the four
branches is merged, and none has been compiled against a later main.

What each is best at, from the lead's reading of the stills (the clips were not watched by the lead; the
numbers are the workers' own):

- **A:** the surface. Knife pleats with one fine orange hairline per pleat, filtered analytically over the
  pixel footprint so nothing crawls. Tip is dark red-brown, not black.
- **B:** the travel. A gait that alternates which point is fixed (tip slides an exact arc about the
  centre of mass, then plants and the body pivots), closing twelve arcs into a ring.
- **C:** the Spill and the setting. A bank, stakes and a clouded sky, all upside down in the black; the
  late reflection is easiest to see here.
- **D:** the fight. One 49-second take through travel, Radius, Unfolding and Spill, with a 1.75 m figure
  for scale and tally marks inside the opened skin.

All four measured the reflection lag within 0.05 s of 2.00 s, conserved skin area from cone to disc
(about 52.0 m²), and kept strongly saturated pixels inside the orange-red band.

## 2. Insights worth keeping

1. **The design is geometrically self-consistent.** The cone's slant length is sqrt(4.0² + 0.75²) =
   4.07 m. That one length is both the "four-metre" Radius sweep and the radius of the "eight-metre" disc.
   A cone of half-angle a unrolls to a sector of 2π·sin(a); here sin(a) = 0.184, so a full disc needs
   5.43 times the cone's surface at every height. About 471 pleats at 1 cm pitch on the rim, each about
   2.7 cm deep, store exactly that. Opening the pleats is the flattening; the climbing wave is a partial
   opening travelling up the same parameter.
2. **Where compile time goes.** Not the GPU. Stage 0 derives a bounding box and a Lipschitz constant per
   distinct shape row of each kind by interval arithmetic over its `trace`. The result is cached on disk
   keyed by the kind's source and the row's shape field bytes. Slow cases, measured by sampling live
   processes: hand-written rotation maths in `trace` (derived L of 12 to 27, once 1e19, against a true 1),
   anything inside `unproved { }` (a blind branch-and-bound over the reach region), long loops in a trace,
   and a box or thin slab as the reach bound inside `unproved`. First versions took 2 to 9 minutes per
   check. After restructuring: 7 to 125 s once per geometry edit, then 0.1 to 0.6 s cached.
3. **What made scenes fast.** Rigid motion through the map stages (`translate`, `rot_x/y/z`), which the
   compiler proves rigid; one row per repeated part; pleats and grooves in shading, not in the shape; an
   ellipsoid, not a box, as a reach bound.
4. **`clip` recompiles and reloads the scene on every frame in which a law wrote a field**
   (`crates/loft-cli/src/clip.rs`, the `compile_snapshot_aov` branch), and writes no frame until the end.
   Measured on the bead probe: about 0.2 s per frame with a law against 0.02 s closed-form. A law is
   therefore affordable when its field reaches the shape through a map stage or as capsule endpoints.
5. **"Two seconds late" was done without any history.** All four made every moving quantity a closed-form
   function of time and evaluated the world at t − 2 s (C emits each reflected row twice, the twin 400 m
   below the arena, and the shade callback traces into the twin). This stops working as soon as real
   physics state is in the reflection.
6. **Travelling without stepping.** A body pivoting on a fixed point does not travel. Each worker had to
   decide what each arc is centred on. B's answer (alternate the fixed point) and A's "compass walk" are
   the two worth keeping.
7. **Strings.** A heavy bead keeps a cord straight, so a simulated string looks like a rod unless the cord
   lags: give the cord nodes much more air drag than the bead (0.90 against 0.999 per tick in the probe).
   A two-sided distance constraint makes each segment a rigid rod, so the string can fold but not go limp.
   Slack needs a one-sided constraint (`max(d - rest, 0)`), and D's follow-the-leader fix-up has to become
   one-sided as well. Slack appears only when the anchor accelerates downward faster than gravity or rises
   and stops abruptly; the snap taut is a visible jerk, a second stand-in for the "ticking" beside beads
   knocking on the wall.
8. **Process.** A machine-wide GPU queue turned a 15-second render into an 8-minute wait; the real load
   was CPU compile, which no slot was accounting for. One loft process at a time across the workers, with
   a 180 s run limit (`out/auncel/gpu.sh`), was the arrangement that worked. `render` (any size, one
   compile, writes WebP) is the right command for stills; the authoring guide names only `still` and
   `clip`.

## 3. The string probe

`out/auncel/probe/`: `law.loft` (36 beads on rigid strings by a pendulum law), `closed.loft` (the
closed-form twin), `string.loft` (36 strings of four free nodes ending in a heavy bead, Verlet plus
distance constraints from a swaying rim), `string.mp4`, `string_sheet.webp`. It compiles and renders on
the pinned binary. 96 frames at 640×400 took 6 s.

D's scene (`auncel/d`, commit `fad31d8b`) has the same model on its 34 threads with five free nodes,
contact against the cone wall and floor (15% of normal speed kept), and two per-frame checks. `check`
at 12 times across the fight: worst penetration 0.00 mm, worst stretch 0.143%. **Nothing with the new
strings has been rendered.**

## 4. What it showed about Loft

"Seen by" counts workers; the lead did not run the minimal reproductions separately.

| # | Finding | Seen by | Fix | Kind |
|---|---|---|---|---|
| F1 | `atan2` (also `acos`, `asin`, `atan`) reachable from a trace panics the bound evaluator: `unreachable!("checked builtin atan2")`, `crates/loft-eval/src/eval.rs:1119` at `c3660843`. Three workers wrote polynomial replacements. Repro: `kind K @World () { trace(p) = length(coords(p)) - 1.0 + 0.0 * atan2(p.z, p.x) }` | A, B, C, D | Interval rules for the inverse trig builtins, or at least a typed error | Bug |
| F2 | A `select` or `if` that depends only on `time` (not on the point) is rejected in a trace: E701 or E211 "no finite Lipschitz bound" | A, B, C, D | A condition independent of the point is not a break in space; bound by the larger branch | Bug |
| F3 | Derived step bounds are very loose for rotated exact shapes, which pushes authors into `unproved` | A, B, D | Recognise rigid motion; let a tilt go into the pose (`place` takes only a yaw) | Framework |
| F4 | A slow scene gives no warning: no per-kind derive time, no default budget | all | `check` prints derive time per kind and warns; a default `--max-build-ms` | Tooling |
| F5 | `clip` rebuilds per law-written frame and publishes frames only at the end, so a time limit loses everything | D, lead | Upload state as rows; stream frames | Framework / tooling |
| F6 | A law could write only scalar floats; `check` accepted a point write and `clip` refused it. Main `dea8c8b7` now reports it as a compile error, which is consistent but still does not allow points or poses | D, probe | Allow point and pose writes | Framework |
| F7 | Culling bounds come from build-time row values, so law-moved geometry can fall outside its box; declared field ranges are not used | D | Bounds from declared ranges | Framework |
| F8 | No contact between objects and no memory of the past in the library | all | See §5 | Framework / library |
| F9 | `--set` cannot reach a sibling module's `param` and silently leaves it unchanged if the entry file declares the same name; a `const` loop bound fails WGSL validation (i32 against u32); `tan` is not a builtin | A | Three small fixes | Bugs |
| F10 | `world_trace_ray` returns −2 on thin geometry; treated as a miss it shows sky through as speckle | C | Resolve or expose the unresolved case | Bug |
| F11 | E211 flagged `1 + 8·min(x, 0.35)` as a divisor that could reach zero | D | Interval of `min` with a non-negative operand | Bug |
| F12 | The authoring guide (`loft context`) omits `render` and says nothing about compile cost | lead | Add both. The creative trainer feeds this text to the policy and pins the binary hash, so it changes training prompts | Docs |
| F13 | Library candidates each written two or three times: a footprint-filtered periodic stripe (A, D), a delayed mirror (A, C, D), a rigid pose from tilt/azimuth/spin (B), strings with contact (D, probe) | | Lift into `stdlib` after one is chosen | Library |

## 5. How these map onto the meta-engine design

`agent_notes/2026-11-23_meta_engine/DESIGN.md` already contains the framework answers to three of them:

- **F8 (contact, the anchor that follows a tilting rim) is capability R:** a law may read any store. That
  is `probe-rows`. With it a string law can read the body's row, and two bodies can touch.
- **F8 (memory) is the one semantic rule of §5.3:** a store read before it is written this frame is last
  frame's value. That gives one frame of history for free. Two seconds of history is a ring of frames or
  rows built on it; it is not designed yet. The Auncel's delayed reflection is the use case.
- **F2 and the E701 limit in the 23 Nov session notes are the same family:** which branches a trace may
  take. F2 (time-only) is the easy half and can land now. Point-dependent branches are the session notes'
  "trace culling" item.

F5, F6 and F7 (law state as data, with bounds from ranges) are not in the design note. They should be
added to it as requirements: they are what makes physics cheap.

## 6. Pending

Status as of this note. Update the table when a row moves. Rows for the integration lanes were updated by the integrator on `lane/integrate-meta`, 23 Nov evening.

| Item | State | Next step |
|---|---|---|
| Local `main` against `origin/main` | Reconciled on `lane/integrate-meta` (merge `943391b5`, origin tip `51eec94b`); main itself not moved | Lead fast-forwards main to the integration tip, then pushes (commands in `agent_notes/2026-11-23_meta_engine/INTEGRATE.md`) |
| lean2 cuts 8, 7, 1 and the spike verdict | In `origin/main` and in the integration branch. Its single schedule reader (`check::motion_schedule`) carried probe-rows' table reads in the merge | Owner decides whether G5.1 is restated; deferred cuts 2, 4, 5, 6 land standalone |
| `merge-2026-11-23-r2` (perf lanes, perf-edges, N-feet gait) | Merged on its own (`b714e4f3`). The branch named `merge-2026-11-23` (`b2dd43c3`) is the abandoned first pass and is **not** merged | Lands with the integration. Its open capsules-20 Speed question is in merge.md |
| `probe-door` (Probe A) | Merged (`a2ed97fe`); one parser conflict with lean2 resolved | Done. Its follow-ups are Cut 1 rows below |
| `cut-tape` (§7.3) | Merged (`3c62c2ef`). The edit refusal path and corpus lint were verified on the integrated tree (INTEGRATE.md §5). `EditSession` adjusted for plan's and probe-door's removed `Program` fields | Done |
| `cut-unused` (§7.2) | Merged (`b4d05743`); `docs/spec/CUTS_2026-11-23.md` now holds both cut lanes' tables | Done. glTF import (394 lines) stays: an owner question in DESIGN §7.2 |
| `probe-rows` (Probe B, WIP) | Merged (`7127e39f`) with `motion-stride` inside it. P2 held, **P3 missed** (motion 0.475 ms per frame against a 0.15 bound; the fat row costs 0.17). Motion-stride changes the four-site walk (crossing, tidal, tidal_fork) and records a regression in crossing at t = 2 s | Owner reviews motion-stride's four-site stills before the push. Make the lockstep pair cheaper: one compute pass per tick group with both entries, and one argument upload per catch-up instead of per tick (P3's measured cost is the two passes and two uploads, about 150 µs per tick) |
| Cut 1 (§7.1) | Partly done by other lanes: body/state/retarget/chain/trunk/segment and solver/constrain went with cut-unused; cull, classify and present special cases went with probe-door. What remains is below | Next worker starts from this list, in this order. (1) **Packed-leg gait**, once P3 is fixed: port `scenes/crab_live.loft`, `scenes/showcase/std1e/{crossing,tidal,tidal_fork}.loft` and `scenes/motion_bodies/{crab,hexapod,quadruped,tripod,biped,heron}.loft` to `stdlib/motion/legs.loft` (`Trunk` + `Leg`, `trunk_plan`), with `stdlib/motion/audit.loft` reading `Leg` rows; then delete from `stdlib/motion/body.loft` the `Walker[sites]` lane columns, `walker_plan` and `kind LegChain` (emitted, read by nothing: DESIGN I4), from `stdlib/motion/gait.loft` the `step` law and `present` and every helper that reads Walker lanes (keep `support_hull`, `hold`, `join`, `split_links`, `commanded`, `scripted_bearing`, `wrap_phase`, `uncaught_spill`, `lift_phase_of`, which `legs.loft` imports), and the copy `scenes/showcase/std1e/gait.loft` (646 lines). Gate: Twin record/replay on every ported scene and the owner's look at the walk. (2) **Kind parameters** then have no user (`Walker[sites: u in 1..9]` is the only one): `KindDecl.params`, `Ck::kind_params`, the E402 one-value rule and the `Emitter.fixed` folding from motion-nfeet-2 (`4b525fc7`) become deletable; a language feature, so the owner's word. (3) **`outline_and_paper`** (two-line arm in `crates/loft-passes/src/pipeline/schedule.rs`): it only publishes the `paper` fact that probe-door's `tonemap` hole already reads; make `paper` an argument of `tonemap` in `stdlib/core.loft` and delete the arm and `Schedule.paper`. Gate: the woodblock scene's module text byte-identical. Not deletable yet, and why: `find_shadow_map` and `shade_tiles`' frame part replay per-kind modules (the kind adapters DESIGN keeps in the kernel); `woodblock(...)` needs world-typed schedule parameters; `bake_tex`/`screen_pass` fold into `law` only after Probe C |
| Probe C (a law over an image: TAA) | Not started | Implement capability D on the probe-door door: a `target History` store and a law over it, run as an authored `pass` with a template; TAA in `stdlib`. Prediction P4: 0.40 ms at 1280×800, falsified above 0.6 ms. Then fold `bake_tex` and `screen_pass` into `law` (DESIGN §7.1 row 4) |
| Ratchet reset | Not done. Measured on the integration tip: **28,906** (`tools/lines.py`, ceiling 33,880; loft-runner 1,943 / 2,200) | After the lead pushes: set the `Rust core` ceiling in `tools/lines.py` to the measured value of the pushed tip (expected 28,906), with a comment naming the cuts, so growth fails again. The 18,000 G5.1 target stays an owner decision |
| `tools-agent-build`, `fix/one-shot-errors` | Merged (`500e76e3`, `19c6ae65`), no conflicts | Done |
| F1, F2, F4, F9, F11 | Not started | Fix lane on the integrated main |
| F3, F5, F6, F7 and two-second history | Not designed | Add to the meta-engine design as requirements |
| F12 guide text | Not drafted | After the fixes, with the trainer's owner |
| Strings in D's scene | Written, checked, not rendered; constraint is two-sided | One-sided slack, an up-and-down test, then the fight re-render |
| The combined Auncel | Owner has not picked | Lead's suggested mix: A's surface, B's gait and ring, C's setting and sky, D's arena and single take, simulated strings |
| Other physics for the character | Not started | The Spill as fluid (`stdlib/physics/particle_flip.wgsl`); balance as a controller (`capture_point_ff`) |
| Auncel branches and worktrees | `auncel/a` to `auncel/d`, unmerged; worktrees `~/git/_wt/loft-auncel-{a,b,c,d}` | Port the chosen scene to the integrated main, then remove with `cambium remove --safe` |
