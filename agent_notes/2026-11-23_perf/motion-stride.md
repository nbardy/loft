# motion-stride: several feet aloft, and a body test set

Branch `motion-stride` from `motion-nfeet-2` (4b525fc7). Theory and predictions:
`agent_notes/2026-11-23_motion_stride/THEORY.md` (written before the code). Evidence:
`~/git/loft/out/motion-stride/`. Adapter Apple M4 (Metal). No Rust changed: the lane ran on the
motion-nfeet-2 `target/lab` binaries (same commit), so it used no compile slots.

## 1. What changed

- **State.** The single swing (`foot`, `swing`, `span`, `aim_x/z`, `lift`, `rest_time`) and the dead
  neighbour pairs are gone. Each site has `clock`, `span`, `aim_x`, `aim_z`; a site is aloft while
  `span > 0`. Plan constants written once in `walker_plan`: `sweep`, `swing_time`, `lead`/`wave`
  (the phase plan), `knee`, `sole`, `torso`.
- **Phase law** (`step`). β(v) = min(3/4, 1/(1 + v·T_sw/s)); stance progress u = |goal − anchor| / stride;
  Wilson targets with a leader tree, threshold 1 + clamp(e, −0.3, 0.15). Of the feet past threshold, the
  one furthest past that the support allows lifts; one lift per tick, any number aloft.
- **Veto.** Area support: pressure point now and centre at touchdown 2 mm inside the remaining hull; a foot
  overdue by 0.15 of a stride is held only to Theorem B (centre at touchdown inside rest + landing).
  Bodies of ≤ 3 sites: dynamic path (weight transfer, capture within 3 cm across/behind, landing within reach).
- **Knee pole.** `knee` vector in the body frame, mirrored per side; [0, 1, 0] reproduces the old pole.
  The crab plan uses [0, 1, 1.2].
- **Sole.** Contact radius dilating the hull (`pressure`); 0 is the old point contact.
- **Audit.** `stdlib/motion/audit.loft`: law `audit` plus checks `margin_held` (all feet down),
  `upright`, `no_slide`, `cadence`. `present` accumulates `slide`, `step` counts `stalls`.
- **Removed:** `share`, `kept`, `choose_foot` (and their `support_cases` rows; new rows cover the aloft
  mask and the sole).
- **Scenes.** `scenes/motion_bodies/{crab,hexapod,quadruped,tripod,biped,heron}.loft` + `stage.loft`.
  `crab_live` passes the new `walker_plan` arguments; the `tidal_fork` gait fork is re-forked from the new
  gait (swing height 0.185 m kept). `Walker.rest_h` range widened to 0.3..1.0 m for the heron.

## 2. Crab: predicted against measured

| | before (motion-nfeet-2) | predicted | measured |
|---|---|---|---|
| cycle at 0.31 m/s | 2.4 s | 1.01 s | **0.98 s** (mean over all feet, forward cruise; per-foot lift intervals 1.0–1.15 s) |
| trail (stance displacement at lift) | 0.75 m | 0.236 m root, ≤ 0.271 m | **0.271 m** max over 1.0–1.7 s |
| deepest behind home | – | 0.051–0.086 m | **0.086 m** |
| feet aloft at once (cruise) | 1 | 2 (one per side) | 2–3 in cruise; 4–5 during start-up and at the reversal |
| support | – | never negative | lowest swing margin +0.4 mm, standing +0.42 m, no stalls |

Hit on cycle (−3%) and trail (at the predicted bound). The bunching to 4–5 aloft at the start and
the reversal was not predicted: every foot's goal jumps at a velocity change, so many cross threshold
together. The hull veto keeps it statically safe (margin never negative), so it is a look issue only.

## 3. Body test set (checks at t = 1.7, 3.2, 4.0 s; `out/motion-stride/checks.txt`)

| body | planted | margin_held | upright | no_slide | cadence (meas./law) | verdict |
|---|---|---|---|---|---|---|
| crab (8) | held | held | held | held (0 mm) | 0.98 / 1.01 s | **pass** |
| hexapod (6) | held | held | held | held | 0.91 / 0.95 s | **pass** (swing margin −14 mm) |
| quadruped (4, upright hound) | held | held | held | held | 1.33 / 1.07 s | **pass** (swing margin −148 mm, see §4) |
| tripod (3) | held | missed | held to 3.2 s, missed at 4.0 | held | missed (3.4 s) | **fails** |
| biped (2) | held | n/a (no area) | held to 1.7, missed at 3.2 | held | missed | **fails** |
| heron (2) | held | n/a | held to 1.7, missed at 3.2 | held | missed | **fails** |

Foot slide is 0 mm in every body: planted toes never move (anchors change only at landing).

## 4. What needed gait changes (findings)

Every body is expressed as `walker_plan` data. Two pieces of data the old plan lacked were added:
`knee` (a heron's joint bends backward, a crab's sideways) and `sole` (a biped cannot start on point feet:
the support between steps is a segment across the travel, and the pressure point projected onto it has
no forward component). Gait code changes the bodies needed:

