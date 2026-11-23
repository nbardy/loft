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

Status as of this note. Update the table when a row moves.

| Item | State | Next step |
|---|---|---|
| Local `main` against `origin/main` | Diverged: local has `dea8c8b7`, `8faf634d` and a duplicate of the reward-data note; origin has the lean2 merge `cad2bf4b` | Reconcile inside the integration lane |
| lean2 cuts 8, 7, 1 and the spike verdict | Merged on `origin/main` (`cad2bf4b`). Verdict: no-go for 18,000; smallest honest target ≤32,500 | Owner decides whether G5.1 is restated; deferred cuts 2, 4, 5, 6 land standalone |
| `merge-2026-11-23` (perf lanes, N-feet gait) | Not in `origin/main`; the redesign lanes are built on it | Lands with the integration |
| `probe-door` (Probe A: authored frame passes; cull, classify, present in stdlib) | Committed; module text reported identical for 242 modules | Integrate |
| `cut-tape` (§7.3: Tier 0 tape VM removed; ratchet 34,285 → 31,163) | Committed; edit refusal path and lint unverified | Integrate; verify the two |
| `cut-unused` (§7.2 cuts, `docs/spec/CUTS_2026-11-23.md`) | Committed | Integrate |
| `probe-rows` (Probe B: rows read rows) | WIP commit; P2 held, P3 missed | Integrate if its gates hold; record P3 as a measured miss |
| Cut 1 (§7.1: delete what A and B superseded) | Not started | After A and B are in |
| Probe C (a law over an image: TAA) | Not started | After Cut 1 |
| Ratchet reset | Not done | After the cuts |
| `tools-agent-build`, `fix/one-shot-errors` | Committed, small | Integrate |
| F1, F2, F4, F9, F11 | Not started | Fix lane on the integrated main |
| F3, F5, F6, F7 and two-second history | Not designed | Add to the meta-engine design as requirements |
| F12 guide text | Not drafted | After the fixes, with the trainer's owner |
| Strings in D's scene | Written, checked, not rendered; constraint is two-sided | One-sided slack, an up-and-down test, then the fight re-render |
| The combined Auncel | Owner has not picked | Lead's suggested mix: A's surface, B's gait and ring, C's setting and sky, D's arena and single take, simulated strings |
| Other physics for the character | Not started | The Spill as fluid (`stdlib/physics/particle_flip.wgsl`); balance as a controller (`capture_point_ff`) |
| Auncel branches and worktrees | `auncel/a` to `auncel/d`, unmerged; worktrees `~/git/_wt/loft-auncel-{a,b,c,d}` | Port the chosen scene to the integrated main, then remove with `cambium remove --safe` |
