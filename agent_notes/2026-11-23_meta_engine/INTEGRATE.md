# Integrating the meta-engine lanes, 23 Nov 2026

Branch `lane/integrate-meta` in `~/git/_wt/loft-integrate-meta`, from local main `6ea8a122`. Main is not
fast-forwarded and nothing is pushed here; the lead does that after reading this note. §1–§4 were written and
committed before the first merge. Results are appended in §5 onward without editing §1–§4.

Inherited: the lanes, their notes and measurements, the gate harness, the lead's suggested order. Mine: the
order change in §1, the conflict map in §2 (from `git merge-tree` dry runs), the predictions in §4 (the ratchet
numbers come from `scc` run on each lane's archived tree, so they are arithmetic, not guesses), and every
resolution in §5.

Also carried in this branch: `agent_notes/2026-11-23_meta_engine/DESIGN.md` and
`agent_notes/2026-11-23_session/NOTES.md`, copied byte for byte from the primary checkout, where they were
untracked. Every lane cites DESIGN.md; without this copy the merged history would cite a file it does not hold.

## 1. Inputs and order

| # | Input | Tip | What it is | Ratchet of its own tree |
|---|---|---|---|---:|
| 0 | local main | `6ea8a122` | `dea8c8b7`, `8faf634d` (compiler), the reward-data note, the Auncel notes | 34,047 |
| 1 | `origin/main` | `51eec94b` | lean2 (`cad2bf4b`: cuts 8, 7, 1, ratchet at 33,880, spike verdict) and two notes commits. It moved from `7704c02a` to `51eec94b` (a pending ledger, notes only) while this note was written | 33,895 |
| 2 | `tools-agent-build` | `ee0f403d` | GPU lock and quiet window through `agent-build` | 33,982 (= its base) |
| 3 | `fix/one-shot-errors` | `257007f8` | catalog names without an import; five first-attempt errors | 34,113 |
| 4 | `merge-2026-11-23-r2` | `a3cfe2c0` | perf-instrument, -load, -frame, -precision, perf-edges, motion-nfeet-2, and its note `agent_notes/2026-11-23_perf/merge.md` | 34,285 |
| 5 | `probe-door` | `43e590ce` | Probe A: authored frame passes; cull, classify, present in `stdlib/core.loft`; `on load`/`on dirty` on `find_shadow_map` | 34,868 |
| 6 | `cut-tape` | `328b184e` | §7.3: Tier 0 tape VM removed, edits recompile the kind | 31,163 |
| 7 | `cut-unused` | `1a888657` | §7.2: plan, solver, body/state/retarget, input/yield/wgsl det/waive, kind queries medium/sigma/tau, triangle ray query | 30,937 |
| 8 | `probe-rows` | `6461478d` | Probe B (WIP): rows read rows, legs as rows, `crab_rows`; **carries `motion-stride` (`6e73400c`)** | 34,687 |

Order: 0 ← 1 ← 2 ← 3 ← 4 ← 5 ← 6 ← 7 ← 8, one real merge commit each.

Changes from the lead's suggested order, and why:

- **Step 4 is new: `merge-2026-11-23-r2` merges on its own, before the probes.** All four redesign lanes sit on
  it, so it would arrive anyway inside probe-door's merge. Merging it alone puts its conflicts with main in one
  commit named for it, and leaves each lane merge carrying only that lane's diff, which is what makes a single
  lane revertible (step 8 may need that).
- **The branch called `merge-2026-11-23` (`b2dd43c3`) is not merged.** It is the abandoned first pass, built on
  a main that was later rewritten; merge.md explains why `-r2` replaced it. Merging it would bring back the
  dropped `09442ac8` history as duplicates. The lead's brief names `merge-2026-11-23`; the lanes actually carry
  `-r2`, so `-r2` is what lands.
- Otherwise the lead's order holds. probe-door before the cuts: the cuts then delete around a tree that already
  has the new door, so the "a probe's capability survives" check is made once, on the cut's side. probe-rows
  last: it is WIP, and last is the cheapest place to revert.

**motion-stride rides inside probe-rows.** It was not on the lead's list. It replaces the four-site gait's
foot chooser with a phase law; its own note says the four-site scenes (crossing, tidal, tidal_fork) are not
byte-identical to main from the first step, and records a regression: in crossing at t = 2 s a hind leg trails
past the ferry deck. No frozen gate picture uses the gait, so the Pictures gate cannot see it. Landing
probe-rows lands that motion change; it is flagged for the owner in the report rather than silently carried.

## 2. Expected conflicts

From `git merge-tree --write-tree` on the simulated chain (steps 1–4 are textually clean) and pairwise between
lanes.

| Merge | File | Between | Expected resolution |
|---|---|---|---|
| 5 probe-door | `crates/loft-parse/src/parser.rs` | lean2 `ac2db5a6` (body ranges kept for context and live edit) | keep both: lean2's ranges and probe-door's parametrised schedules and `on` clause |
| 6 cut-tape | `crates/loft-tier0/src/tier0/session.rs` | lean2 `ac2db5a6` edited it; cut-tape deletes the crate | delete (the cut wins); lean2's `ast.rs` range field must still be read by `context.rs` |
| 7 cut-unused | `Cargo.toml`, `docs/spec/CUTS_2026-11-23.md` | cut-tape (both edit the members list; both create the cuts doc) | union of member removals; one table holding both lanes' rows |
| 7 cut-unused | `crates/loft-check/src/motion_source.rs`, `plan_source.rs` | lean2 `64d02879` refactored them; cut-unused deletes them | delete. Semantic risk: lean2's new single readers (`solver_config`, `check::motion_schedule`, `plan_source::operands`) are called from `check.rs` and `loft-passes/src/motion.rs`; the solver and plan halves must go, the `run_laws`/`check` schedule reader must stay |
| 7 cut-unused | `crates/loft-cli/src/context.rs` | lean2 `ac2db5a6` | keep lean2's range reader, drop the cut constructs' arms |
| 7 cut-unused | `parser.rs`, `pipeline/assemble.rs`, `sexp.rs`, `docs/spec/loft.lark` | lean2 and probe-door | keep probe-door's schedule params, `pass` frame stores and `on` clause; drop the cut items |
| 8 probe-rows | `crates/loft-passes/src/motion.rs` | lean2 `64d02879` | keep lean2's one schedule reader and probe-rows' table reads and lockstep |
| 8 probe-rows | `crates/loft-emit/src/emit.rs` | cut-tape (edit hook) and cut-unused (solver) | keep probe-rows' row-read lowering; the edit and solver arms go |
| 8 probe-rows | `crates/loft-emit/src/emit/solver.rs` | cut-unused deletes it; probe-rows edited it | delete; check what probe-rows needed there and move it if it is not solver-only |
| 8 probe-rows | `crates/loft-passes/src/pipeline/lower_passes.rs` | cut-unused (`run_solver` arm) | keep probe-rows' lockstep lowering, drop `run_solver` |

Expected compile errors after all merges (no textual conflict, semantic only): callers of lean2's
solver/plan readers in `check.rs`; probe-rows or probe-door code that names a removed AST variant or the
removed `Options::edit`; fix/one-shot-errors' `check.rs`/`scope.rs`/`eval.rs` edits next to cut-unused's removal
of the `input`/`plan` scope branches.

## 3. Build plan

All merges written first. Then: one `CARGO_BUILD_JOBS=2 tools/dev.sh check --workspace`, all errors fixed in one
batch; one focused test run in the lab profile (so the full gate's lab build reuses it); `tools/check.sh static`;
`tools/check.sh full` once, with `LOFT_BASELINE_BIN=~/git/_wt/loft-lean2/target/lab/loft` (lean2, the last
scored main: Pictures 19/19, Twin 36/36, Speed 9/12, Load 23/33, Truth 34/34, lines 33,880) and
`LOFT_BASELINE_PICTURES=~/git/_wt/loft-lean2/out/check.h9GZ7P/candidate-pictures` (lean2's own capture), margin
0.05. Every job through the repo wrappers.

## 4. Predictions

| # | Prediction | Basis | Falsified if |
|---|---|---|---|
| I1 | Ratchet (`tools/lines.py`, Rust core) **28,850 ± 150** with all eight inputs; **28,440 ± 150** if probe-rows is reverted. Both under the 33,880 ceiling. loft-runner 1,943 (1,917 without probe-rows) under 2,200 | sum of measured per-tree deltas: 33,982 base, +65 local main, −87 lean2, +66 fix, +303 r2, +583 door, −3,122 tape, −3,348 unused, +402 rows = 28,844. Deletions that two lanes both counted (lean2's edits inside files the cuts delete, probe-rows' edits to `solver.rs`) push it up by tens, not hundreds | outside the band |
| I2 | Pictures 19/19 held against lean2's capture, 0 changed pixels; the 18 deterministic `gate.json` RGBA hashes byte-equal to lean2's and r2's; coral_shrine's hash differs (it differs run to run on main) | every lane reported module text identical (probe-door 242/242, probe-rows 16 scenes, cut-tape P7) and lean2 changed no WGSL | any of the 18 hashes moves, or the capture shows a changed pixel |
| I3 | Twin 36/36, Truth 34/34 | as I2 | any miss |
| I4 | Speed and Load score contended or Unknown on part of the set; Load near 26–29/33 with the known cold-load misses; no new miss attributable to the merge | shared machine; r2 26/33, cut-tape 29/33, lean2 23/33 | a warm-load or Speed miss on a scene whose module text changed (none should) |
| I5 | Tests: the workspace suite passes except known GPU-timing flakes. Gone from the suite: loft-tier0's tests, `edit_gate::time_edit_and_compare_native_promotion` (the known flake), the `tier0_words` golden, loft-plan's tests, the solver/body/plan diagnostic fixtures. Moved: the E101–E503 snapshots to loft-passes (cut-tape) minus E503 and E601 (cut-unused). New: probe-door's and probe-rows' edits to `motion_lane`, `engines_cpu`, `clip_contract` | lane notes | a failure outside timing tests |
| I6 | Compile: 5–15 errors after the textual merge, all in the places listed at the end of §2; fixed in one batch | three lanes were cut from a pre-lean2 base | a second batch needed |
| I7 | probe-rows' own claims re-hold on the combined tree (Twin record/replay equal for crab_rows and the motion scenes, P2 held, P3 missed). It is kept unless a test or gate fails because of it | its note | — |
| I8 | Motion stills for crossing, tidal and tidal_fork differ from main (motion-stride, intended); capsules, blobs, hermit and coral unaffected | motion-stride note §5 | — |

## 5. Results, 23 Nov evening

Appended by the meta-engine thread that took over after the 16:48 reset. The integrator's own account of the merges
died with its session; what it left on disk is in `out/integrate/` (lab build 6 m 47 s, 48 native test executables
passed, the edit refusal and lint logs) and in the pending table of `agent_notes/2026-11-23_auncel/NOTES.md`.

**Revision.** `c6fb08ae`: the integrator's tip `4bf07e3f`, plus its notes update, plus merges of `origin/main`
`90fc479a` (two design notes) and local main `116a64aa` (a merge commit whose content was already here). No code
differs from `4bf07e3f`. Main can fast-forward to it.

**One run of `tools/check.sh full`**, 17:59–18:27, baseline lean2 (`loft-lean2/target/lab/loft` and its picture
capture), margin 0.05. Exit 1, from the paired-timing step only. The machine was shared throughout: the load
control's p50 was 2.5–2.8 ms against a quiet bound of 1.1 ms. Evidence: `out/check.PdlhTQ/`, with the four small
reports copied to `out/integrate/gate_c6fb08ae/`.

| Step | Result |
|---|---|
| static | 330/330 grammar files in both recognizers; corpus 295 identities; pins equal |
| wasm32 check, doc tests | pass |
| native tests | "Passed all 48 native test executables" |
| corpus lint | exit 0; one warning, `corpus/l5d/examples__shatter.loft:119` |
| bead parity | 9/9 frames |
| Pictures | **19/19**, 0 changed pixels and 0 code delta against lean2's capture on all 19, coral_shrine included |
| stochastic | reference-sphere held (41.7–42.1 dB); the mutant fails, as it must |
| Twin | **36/36** |
| Truth | **34/34** |
| Speed | 6/12 held (the six command counts), **6 Unknown**: every p95 check was contended, 0 of 10 blocks quiet |
| Load | 25/33. The 8 misses are the six cold loads and hermit-1/-20 warm. All 8 are among lean2's 10 misses; lean2's two "14 pipelines == 13" misses on hermit now hold at 13 |
| lines | Rust core **28,906** / 33,895; loft-runner 1,943 / 2,200 |
| paired timing (`tools/gates/timing.py`) | **did not run**: crashed in its first control, see below |

**Against §4.**

| # | Verdict | Measured |
|---|---|---|
| I1 | held | 28,906 (band 28,700–29,000); runner 1,943 |
| I2 | held | 0 changed pixels on 19/19. coral_shrine also matched, which the prediction did not expect |
| I3 | held | Twin 36/36, Truth 34/34 |
| I4 | Speed unknown; Load 25/33 against "near 26–29" | No new kind of miss. Hermit warm load is 0.098 s and 0.102 s against lean2's 0.061 and 0.064 (budget 0.06). On this run that cannot be told apart from contention, and the paired run that would decide it did not happen |
| I5 | held | the suite passed; this section does not re-list which tests left or moved |
| I6 | not recorded | the integrator's error count died with its session; the fix is one commit, `4bf07e3f` |
| I7 | held as far as the suite goes | the Twin and native suites pass with probe-rows in; P3 stays a measured miss (PROBE_B) |
| I8 | not checked | no motion still was rendered for crossing, tidal or tidal_fork |

**The paired-timing crash is a finding, not a flake.** `timing.py` profiles a load control with the candidate binary
inside the baseline's source tree. The candidate refuses lean2's `stdlib/engines/capsules.loft`:
`error[E520]: unknown pass cull_indirect`. Probe-door moved `cull_indirect`, `tile_classify` and `tonemap` out of the
compiler into `stdlib/core.loft`, so **a binary and a stdlib from different sides of this merge do not work
together, in either direction**:

- new binary, old stdlib: E520 (above);
- old binary, new stdlib: `error[E001]: expected {, found (` at `stdlib::core:31`, `pub schedule cull_indirect(..kinds)`.
  Measured with main's `target/release/loft` (built 17:49 from `116a64aa`) running `context` in this worktree.

The harness is fixed here: the control now compiles the candidate's own sources in both arms
(`tools/gates/timing.py`). It has not been rerun; it needs a quiet machine.

The second direction is what the Creative trainer would hit. It runs `~/git/loft/target/release/loft` against a
symlink to `~/git/loft/stdlib` (`creative/training/loft/library.py:72`, `render.py:903`). Moving main to this tip
changes the stdlib at once and the binary only when someone rebuilds it. In between, every `check` and `clip` exits 1,
which the trainer scores as an invalid program, reward 0, without stopping the run. **Main must not move while a
trainer run is live, and the release binary must be rebuilt in the same step.**

**Also measured on this tip.** `loft context` is 24,460 characters against 18,376 on main `116a64aa` (+33%). 1,746
of the increase is the four schedule functions in `stdlib/core.loft` printed with their bodies and without their
parameters. The rest is the motion library (N-feet gait, `stdlib/motion/legs.loft`, `audit`). The trainer's prompt
is this text.

**Before main moves.**
1. The owner's yes on `docs/spec/CUTS_2026-11-23.md` and on `motion-stride` (§1).
2. No trainer run live; rebuild `target/release/loft` with the fast-forward.
3. A quiet paired `timing.py` run, for Speed and for hermit warm load. Until then both are Unknown, not held.
4. `loft context` prints a schedule function as its signature.
5. ~~The trainer's 44 seeds on this tip.~~ Done, 18:42: each of the 44 programs in Creative's
   `seed_programs/crab-overfit-v15` compiled with `loft build` (no device) by main's release binary in main's tree
   and by this tip's lab binary in this tree. **42 compile on both. The same two fail on both**, `bubble-scatter` and
   `sand-scatter`: "the portable artifact needs a standalone authored compute schedule", which is `build` refusing
   an authored compute pass inside a render frame, not a difference between the trees. Not covered: their pictures,
   and `check` and `clip`, which the trainer actually calls and which need the GPU.
