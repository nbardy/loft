# Loft v1.5 → Training Readiness Sprint Tracker (2026-11-21)

Single source of truth for all remaining tasks, architecture decisions, and insights to close out Loft v1.5 and launch **crab-overfit** (and general meta-engine) RL training in `~/git/creative`.

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

### Wave 15-2 — Core Loft v1.5 Completion (LIVE — `wave15_2.sh`)
- [x] `v15_exec` merged to `main` (`5e252da`): unified runner (`2,179/2,200`), `wgpu`-free `loft-std` (`0`), hardware Metal `RAY_QUERY` B-20 **`10.52 ms` p95** vs `14.58 ms` legacy, `v15a`/`v15b`/`v15c`/`x2a` folded, 48 old branches cleaned up
- [ ] `v15_stdlib_finish` (live): recover `std1d`+`std1c`+`port1` + R4 `fn material(hit) -> Bsdf` + 100% grammar pass
- [ ] `v15_perf` (live): R1 #2 sky bake cube binding, R1 #3 Shrine cluster culling, R7 motion-only FMA barrier + MSL leak guard
- [ ] `v15_native` (live): R6 rungs 2–3 (`EXPERIMENTAL_COOPERATIVE_MATRIX`, `bk1b` MSL auto-slots, `MetalFX` `HostCallback`, `wasm32` clean)
- [ ] `cap1c` (live): R3 6–10 material-family uber-shaders + packed `MaterialTable` + bindless/atlas (no auto-merge, no VM shading)
- [ ] `v15_bloat` (live): R5 binary `import_table`, `ad.rs` fix, regenerated corpus `≤ 10k` median tokens, `loft lint`
- [ ] `cap1b` (live): G2/G8.2 P2G fixed-point `Det` + `f32` atomics `NonDet` + WebGPU sorted gather MPM
- [ ] `iv15_2` (Astra, queued): merge all 6 into `main`, quiet-GPU release verification, `V1_5_CLOSEOUT.md`, delete merged worktrees & branches

### Parallel Now — Creative Trainer Rust Loft Bridge (LIVE NOW, 0 Wait)
- [ ] `creative_v15_runtime` (`~/git/_wt/creative-v15-runtime`, branch `loft/v15-runtime`): add `LoftV15` variant to `training/loft/{library,render,learner,config,tasks,selfcheck}.py`, ```` ```loft ```` extraction, exit-code `0..7` mapping, `configs/loft/system-v15.txt` + `crab-v15-overfit.jsonc` scaffolding, full `pytest` pass (preserving `Frozen`/`Vendor`)

### Wave 15-3 — Pre-Training Authoring & Seeds (`wave15_3.sh`, auto-starts after `wave15_2.finished`)
- [ ] **Stage B1 (parallel):**
  - `v15_context`: AST-driven `loft context` (`≤ 2,400` words, meta-engine mandate + worked engine-rewrite example + snippet check), real `loft fork`, updated `AGENT_CARD.md`
  - `v15_motion_mood`: `stdlib/motion/springs.loft` (Disney 12 principles) + `stdlib/motion/mood.loft` (6 composable moods + blending) + `scenes/showcase/mood_springs.loft`
- [ ] **Stage B2 (solo):**
  - `iv15_3a`: merge `v15_context` + `v15_motion_mood` into `main`, build release `target/release/loft`, remove merged worktrees & branches
- [ ] **Stage B3 (parallel, waits for `iv15_3a` + `creative_v15_runtime`):**
  - `v15_crab_seeds`: migrate 24–32 stratified `crab-actions-v2` seeds (keeping eval/edit anchors) to `.loft` v1.5 **+ generate 16–20 new full-engine-breadth crab seeds** (PBR, all 7 styles, forked schedules/passes, `MaterialTable`, Spore gait/IK, Disney springs, composable `Mood` transitions, GPU P2G/compute) → render `[0, 0.37, 0.91, 1.53]` → export `crab-overfit-v15` pack + `crab-v15-overfit.tasks.jsonl`
  - `v15_meta_seeds`: create 12–15 general non-crab meta-engine "improve" seeds in `~/git/loft/training/seeds/` + `tools/basin_test.sh` → export `loft-v15-meta` pack
- [ ] **Stage B4 (solo):**
  - `iv15_3b` (Astra): merge all Loft & Creative branches cleanly, run `pytest` + end-to-end `LoftV15` render smoke + `loft context` check, clean up all merged worktrees & branches, tag **`v1.5-train-ready`**, write launch command

### Non-Blocking / Owner-Only Follow-Ups (Anytime)
- [ ] Owner installs in `WRAPUP.md` & `HANDOFF_FREEZE_HELPER.md` (`freeze-helper` after `freeze-lineage-optin` merges, plus `cambium ee292e8`)
- [ ] `cleanup_tools` remote URL + 9-file reconciliation
- [ ] Optional solo core-slimming pass (`36k → ≤ 18k` Rust LOC in `loft-passes`) — does not block training
