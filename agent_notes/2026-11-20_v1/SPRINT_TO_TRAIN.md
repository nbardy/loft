# Loft v1.5 → Training Readiness Sprint Tracker (2026-11-21)

Single source of truth for all remaining tasks, architecture decisions, and insights to close out Loft v1.5 and launch **crab-overfit** (and general meta-engine) RL training in `~/git/creative`.

## Final integration

Loft merged all seven branch tips in `07cbd67`. The verified Rust sources are at `50d882a`. Creative merged runtime `aaeb7ddc` and crab seeds `0f135a73` into `loft/v15-ready`. Commit `e3310a27` decodes WebP seed frames for the judge’s PNG and motion-clip inputs. GitHub rejected the branch push with GH001 because the inherited Usenet mailbox is 477.08 MB. The integration checkout is `~/git/_wt/creative-v15-ready`.

### Size

`python3 tools/lines.py` with scc 3.7.0 compared pre-merge main `67cb62b` with `50d882a`. The core scan counts Rust code under `crates/` and excludes `tests/` directories. The extracted `tools/loft-verify` contributes 3,907 Rust code lines outside that scan, up from zero. Some of the runner-lane reduction comes from relocating tests.

| Crate | Before | After | Change |
|---|---:|---:|---:|
| loft-abi | 372 | 372 | 0 |
| loft-builtins | 293 | 293 | 0 |
| loft-check | 5,140 | 5,096 | -44 |
| loft-cli | 6,610 | 2,777 | -3,833 |
| loft-emit | 2,975 | 2,400 | -575 |
| loft-eval | 3,375 | 3,391 | +16 |
| loft-format | 740 | 690 | -50 |
| loft-gltf | 375 | 375 | 0 |
| loft-ir | 627 | 627 | 0 |
| loft-native | 1,438 | 1,438 | 0 |
| loft-parse | 2,680 | 2,680 | 0 |
| loft-passes | 3,964 | 4,273 | +309 |
| loft-plan | 859 | 859 | 0 |
| loft-runner | 2,178 | 2,155 | -23 |
| loft-std | 1,991 | 1,871 | -120 |
| loft-tier0 | 1,986 | 1,986 | 0 |
| loft-wgpu | 2,798 | 2,588 | -210 |
| shared | 10 | 10 | 0 |

| Ratchet | Before | After | Ceiling | Result |
|---|---:|---:|---:|---|
| Rust core | 38,411 | 33,881 | 18,000 | Missed |
| loft-runner | 2,178 | 2,155 | 2,200 | Held |
| loft-cli/native | 0 | 0 | 450 | Held |
| loft-std wgpu deps | 0 | 0 | 0 | Held |

### Verification

- Release `loft` and its sibling `loft-verify` built in 6m 06s. The lab verifier built in 2m 35s. All builds used two jobs and the shared lock. Integration required compile retries for builtin dispatch, the split compiler entrypoints and the relocated interop test. The release context exited 0 at **1,319 words**.
- Static verification passed. Earley and llguidance each classified **326/326** inputs as expected. Corpus provenance covered 295 distinct Phase 5 identities, and the host GPU dependency pins matched.
- The native suite first ran 187 tests, with 183 passes, four failures and one additional ignored archive test. Three corrected fixture/guide tests passed together. The fourth passed under the quiet guard at 0.365959 ms promotion overhead p95, down from 0.683084 ms on the first attempt against a 0.500 ms budget. Pixel differences above 1/255 were 0/140,000. The ignored archive test passed separately under the same guard.
- The Apple M4 Metal run at 1280×800 held **Pictures 19/19**, **Twin 36/36**, and **Truth 34/34**. Speed held 10/12 and Load 25/33, giving verifier exit 4. Capsules at counts 1 and 20 missed p95 at 0.917 and 1.383 ms. Four cold loads, two warm loads and both Hermit pipeline counts missed their budgets. No timing checks were marked contended. Of the 19 RGBA hashes, 12 matched the archived Wave 15-2 release. The six Hermit frames and Shrine differed. All 19 met the current frozen picture tolerances.
- The basin generated three prompts. All 12 meta seeds passed check, strict lint, both grammar recognizers, profiling and still capture. The 384×384 profiles used default overrides, 30 warmups and 60 timed frames. GPU p95 ranged from 1.223 to 4.104 ms, with triangle p95 from 0.032 to 0.045 ms.
- Creative passed all 28 Python tests in `training/loft/` and all four TypeScript tests in `training/loft/host/imports.test.ts`.
- The live trainer requested 512×512 beauty and depth at 0.91 s for each of 44 crab programs using strict lint and the configured budgets. All 28 migrated programs and 12/16 authored programs passed initially. Pebble Walker passed its focused retry. Black Glass again exceeded 50 ms GPU p95. Windup Snap and Spring Fiddler again exceeded the 2,000 ms CPU build cap, at 5,913.617 and 5,801.638 ms. **41/44 programs passed across both runs.**
- The primary Loft checkout retains logs, measurements, captures and a verified incremental bundle for Creative in `out/meta/iv_final/`. The recorded seed and size misses remain open for the release tags.

