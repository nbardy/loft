# perf-integrate: four perf lanes combined, scored once against main

Branch `perf-integrate` from main `7a7cc2f`. Real merges, in this order: perf-instrument `fc71e15`, perf-load `33ef326`,
perf-frame `683bcb3`, perf-precision `6339b0e`. All four branched from `34bd927`. Main is 11 commits ahead of that, but
those commits touch only agent notes and `stdlib/{engines,materials,styles}` `.loft` files, not Rust or the core WGSL.
motion-nfeet (now `7cbbd27`) was left out on the coordinator's instruction: it adds about 0.75 ms to 4-site crossing
and needs a fix first.

## Conflicts and resolution

- `crates/loft-cli/src/main.rs` (`loft load`), the only textual conflict, between perf-load and perf-precision. perf-load
  replaced the single load line with a cold/warm pair, front-end stage laps and per-pipeline times, and dropped the
  one-frame GPU reading. perf-precision appended `precision {}` to the old line. Resolution: keep perf-load's loop and
  add `precision {}` (`l.precision.name()`) to each cold/warm line. Both behaviours are kept.
- `crates/loft-passes/src/bundle.rs` merged without conflict, and the three edits compose correctly:
  `indexed` tests `vs_grid` (load); classify binds `hdr` at (1,3) only when the module declares it (frame); textures
  come from `Role` and `shader()` takes the HDR format (precision). The one semantic interaction is under
  `--precision f32`, where `retarget_hdr` rewrites every global named `hdr` that is declared `rgba16float`. perf-frame's
  halo classify declares exactly `var hdr: texture_storage_2d<rgba16float, write>` (`HALO_BINDINGS` in
  `emit/shade_tiles.rs`), so it is retargeted along with shade and stays bindable to the `Rgba32Float` texture.
  Present reads `hdr_tex` as `texture_2d<f32>`, which accepts either format. No code change was needed.
