# merge-2026-11-23: perf-integrate, perf-edges and motion-nfeet-2 onto main

Branch `merge-2026-11-23-r2` from main `d22c6244`. Three real merge commits, in order: perf-integrate `10d4576`
(four perf lanes), perf-edges `65c32ecf` (`edges: none | coverage | taps`, default `none`), motion-nfeet-2 `4b525fc7`
(N-feet gait, compile-time kind parameters; contains motion-nfeet `7cbbd27`). No pictures or approvals were changed.

## Why `-r2`

The first pass (`merge-2026-11-23`, kept as `merge-2026-11-23-on-09442ac8`) was built on main `09442ac8`. While its
gate ran, main was rewritten: `09442ac8` is no longer an ancestor of main, its commits came back linearised
(`fa0ac8b0`, `303bdf5b`, `060ea5fd`), and `87fd9974` landed the same timestamp fix that pass had made itself.
Merging that main into the old branch would have reintroduced the dropped commits as duplicate history, so the three
merges were replayed on the new main instead. Both passes merged without textual conflicts. The `-r2` tree differs from
the first pass only by main's new commits, and by using main's timestamp fix instead of this lane's own.

## Conflicts and resolutions

- No textual conflicts in any of the three merges, on either base.
- One semantic conflict, found by the workspace tests on the first pass: main's timestamp validation (`86cfa221`)
  rejected a pass that ends before the previous one. In the merged build, the telemetry fixture's classify lands
  inside Vis on every run (Metal runs independent compute concurrently), so `loft-cli::motion_lane
  profile_json_counts_executed_fixture` failed six runs out of six with "unusable required timestamps". The first pass
  fixed it by accepting nested pairs. Main's `87fd9974` makes the same change, so `-r2` keeps main's version. Main also
  fixed `clip_contract`'s metadata key count (9 to 10, `frame_timings`), which had failed on main as well.
- Checked by reading: perf-edges removed `samples:`, and no `.loft` file outside `examples/` uses it (main's new stdlib
  and corpus files included). motion-nfeet-2's `Emitter.fixed` defaults to none on the two `Emitter::new` paths in
  `emit/shade_tiles.rs` that perf-edges uses, so World/coverage modules read kind parameters from the row: correct,
  just not folded. Main's timestamp `required` vector starts all true (`Clock::new`), so perf-instrument's
  `load_control` clock still validates.

## Build and tests (`-r2`)

- `tools/dev.sh test --workspace`: 187/188, 1 skipped. The failure is `encode::edit_gate::time_edit_and_compare_native_promotion`
  (overhead p95 0.81 ms against 0.5), which also failed when rerun alone under the GPU lock. It is the known GPU timing
  flake in loft-runner, which no lane touched. In `run_native_tests.py` under the GPU lock it passed: **all 51 native
  test executables passed**.
- Lab build: `--workspace --profile lab --bins --tests --examples` (`out/build-r2.jsonl`).
- `tools/check.sh static`: pass. Corpus lint: pass. Bead: 9/9.
- Lines: Rust core 34,285 against the 18,000 budget (FAIL, as on main; perf-integrate had 34,066).

## Gates (Apple M4 Metal, 1280x800)

`tools/check.sh full` was started once on the first pass. The coordinator stopped it because `quiet_gpu.sh` held the GPU
lock idle for 25 minutes waiting for build slots held by other sessions. Its pictures, tests and bead results are listed
above. For `-r2`, the pieces of `full_gate` ran individually (`~/git/_wt/merge-gates.sh`). Each quiet window was taken
only after a non-blocking flock probe found `slot.0` and `slot.light` free, under a watchdog that kills `quiet_gpu`
if it holds the GPU lock 120 s without starting the timed command (`~/git/_wt/merge-qrun.sh`). Every window started
within 2 s of taking the lock. Baseline binary: a lab build of main `09442ac8` (copied from the clean `loft-lean2-base`
checkout into `~/git/_wt/loft-merge-2026-11-23-base`, a clean checkout of `09442ac8`). Main's commits since then change
the front end and timestamp validation only; the six gate scenes' WGSL is identical apart from a helper's file index
(see below). Baseline pictures: perf-integrate's main capture (`7a7cc2f`). No render code changed on main since then.

| Gate | main (OPEN.md) | perf-integrate | merge-r2 |
|---|---|---|---|
| Pictures | 19/19 | 19/19 | **19/19** |
| Twin | 36/36 | 36/36 | **36/36** |
| Speed | 6/12 | 9/12 (3 contended, 0 missed) | **10/12 (1 contended, 1 missed: capsules-20 p95 1.108 against 1.1 ms)** |
| Load | 23/33 | 26/33 | **26/33** |
| Truth | 34/34 | 34/34 | **34/34** |