### Training

The general pack contains 12 non-crab meta-engine seeds. The crab pack contains 28 migrated and 16 authored programs, split 33 train and 11 eval. All 176 pinned 512×512 reference frames matched their SHA-256 checksums. The task file contains 117 rows. The offline launch preflight prepared four rounds, eight eval groups and 64 training programs at a $6.64 ceiling against the configured $15 budget. Its output directory must be fresh for a first launch.

Run from the integration checkout with trainer and judge credentials configured. The command starts the paid run in the Creative background supervisor.

```sh
cd ~/git/_wt/creative-v15-ready && uv run --no-sync python scripts/bg.py start crab-v15-overfit -- uv run --no-sync --with tinker==0.39.0 --with tinker-cookbook==0.6.1 --with tml-renderers==0.4.1 --with pillow --with wandb python -m training.loft --config configs/loft/crab-v15-overfit.jsonc --execute
```

## Key Insights Recorded From This Sprint

1. **Superintelligence Compile Discipline (global Rule 4 in `~/agents.md` + symlinked `~/.claude/CLAUDE.md` & `~/.codex/AGENTS.md`):**
   - Never run timid small edit-compile loops. Read types, borrows, and call sites upfront, write the complete implementation across all files in one decisive sweep, then compile (`≤ 2–3` builds total per lane).
2. **`loft context` & `loft fork` were missing in the Rust port:**
   - Legacy TS had `src/cli/context.ts` (~2,250 words). New Rust `loft-cli` has no `context` command yet and `fork.rs` is a 4-line stub.
   - Generate signatures directly via **`loft-parse` AST inspection** over `stdlib/**/*.loft` (zero drift, ultra-lean tokens) + at most a 1-line `#` doc comment ONLY where a contract/unit/invariant isn't self-evident (0 prose on obvious math/SDF functions), capped at **≤ 2,400 words**.
3. **Break the "black-box engine" prior in training:**
   - Models assume Unity/Three.js fixed engines unless taught + rewarded otherwise.
   - `loft context` and `configs/loft/system-v15.txt` must explicitly teach 4 escalation rungs (Scene/Rows → Style/BSDF → Fork/Rewrite Schedule/Integrator/Motion → Brand-New Engine with `pass`/`kernel allow`/`World` RT/WGSL) and include a worked ~30-line engine-rewrite example.
   - Starting seeds for `Improve` tasks (`axes: ["shape", "model", "renderer"]`, plus motion `["action", "integrity", "manner", "animation_principles"]`) MUST include editable meta-engine code (forked schedules, styles, springs, moods, compute passes) so the learner practices rewriting engines, not tweaking numbers.
4. **Spore motion + Disney springs + Emotion/Moods:**
   - `stdlib/motion/{solve,body,gait}.loft` already covers Spore-like procedural motion (`capture_point_ff`, `solve_2bone_ik`, `solve_s_neck_ik`, `solve_prior_xpbd`, `step_spring_exact`, `hull`/`contact`, planted-foot `check`).
   - Missing piece: pure meta-engine-editable **`stdlib/motion/springs.loft`** (Disney follow-through chains, volume-preserving squash-stretch, anticipation/overshoot, restless micro-motion, strike-and-hold) and **`stdlib/motion/mood.loft`** (6 composable `Mood` presets + blending) — directly powering `wording.json`'s `mood`, `manner`, and `animation_principles` axes and crab edit instructions (`"make the subject look sad"`, `"smile"`, `"epic boss"`).
5. **Crab-Overfit Migration + Engine Breadth (`~/git/creative`):**
   - Current `crab-actions-v2/seeds.jsonl` (103 seeds, `loft_0004`) is legacy TS `strand()` code.
   - `~/git/creative`'s `LoftV15` Rust CLI adapter can be built **immediately in parallel** (Python-only, zero Rust compile contention).
   - Seed migration & full-engine-breadth crab seed generation runs right after `iv15_3a` once `stdlib/`, `loft context`, `loft fork`, `springs.loft`, and `mood.loft` are finalized on `main`.
6. **macOS 15.5 Anti-Freeze & Branch Hygiene:**
   - Zero `ps`/`pgrep`/`lsof` polling; file-only status (`swarm_status.sh`), staggered worktree creation, never overwrite a running shell script in place (atomic temp+`mv`).
   - Every integrator merges passing branches, removes merged ephemeral worktrees via `git cambium remove --safe`, and deletes merged local + remote branches.
   - `freeze-helper` handoff saved separately at `agent_notes/2026-11-20_v1/HANDOFF_FREEZE_HELPER.md` (preserving active `~/git/_wt/freeze-lineage-optin`).

