# Step v15_seeds_creative — Meta-Engine "Improve" Seed Pack, `~/git/creative` Rust Loft Bridge & Basin Test

Read `~/git/loft/AGENTS.md`, `~/git/loft/docs/spec/06_ADDENDUM.md` (§17), `~/git/creative/training/loft/README.md`, `~/git/creative/training/loft/library.py`, `~/git/creative/training/loft/render.py`, `~/git/creative/training/loft/data.py`, `~/git/creative/training/loft/build_seed_tasks.py`, `~/git/creative/configs/loft/crab-curriculum.jsonc`, and `~/git/creative/configs/loft/system.txt`.

Also run `~/git/loft/target/release/loft context` first so you see the exact live context surface your seeds and trainer will use!

## Goal

Bridge the RL trainer in `~/git/creative` to the new Rust `.loft` meta-engine, build a curated **meta-engine "Improve" & "Edit" seed pack** (`loft-v15-meta`) that explicitly motivates agents to **rewrite engines, schedules, integrators, styles, kernels, and motion/mood laws** (not just tweak parameters), and verify end-to-end training readiness including a cold **basin test**.

## Task

### Part 1 — Meta-Engine Seed Pack & Basin Harness in `~/git/loft`
Work in `~/git/_wt/loft-v15-seeds` on branch `loft/v15-seeds` from `origin/main` (`git cambium add -b loft/v15-seeds -name v15-seeds -ephemeral ~/git/_wt/loft-v15-seeds origin/main`).

1. **Create 12–15 compact, self-contained `.loft` seed programs (`training/seeds/` + `training/seeds/manifest.jsonl`):**
   Every seed MUST compile cleanly under `~/git/loft/target/release/loft check`, pass `loft lint`, stay concise (well under token caps), and have a concrete **meta-engine rewrite opportunity** (where tweaking params is insufficient and rewriting/forking the engine, schedule, style, kernel, or motion law wins on `["shape", "model", "renderer"]`):
   - **Rendering / engine-rewrite seeds (5):**
     1. Naive slow raymarch scene → invites replacing schedule with vis-buffer + hardware RT `shade_tiles`
     2. Flat single-pass PBR diorama → invites custom multi-pass stylized ink/contour + screen-space outline pass
     3. Expensive per-pixel sky/atmosphere scene → invites dirty-flagged baked sky cubemap + custom lighting pass
     4. Multi-object variety scene with repeated kinds → invites data-driven `MaterialTable` uber-family + bindless/atlas
     5. Underwater/glass caustic diorama → invites custom volumetric/caustic pass or forked integrator under frame budget
   - **Motion / Spore + Disney springs + Mood seeds (4–5):**
     6. Stiff linear-animated quadruped → invites `capture_point_ff` + `solve_2bone_ik` + planted-foot `check`
     7. Rigid bouncing creature/prop → invites `stdlib::motion::springs` (`spring_chain_step`, `squash_stretch`, `anticipation_overshoot`)
     8. Neutral walker → invites `stdlib::motion::mood` transitions (`timid → proud → ferocious`) and custom mood authoring
     9. Non-crab creature (e.g. heron/serpent/beetle) → invites writing a custom gait/S-neck/stalk solver law
   - **GPU simulation / escape-hatch seeds (3–4):**
     10. Particle splash / granular pile → invites `kernel allow(atomics, workgroup)` P2G/MPM `NonDet` vs fixed-point `Det`
     11. Reactive wave/field surface → invites custom compute pass coupled into `World` shading
     12. Custom stylized post-FX / raw WGSL `pass` seed → demonstrates lawful engine escape hatch when stock schedules don't fit
2. **Add `tools/basin_test.sh`:**
   - Given a model/runner or dry-run mode, prepares a cold prompt containing ONLY `loft context` + 3 fresh non-crab/non-harbor prompts (e.g. "a clockwork beetle crossing rain puddles in ink wash", "a glass jellyfish pulsing over a bioluminescent reef", "an exhausted stone golem waking up with heavy spring follow-through"), and scores every candidate with `loft check`, `loft lint`, `loft profile`, and a still render.
3. Verify ALL seeds with `~/git/loft/target/release/loft check`, `loft lint`, `tools/check_loft_grammar.py`, and one quiet-GPU render smoke (`loft still` / `loft clip` on representative seeds). Push `origin loft/v15-seeds`, merge cleanly into `~/git/loft` `main` if green, remove the ephemeral worktree (`git cambium remove --safe v15-seeds`), delete the merged branch, and push `origin main`.

### Part 2 — Wire `~/git/creative` to Rust Loft v1.5
Work in `~/git/_wt/creative-loft-v15` on branch `loft/v15-meta-trainer` from `~/git/creative`'s `HEAD` (`git -C ~/git/creative cambium add -b loft/v15-meta-trainer -name creative-loft-v15 -ephemeral ~/git/_wt/creative-loft-v15`). **Do NOT break existing `Frozen` / `Vendor` TS contracts or tests!**

1. **Add a clean `LoftV15` (native Rust `.loft`) variant to `training/loft/library.py` & `render.py`:**
   - Follows "One Clean Path" sum-type dispatch (`Library = Frozen | Vendor | LoftV15`).
   - Invokes the Rust `loft` binary (`loft check`, `loft lint`, `loft clip --max-build-ms ... --max-frame-ms ... --json ...`, `loft context`), mapping exit codes `0..7` cleanly (`docs/spec/06_ADDENDUM.md §17`).
   - Keeps `Frozen` and `Vendor` intact and passing all existing `pytest` unit tests!
2. **Add `configs/loft/system-v15.txt` & `configs/loft/v15-meta.jsonc` + `configs/loft/v15-meta.tasks.jsonl`:**
   - `system-v15.txt` teaches the new `.loft` meta-engine (removes the legacy TS `"no custom shader code"` ban, embeds/references live `loft context`, explicitly rewards rewriting/forking engines, schedules, styles, kernels, and motion/mood laws — not just tweaking numbers).
   - Export the verified seeds from `~/git/loft/training/seeds/` into `training/loft_envs/seed_programs/loft-v15-meta/` (`seeds.jsonl`, `programs/`, `renders/`) in the exact schema `training/loft/data.py` and `build_seed_tasks.py` expect, and generate `configs/loft/v15-meta.tasks.jsonl` covering `generate`, `improve` (`["shape", "model", "renderer"]`), and `edit` tasks.
3. **Verify `~/git/creative`:**
   - Run existing `pytest` suite (`uv run pytest training/loft/test_*.py`) plus a new unit test for `LoftV15` contract + end-to-end smoke check (`selfcheck.py` or dry-run render of 2 `loft-v15-meta` seeds through the `LoftV15` adapter!).
   - Commit cleanly on `loft/v15-meta-trainer` (and push if remote exists; do NOT force-push or touch unrelated dirty files in `~/git/creative`).

Write `~/git/loft/agent_notes/2026-11-20_v1/lanes/v15_seeds_creative.md` (≤ 35 lines) summarizing seed counts, grammar/lint/render results, `creative` pytest results, and exact command to launch the first v1.5 training run!

## Rules (Superintelligence Compile Discipline)
- Write ALL seed files and ALL Python adapter/config files completely BEFORE running checks/tests. Zero small edit-test loops.
- Serialize any GPU renders through `~/git/loft/tools/gpu_lock.sh`.

Final message ≤150 words.