- Pictures: the 18 deterministic RGBA hashes are byte-identical to scored main (`~/git/loft/out/meta/gate.json`) and to
  perf-integrate's candidate. 0 changed pixels against the main capture on all 19, coral_shrine included (its hash also
  equals perf-integrate's). The stochastic reference sphere held and its mutant fired.
- Motion stills: **21/21 byte-identical** by RGBA hash to `~/git/loft/out/motion-nfeet-2/stills_vs_main` (crossing, tidal,
  tidal_fork at t = 0..3 s, step 0.5), with `loft still SCENE OUT --t T` from the `-r2` lab binary. The first pass also
  gave 21/21.
- Load failures (7): cold on capsules-20, blobs-1, blobs-20, hermit-1 and hermit-20; warm on hermit-1 (0.082 s) and
  hermit-20 (0.063 s). capsules-1 cold held this time (0.422 s). This is the same count as perf-integrate, and these
  caps fail on main too.
- Shader identity: `loft show` for hermit, capsules and blobs (count 20) is identical between perf-integrate's binary and
  `-r2` except `loft_internal_raw_{10,6}_750` -> `loft_internal_raw_2_750`. That rename comes from main's prelude
  change (`e5b6eace`) moving a source file index. So any timing difference between perf-integrate and `-r2` on the gate
  scenes is noise.

## Paired timing (`tools/gates/timing.py`, 6 AB/BA pairs, margin 0, base main `09442ac8`)

Full run (`out/merge-r2/timing/`), p95 medians base -> merge-r2. Every interval spans zero, so every verdict is "unknown",
except blobs-20 cold "missed" (cold caps fail on main as well):
capsules-1 0.669 -> 0.627, capsules-20 0.813 -> 0.857, blobs-1 0.759 -> 0.604, blobs-20 1.487 -> 1.362,
hermit-1 3.710 -> 3.737, hermit-20 5.599 -> 5.919 ms (delta CI [-0.065, +1.073]; perf-integrate measured -4% on the same
WGSL; noise).

### Small-scene verdict (perf-integrate's open item)

`timing.py --only capsules-1,blobs-1`, quiet window (load 5.6, candidate load control 0.72-0.74 ms in 11 of 12 arms):

| scene | base p95 | merge-r2 p95 | median delta | delta CI |
|---|---:|---:|---:|---:|
| capsules-1 | 0.660 | 0.631 (-4%) | -0.034 | [-0.065, +0.089] |
| blobs-1 | 0.710 | 0.659 (-7%) | -0.038 | [-0.104, +0.270] |

**Within noise, no regression.** The earlier +39% / +14% came from contended candidate arms. blobs-1 cold also held
(0.461 -> 0.384 s, CI [-0.123, -0.039]).

### capsules-20 Speed miss

The single gate sample missed by 0.008 ms: p95 1.108 against 1.1 ms, with 8 of 10 blocks quiet and a load-control p50
of 1.078 ms, just under the 1.10 quiet bound. perf-integrate's gate read 1.034 ms on the same WGSL, but marked it
CONTENDED, so capsules-20 has never had a quiet verdict. In the six paired gate runs above, the candidate's median is
0.857 ms (base 0.813), and 1 of 6 candidate samples is over budget (1.49 ms, in a pair whose load control was quiet)
against 0 of 6 for the base. The delta CI is [-0.147, +0.710]. Code identity rules out this merge as the cause; the
budget sits inside the scene's run-to-run spread. A focused paired rerun (`timing.py --only capsules-20`) waited
40 minutes for both build slots without getting them (holders at 14:27: `loft dev.sh test loft-cli` in the main
checkout, `loft dev.sh check --workspace` in loft-probe-rows; earlier car_self_driving cargo/uv and wave_sim `ab.sh`).
Under the coordinator's rule, it stopped without taking the GPU lock.

**Main was not fast-forwarded.** The brief allows a fast-forward only with no Speed miss, and the focused rerun that
could have settled the miss could not get a quiet window.

## Evidence

`~/git/loft/out/merge-2026-11-23/`: `r2/` (gates.log, gate.json, pictures.json, stochastic.json, timing*, motion_stills,
bead, logs), `first-pass/` (the stopped check.sh run on `09442ac8`, its pictures and motion stills), and `lanes/`
(copies of the three lane worktrees' ignored `out/`).

## Open

- capsules-20 p95 sits on its 1.1 ms budget. Main is not fast-forwarded until a quiet paired `timing.py --only capsules-20`
  (or an owner call) settles it. To fast-forward: `cd ~/git/loft && git merge --ff-only merge-2026-11-23-r2`, after merging any newer main.
- edit_gate timing test is still flaky under GPU contention.
- Cold Load caps still fail on every scene except capsules-1.
- `edges: coverage` is the visible improvement. Making it the default needs a re-pin: owner decision.