1. **Quadruped (narrow, upright).** With a strict veto no front foot ever lifts while moving forward: the
   centre must be behind a diagonal of the three-foot triangle, and at touchdown it is 8 cm further
   forward. Fixed by the overdue rule (Theorem B for a foot 0.15 stride late). Cost: the pressure point
   grazes the three-foot hull during front swings (the wanted point leaves it by up to 148 mm, i.e. the
   body briefly cannot brake), and the cycle is 24% over the law (vetoes). A real fix is trunk sway.
2. **Tripod, biped, heron: every lift leaves a segment or a point.** Static walking is impossible with
   point feet (a three-legged body included). Implemented the dynamic path: weight transfer (the wanted
   pressure point pushes ξ onto the remaining support, gain 3, across the travel and from behind), a lift
   only when ξ is within 3 cm of it, and landings aimed at ξ's divergence (e^{T/τ} − 1). Measured: the
   biped and heron walk forward upright through 1.7 s but fall (capture point beyond reach) at the
   reversal; the tripod stays upright to 3.2 s but steps 3 times in 4 s and its standing margin is
   −1.5 m (the transfer push saturates the support). Not periodic.
   **What the theory says is missing:** a footstep-aware DCM plan. The periodic lateral solution needs ξ
   at lift d = 2a/(E + 1) from the stance foot (a half-stance, E = e^{T_sw/τ} = 3.2 for the biped), which a
   per-tick projection does not produce; and a reversal needs a braking step plan (Theorem A's segment),
   not the velocity-tracking pressure point. Both are a controller with a short footstep horizon: beyond
   this lane.

## 5. Four-site scenes (crossing, tidal, tidal_fork)

Not byte-identical: the phase law replaces the strain chooser, so the step order changes (now a
lateral-sequence walk: hind B, front B, hind A, front A) from the first step. Keeping identity would
mean keeping `choose_foot` beside the new law, two controllers. At t = 0 (no step yet) the frames differ
on 20–24 pixels by 1/255 (ulp noise from the column torso and the knee term); the body is unchanged.
Before: `~/git/loft/out/motion-nfeet-2/stills_vs_main/`; after: `~/git/loft/out/motion-stride/stills_4site/`
(same names, t = 0..3 s; `<scene>_before_after.webp` puts t = 1, 2 before on top, after below).
**Regression to note:** in crossing at t = 2 s a hind leg trails far back, past the ferry deck. That is
the four-site front-lift veto of §4 (the walker turns its trunk in this scene, so the goals rotate and
the vetoed foot falls further behind). tidal and tidal_fork read as a slow lateral-sequence walk.

## 6. Verification

- **Twin:** `loft-verify trace-check --record --t 3` then `--replay`, state hash equal on all ten motion
  scenes: crab_live c332b47d, crossing d3290014, tidal e07ce3a7, tidal_fork b2432cd8, crab e468ab95,
  hexapod 509ccd9b, quadruped c5ba8892, tripod 234c6916, biped 5a68f939, heron 6cb337d0
  (`out/motion-stride/twin/results.txt`). The body scenes are twinned from check-free copies
  (`out/probe/bodies/`, same laws): `trace-check` stops at the first tick a check is not held, and
  `cadence` has no cycle to measure at tick 0. `loft clip` has the same rule, so the MP4s use those copies too.
- **Static:** `tools/check.sh static`: pins equal to Keel, 295 Phase 5 identities, 335/335 grammar files.
  (The grammar recognizer rejects a comment as the first line of an `if` block, and an odd number of
  apostrophes in comments; both fixed in the new code.)
- **Checks:** `planted` and `support_cases` hold on crab_live, crossing, tidal and tidal_fork at 1.5 and 3 s.

- **Timing** (`tools/gpu_lock.sh loft profile --size 1280x800 --frames 300`, Apple M4, load average
  9.4–9.6, paired and interleaved, before = motion-nfeet-2 tree, same binary), GPU p50 / p95 ms:

  | scene | round a | round b | round c |
  |---|---|---|---|
  | crab_live before | 14.48 / 15.83 | 14.76 / 17.13 | 15.41 / 17.53 |
  | crab_live after | 14.66 / 15.82 | 14.76 / 16.33 | 14.95 / 17.71 |
  | crossing before | 6.74 / 7.92 | 7.28 / 13.00 | 7.18 / 8.53 |
  | crossing after | 6.38 / 8.18 | 6.90 / 11.41 | 6.95 / 9.19 |

  Predicted unchanged within noise: held (crab −0.5..+1%, crossing −4..−5%, within the round spread).
  Bodies p50 / p95: crab 15.59 / 17.71, hexapod 12.32 / 13.60, quadruped 7.17 / 8.13, tripod 6.39 / 8.32,
  heron 6.45 / 7.85, biped 5.83 / 7.24. The trace cost scales with leg count; per-leg culling is the
  separate lane.

## 7. Outputs (`~/git/loft/out/motion-stride/`)

- `clips/<body>.mp4` (120 frames, 30 fps, 1280×800, SVT-AV1 preset 10 CRF 42), `stills/<body>_f015|045|075.webp`
- `gait/<body>_gait.png` (contact timing, 20 Hz samples), `gait/summary.json`
- `stills_4site/*.png` (lossless), `checks.txt`, `profiles/*.json`, `twin/*.trace`
