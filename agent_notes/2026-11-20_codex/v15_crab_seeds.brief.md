# Step v15_crab_seeds — Migrate Crab-Overfit Seeds to `.loft` v1.5 + Generate Full-Engine-Breadth Crab Seeds

Read `~/git/loft/AGENTS.md`, `~/git/loft/agent_notes/2026-11-20_v1/SPRINT_TO_TRAIN.md`, `~/git/creative/configs/loft/crab-curriculum.jsonc`, `~/git/creative/configs/loft/crab-actions.instructions.jsonl`, `~/git/creative/training/loft/build_seed_tasks.py`, `~/git/creative/training/loft/data.py`, and inspect `~/git/creative/training/loft_envs/seed_programs/crab-actions-v2/seeds.jsonl`.

Also run `~/git/loft/target/release/loft context` first so every seed uses the exact live `.loft` v1.5 syntax, `stdlib/` imports, `springs.loft`, and `mood.loft`!

## Goal

Build the **`crab-overfit-v15`** seed pack so crab-overfit RL training starts with BOTH:
1. **Migrated crab-overfit seeds** (preserving continuity with `crab-actions-v2`'s train/eval splits and edit-instruction anchors), AND
2. **New crab-overfit seeds exercising the FULL meta-engine breadth** — so the learner sees editable `schedule`/`pass` engines, PBR + all 7 styles, `MaterialTable`, hardware RT, Spore gait/IK, Disney springs, composable `Mood` transitions, and GPU physics directly inside the crab starting programs!

## Task

Work in `~/git/_wt/creative-crab-seeds` on branch `loft/v15-crab-seeds` in `~/git/creative` (`git -C ~/git/creative cambium add -b loft/v15-crab-seeds -name creative-crab-seeds -ephemeral ~/git/_wt/creative-crab-seeds origin/loft/v15-runtime` — or `loft/v15-runtime` local branch if not pushed).

1. **Migrate 24–30 diverse stratified seeds from `crab-actions-v2/seeds.jsonl` → idiomatic `.loft` v1.5:**
   - MUST include the 5 eval/edit anchor IDs referenced by `crab-curriculum.jsonc`:
     - `v1/train-11/1`, `r38retry-before/before-0/1` (improve eval)
     - `v1/train-1/1`, `v1/train-14/0` (edit eval)
     - plus `v1/train-0/0`, `v1/train-0/1`, `v1/train-1/0` and a diverse spread across styles (`toon`, `woodblock`, `biolume`, `papercraft`, `pbr`) and silhouettes/poses!
   - Translate faithfully into clean, compact `.loft` v1.5 (using `kind`, `build for`, `stdlib::sdf::solid`, `stdlib::materials::bsdf`, `stdlib::styles::*`, `stdlib::engines::*` — NO bloated raw float dumps, passing `loft lint`!).

2. **Generate 16–20 brand-new crab seeds covering the FULL meta-engine breadth (environment `loft_0004`, split across `train` and `eval`):**
   - **Meta-engine / custom renderer crabs (5–6):**
     - Custom forked `schedule` + stylized ink/contour `pass` crab (demonstrating inline engine editing!)
     - Vis-buffer + hardware RT shadows/GI PBR crab with reflective wet carapace (`ggx` + `coat`)
     - Data-driven `MaterialTable` multi-material coral/barnacle/shell-encrusted boss crab (`0 ms` row variety)
     - Dirty-flagged baked sky cubemap sunset/tidepool crab
     - Volumetric/caustic underwater reef crab with custom lighting pass
   - **Spore motion + Disney springs + Composable Mood crabs (6–8):**
     - Spore procedural `Walker` crab (`capture_point_ff`, `solve_2bone_ik`, convex `hull`, planted-foot `check`)
     - Disney spring follow-through crab (`stdlib::motion::springs`: lagging eye stalks, waving antennae, squash-and-stretch hop, anticipation/overshoot claw snap)
     - 4 distinct `stdlib::motion::mood` crabs (`timid` retreating crab, `jaunty` strutting crab, `ferocious` epic boss crab, `exhausted` heavy-limbed crab + live `mix_mood` transition — directly pairing with `"make the subject look sad"`, `"smile"`, `"epic boss"` edit tasks!)
     - Style-diverse crabs covering `woodblock`, `sketch`, `neon`, `biolume`, `papercraft`, `pbr`, `toon`
   - **GPU physics / compute crabs (3–4):**
     - `kernel allow(atomics, workgroup)` P2G sand/splash/bubble crab (`NonDet` splash or `Det` tide particles)
     - Raw WGSL custom post/shimmer `pass` crab demonstrating lawful escape-hatch authoring

3. **Validate, Render & Export `crab-overfit-v15`:**
   - Validate **100%** of seeds (all 40–50 seeds) with:
     - `~/git/loft/target/release/loft check` (exit 0)
     - `~/git/loft/target/release/loft lint` (clean / within token cap)
     - `~/git/loft/tools/check_loft_grammar.py` (both Earley + llguidance)
   - Render all 4 timestamps `[0, 0.37, 0.91, 1.53]` at `512×512` (serialized via `~/git/loft/tools/gpu_lock.sh`), compute SHA-256 receipts, and write the complete self-contained seed pack to:
     - `training/loft_envs/seed_programs/crab-overfit-v15/{seeds.jsonl, frames/, manifest.json, SHA256SUMS, REPORT.md}` (matching `training/loft/data.py`'s exact schema, embedding live `loft context` in `system`!).
   - Generate **`configs/loft/crab-v15-overfit.tasks.jsonl`** combining `generate`, `improve` (`["shape", "model", "renderer"]`), `improve_motion` (`["action", "integrity", "manner", "animation_principles"]`), and `edit` tasks (using `crab-actions.instructions.jsonl`) and verify `configs/loft/crab-v15-overfit.jsonc` loads cleanly with `training/loft/config.py`!
   - Commit cleanly on `loft/v15-crab-seeds` and write `~/git/loft/agent_notes/2026-11-20_v1/lanes/v15_crab_seeds.md` (≤ 35 lines).

## Rules (Superintelligence Discipline)
- Write the seed generator/migration script and ALL new crab `.loft` seeds completely BEFORE running the batch check & render pass. Serialize GPU renders through `~/git/loft/tools/gpu_lock.sh`. Do NOT touch Rust code.

Final message ≤150 words.
