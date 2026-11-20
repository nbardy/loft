# Lane: port creative's image-reconstruction pipeline and its generated data to the new Loft

Worktree: ~/git/_wt/creative-reconstruct (branch loft/reconstruct-migrate, from creative main b7f79a82). The data lives
git-ignored in the MAIN checkout: ~/git/creative/3d_scrape/data/ (read it there; never modify or delete anything in it;
write new outputs beside it). Read art/reference_sculptures/loft/README.md first, then recreate.py, dataset.py,
native_build.py, build.ts, native_bake.ts, native-prompt.md, python_build.py, seed_sft.py, render_tools.py.

Facts: the reconstruction runner reaches Loft through LOFT_ROOT (default ~/git/loft) and `bun src/cli/main.ts context`;
dataset.py snapshots a runtime copy of Loft into each dataset (out/runtime/loft). The pilot
(3d_scrape/data/reconstruction-pilot, 50 samples; reconstruction-v1, 4 samples; the 2-trajectory seed
art/reference_sculptures/loft/examples/seed-trajectories-loft-python.jsonl and 3d_scrape/data/seed-trajectories-loft-python.jsonl;
the interrupted reconstruct-curated-v1 queue) was generated against an OLD Loft. Loft main has changed a lot (see
~/git/loft/docs/proposals/2026-11-18_authoring_api.md, docs/api.md). A sibling lane is writing
~/git/_wt/loft-migration/docs/MIGRATION.md and the codemod ~/git/_wt/loft-migration/tools/migrate.ts; it is finished when
~/git/loft/agent_notes/2026-11-18_cook/migration/codemod.done exists (poll for it with a sleep loop before the data step).
Training (training/loft) is being migrated on branch loft/migrate in ~/git/_wt/creative-loft-migrate (vendor/loft
submodule, prompts from the pinned Loft's `loft context`): SFT and RL must use the SAME Loft API.

Do:
1. Scripts: make the reconstruction runner, dataset builder, native build/bake, capture and seed_sft use the same
   pinned Loft as training (the vendor/loft submodule; add/init it in this worktree if this branch lacks it, at
   ~/git/loft origin/main), with the Loft commit recorded in every run/sample manifest. Prompts and catalogs come from
   that Loft's `loft context`, never a stale copy (check native-prompt.md / prompt.md / python-prompt.md for hand-written
   API text that is now wrong; fix or generate it). Python and recipe methods: check whether they touch Loft at all;
   port what does. Only `uv` for Python.
2. Data (after codemod.done): write a ported copy of every Loft-authored state (each round's scene.ts / model source in
   reconstruction-pilot, reconstruction-v1 and both seed-trajectory files) to new directories beside the originals,
   e.g. 3d_scrape/data/reconstruction-pilot-loftmain/ (same layout; provenance per sample: old source sha256, old Loft
   commit, new Loft commit, codemod version, hand-edited yes/no). Port with the codemod; hand-fix what it reports
   (keep the art; change only what the API requires). Re-render each ported state with the SAME camera and studio as
   the original so trajectories' observation images match their code; record any visual drift.
   Non-Loft methods (python, recipe) copy over unchanged if they do not depend on Loft.
3. Rebuild pairs.jsonl / samples.jsonl exports for the ported sets with the existing exporters, and the seed SFT
   export (seed_sft.py --export) for the ported seed, offline only (no --train, no Tinker, no provider calls).
4. Do NOT restart generation (it calls a paid provider). Instead write the exact resume command for the interrupted
   queues on the new Loft into README.md, and a short MIGRATION note under agent_notes/.
Rules: no full test suites (shared machine); focused checks only; small renders; no paid calls. Commit on
loft/reconstruct-migrate staging by path (never git add -A; never data_scraping/; never the git-ignored data), message
ending with: Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>. Do not push. Final message: 200 words with counts
(states ported by codemod / hand-fixed / failed with reasons; renders redone; drift) and the commit SHA.