- `crates/loft-native/src/lib.rs`, `crates/loft-cli/src/profile.rs` and `crates/loft-emit/src/emit.rs` auto-merged
  (instrument's `load_control` alongside precision's features and `Loaded::precision`; load's `vs_grid` alongside
  frame's present/classify emission).
- Fix commit `490b20e`: `tools/check_core_words.sh` rejects the word `shadow` in `crates/loft-ir`. perf-precision's
  `Role::Depth` doc comment used it, so `check.sh full` stopped in static checks (perf-precision fails this check on
  its own as well). The comment was reworded; no code changed.

## Build and tests

- `CARGO_BUILD_JOBS=2 tools/dev.sh check --workspace`: clean on the first build.
- `tools/dev.sh test --workspace`: 179/180 run. `loft-runner encode::edit_gate::time_edit_and_compare_native_promotion`
  failed its timing assertion (overhead p95 0.63 ms, limit 0.5) at load average 22, without the GPU lock. Rerun under
  `tools/gpu_lock.sh ... --no-fail-fast`: **187 passed, 1 skipped**. That test failed again inside `check.sh full`
  (overhead p95 4.42 ms). It is a GPU timing assertion in loft-runner, a crate no lane touched. It is contention, not
  this merge, but it is recorded as a failing executable in this run.

## Five gates (Apple M4 Metal, 1280x800, 30 warmup + 300 frames)

`CARGO_BUILD_JOBS=2 LOFT_TIMING_MARGIN=0 LOFT_BASELINE_BIN=<main lab loft> LOFT_BASELINE_BUILD_LOG=<main build.jsonl> tools/check.sh full`,
started 10:36 at load average 7.8 (1-minute) after waiting for under 8. It ended 11:41 at 4.9. Baseline: a lab build of
main `7a7cc2f` in `~/git/_wt/loft-perf-integrate-base`. Evidence: `out/check.tiUDgw/` in this worktree (ignored).

| Gate | main (last scored, OPEN.md) | perf-integrate |
|---|---|---|
| Pictures | 19/19 | **19/19** |
| Twin | 36/36 | **36/36** |
| Speed | 6/12 (all six p95 failed) | **9/12, 3 contended, 0 missed** |
| Load | 23/33 | **26/33** |
| Truth | 34/34 | **34/34** |

Speed is not like for like: main's verdict used the triangle control and this one uses perf-instrument's load
control with block brackets.

Pictures: all 18 deterministic RGBA hashes are byte-identical to the previously scored main (`~/git/loft/out/meta/gate.json`)
and to the fresh main capture (0 changed pixels each). coral_shrine differs from scored main (45 vs 94 pixels over 2/255),
which is inside its known 38-149 pixel run-to-run noise. 9/9 bead frames passed. Stochastic reference sphere held and its
mutant fired. No pictures or approvals were edited.

Load failures remaining: all six cold caps (capsules-1 0.528, capsules-20 0.677, blobs-1 0.591, blobs-20 1.064,
hermit-1 1.640, hermit-20 1.655 s) and hermit-20 warm 0.127 s, a single gate sample; the paired median is 0.048 s,
below the 0.06 s cap. Load gained hermit-1/20 pipelines (13 == 13, was 14) and hermit-1 warm.

Lines: Rust core 34,066 against the 18,000 budget (FAIL, as on main, which has 33,881; +185 from the four lanes).

## Lane claims re-measured in the combined build

Paired timing (`tools/gates/timing.py`, six AB/BA pairs, margin 0). Medians are shown as baseline -> candidate. Only
hermit-20 p95 gets a verdict. Every other interval spans zero or the budget, so it is "unknown". Note that timing.py's
`triangle_control_ms` for the candidate arm is now the load control (0.75 ms quiet), so the two arms' controls are not
comparable.

| Claim (lane) | Measured here |
|---|---|
| Hermit-20 GPU p95 (frame/precision) | 4.650 -> 4.426 ms (-4%), delta CI [-0.345, -0.062]. **Held** (5.6 budget); gate 4.527 ms, 10/10 quiet blocks |
| Capsules-20 GPU p95 | 0.951 -> 0.924 ms (-4%), CI spans 0. Unknown; gate 1.034 ms, CONTENDED (6/10 quiet) |
| Present on sky scenes (frame, predicted -12..-25%) | blobs-20 0.120 -> 0.087 ms (-28%), capsules-20 0.118 -> 0.102 (-14%); hermit unchanged (no sky), as predicted |
| BVH nearer-child (frame) | only the matched/BVH engines use it; not part of the five gates, so not re-measured |
| Hermit 14 -> 13 pipelines (load) | **13 == 13 at both counts**, now Held in Load |
| Hermit cold load (load, -0.1..-0.25 s) | hermit-1 1.496 -> 1.338 s (-0.16), hermit-20 1.475 -> 1.316 s (-0.17), CIs span 0 |
| Blobs/Capsules cold (load, predicted no change) | blobs-1 0.489 -> 0.425, blobs-20 0.688 -> 0.726 (Missed, 0.5 cap), capsules-1 0.376 -> 0.370, capsules-20 0.581 -> 0.586 s |
| Speed gate (instrument): contended runs give Unknown, never a miss | held: 3 contended -> Unknown, 0 misses; the 3 quiet p95s held (blobs-20 1.250, hermit-1 2.726, hermit-20 4.527 ms) |
| Precision default byte-identical (precision) | held: 18/18 hashes equal main |

## Possible regression (unresolved)

capsules-1 p95 0.656 -> 0.935 ms (+39%) and blobs-1 0.692 -> 0.880 ms (+14%) in the paired medians. Both delta CIs span
zero. In both scenes the candidate's load control read 1.5-2.5 ms (contended; the quiet bound is 1.10), and passes no
lane touched (Cull, Vis) slowed by about the same proportion. The Speed gate marked exactly these two scenes (and
capsules-20) CONTENDED. That points to contention during those candidate arms, not to code. It is not settled; rerun
`timing.py` for capsules-1 and blobs-1 on an idle machine before release.

## Open

- motion-nfeet not merged (coordinator hold).
- edit_gate timing test: flaky under GPU contention; passes under the lock on a quiet slot.
- Light-scene p95 (capsules-1, blobs-1) needs a quiet paired rerun.
- Cold Load caps remain missed on every scene (Metal pipeline floor, see perf-load).