---

## Task Checklist & Critical Path

### Wave 15-2 — Core Loft v1.5 Completion (`wave15_2.sh`)
- [x] `v15_exec` merged to `main` (`5e252da`): unified runner (`2,179/2,200`), `wgpu`-free `loft-std` (`0`), hardware Metal `RAY_QUERY` B-20 **`10.52 ms` p95** vs `14.58 ms` legacy, `v15a`/`v15b`/`v15c`/`x2a` folded, 48 old branches cleaned up
- [ ] `v15_stdlib_finish` (held in the Wave 15-2 audit): recover `std1d`+`std1c`+`port1` + R4 `fn material(hit) -> Bsdf` + 100% grammar pass
- [ ] `v15_perf` (held in the Wave 15-2 audit): R1 #2 sky bake cube binding, R1 #3 Shrine cluster culling, R7 motion-only FMA barrier + MSL leak guard
- [ ] `v15_native` (held in the Wave 15-2 audit): R6 rungs 2–3 (`EXPERIMENTAL_COOPERATIVE_MATRIX`, `bk1b` MSL auto-slots, `MetalFX` `HostCallback`, `wasm32` clean)
- [ ] `cap1c` (held in the Wave 15-2 audit): R3 6–10 material-family uber-shaders + packed `MaterialTable` + bindless/atlas (no auto-merge, no VM shading)
- [x] `v15_bloat` merged in Wave 15-2: R5 binary `import_table`, `ad.rs` fix, regenerated corpus `≤ 10k` median tokens, `loft lint`
- [x] `cap1b` merged in Wave 15-2: G2/G8.2 P2G fixed-point `Det` + `f32` atomics `NonDet` + WebGPU sorted gather MPM
- [x] `iv15_2` audited all six lanes, merged cap1b and v15_bloat, and recorded the four held lanes in [the integration audit](lanes/iv15_2.md).

### Parallel Now — Creative Trainer Rust Loft Bridge (integrated)
- [x] `creative_v15_runtime` (`~/git/_wt/creative-v15-runtime`, branch `loft/v15-runtime`): add `LoftV15` variant to `training/loft/{library,render,learner,config,tasks,selfcheck}.py`, ```` ```loft ```` extraction, exit-code `0..7` mapping, `configs/loft/system-v15.txt` + `crab-v15-overfit.jsonc` scaffolding, full `pytest` pass (preserving `Frozen`/`Vendor`)

### Wave 15-3 — Pre-Training Authoring & Seeds (`wave15_3.sh`, auto-starts after `wave15_2.finished`)
- [x] **Stage B1 (parallel):**
  - `v15_context`: AST-driven `loft context` (`≤ 2,400` words, meta-engine mandate + worked engine-rewrite example + snippet check), real `loft fork`, updated `AGENT_CARD.md`
  - `v15_motion_mood`: `stdlib/motion/springs.loft` (Disney 12 principles) + `stdlib/motion/mood.loft` (6 composable moods + blending) + `scenes/showcase/mood_springs.loft`
- [x] **Stage B2 (solo):**
  - `iv15_3a`: merge `v15_context` + `v15_motion_mood` into `main`, build release `target/release/loft`, remove merged worktrees & branches
- [x] **Stage B3 (parallel, waits for `iv15_3a` + `creative_v15_runtime`):**
  - `v15_crab_seeds`: migrate 24–32 stratified `crab-actions-v2` seeds (keeping eval/edit anchors) to `.loft` v1.5 **+ generate 16–20 new full-engine-breadth crab seeds** (PBR, all 7 styles, forked schedules/passes, `MaterialTable`, Spore gait/IK, Disney springs, composable `Mood` transitions, GPU P2G/compute) → render `[0, 0.37, 0.91, 1.53]` → export `crab-overfit-v15` pack + `crab-v15-overfit.tasks.jsonl`
  - `v15_meta_seeds`: create 12–15 general non-crab meta-engine "improve" seeds in `~/git/loft/training/seeds/` + `tools/basin_test.sh` → export `loft-v15-meta` pack
- [x] **Stage B4 integration.** All seven Loft lane tips and both Creative lane tips are merged. Workspace, static, GPU and trainer verification ran. The launch command and closeout measurements appear above.
- [ ] **Train-ready release.** Three authored programs still exceed the rollout budgets.
- [ ] **Lean-core ceiling.** Counted Rust remains 15,881 lines above the 18,000 target.

### Non-Blocking / Owner-Only Follow-Ups (Anytime)
- [ ] Owner installs in `WRAPUP.md` & `HANDOFF_FREEZE_HELPER.md` (`freeze-helper` after `freeze-lineage-optin` merges, plus `cambium ee292e8`)
- [ ] `cleanup_tools` remote URL + 9-file reconciliation
- [ ] Optional solo core-slimming pass (`36k → ≤ 18k` Rust LOC in `loft-passes`) — does not block training
