# Step v15_meta_seeds — General Full-Breadth Meta-Engine "Improve" Seeds & Basin Test (`~/git/loft`)

Read `~/git/loft/AGENTS.md`, `~/git/loft/agent_notes/2026-11-20_v1/SPRINT_TO_TRAIN.md`, and run `~/git/loft/target/release/loft context` first so every seed uses live `.loft` v1.5 syntax, `stdlib/`, `springs.loft`, and `mood.loft`.

## Goal

Build the non-crab **general meta-engine "improve" seed pack** (`training/seeds/` in `~/git/loft` + exported `loft-v15-meta` pack) and the repeatable **basin test** (`tools/basin_test.sh`) so training generalizes beyond crabs and teaches models to rewrite engines, schedules, styles, kernels, and creature motion across arbitrary subjects.

## Task

Work in `~/git/_wt/loft-v15-meta-seeds` on branch `loft/v15-meta-seeds` from `origin/main` (`git cambium add -b loft/v15-meta-seeds -name v15-meta-seeds -ephemeral ~/git/_wt/loft-v15-meta-seeds origin/main`).

1. **Create 12–15 compact, self-contained non-crab `.loft` seed programs in `training/seeds/` + `training/seeds/manifest.jsonl`:**
   Each seed must compile cleanly (`loft check`), pass `loft lint`, pass both grammar recognizers, stay concise, and present a clear **engine-level improvement opportunity** (not just param tweaks):
   - **Rendering / engine-rewrite seeds (5):**
     1. `raymarch_lantern.loft` — naive slow raymarch shrine/lantern → invites vis-buffer + hardware RT `shade_tiles`
     2. `flat_teapot_pbr.loft` — plain single-pass PBR cracked teapot → invites custom multi-pass stylized ink/contour + screen-space outline pass
     3. `sunset_lighthouse.loft` — expensive per-pixel atmosphere lighthouse → invites dirty-flagged baked sky cubemap + custom lighting pass
     4. `market_stalls_variety.loft` — repeated material kinds → invites packed `MaterialTable` uber-family (`0 ms` variety)
     5. `glass_jellyfish_caustics.loft` — underwater glass jellyfish → invites custom volumetric/caustic pass or forked integrator
   - **Spore motion + Disney springs + Mood seeds (4–5):**
     6. `stiff_beetle.loft` — rigid clockwork beetle → invites `capture_point_ff` + `solve_2bone_ik` + planted-foot `check`
     7. `rigid_golem.loft` — stiff stone golem → invites `stdlib::motion::springs` (`spring_chain_step`, `squash_stretch`, `anticipation_overshoot`)
     8. `heron_moods.loft` — neutral marsh heron (`solve_s_neck_ik`) → invites `stdlib::motion::mood` transitions (`timid → alert → proud`)
     9. `sea_serpent_wave.loft` — segmented sea serpent → invites custom travelling-wave + spring-crest solver law
   - **GPU simulation / escape-hatch seeds (3–4):**
     10. `tidepool_splash.loft` — water/sand splash → invites `kernel allow(atomics, workgroup)` P2G/MPM `NonDet` vs `Det`
     11. `reactive_pond.loft` — stepping stones over pond → invites custom GPU wave compute pass coupled into `World` shading
     12. `aurora_prism.loft` — optical aurora/prism diorama → invites raw WGSL `pass` escape hatch when stock schedules fall short

2. **Add `tools/basin_test.sh`:**
   - Prepares cold prompts containing ONLY `target/release/loft context` + 3 fresh non-crab/non-harbor subjects ("a clockwork beetle crossing rain puddles in ink wash", "a glass jellyfish pulsing over a bioluminescent reef", "an exhausted stone golem waking up with heavy spring follow-through").
   - Includes a `--check <dir>` mode that scores every generated `.loft` file with `loft check`, `loft lint`, `tools/check_loft_grammar.py`, `loft profile`, and renders a contact sheet via `tools/gpu_lock.sh`.
   - Smoke-test `--check` against `training/seeds/` itself so the harness is proven working!

3. **Verify & push:**
   - Verify 100% of `training/seeds/*.loft` with `target/release/loft check`, `target/release/loft lint`, `tools/check_loft_grammar.py`, and render 4 representative stills under `tools/gpu_lock.sh`.
   - Commit cleanly on `loft/v15-meta-seeds`, push `origin loft/v15-meta-seeds`, and write `agent_notes/2026-11-20_v1/lanes/v15_meta_seeds.md` (≤ 30 lines).

## Rules (Superintelligence Discipline)
- Write ALL 12–15 seed `.loft` files, `manifest.jsonl`, and `tools/basin_test.sh` in one decisive sweep BEFORE running the single batch check & render pass. Do NOT touch Rust crates or `~/git/creative` (owned by `v15_crab_seeds` & `creative_v15_runtime`).

Final message ≤150 words.
