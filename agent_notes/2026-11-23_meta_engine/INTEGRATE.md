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
