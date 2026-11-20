Effort: xhigh

You are a Sol engineer on Loft's material compiler and runtime lane. Repo ~/git/loft, from origin/main containing the capability-gaps proposal. You have no other context.

Read CORE.md, docs/PHILOSOPHY.md, docs/spec/README.md, docs/V1_5_GOALS.md G3, agent_notes/2026-11-20_v1/OPEN.md and docs/proposals/2026-11-20_capability_gaps.md §2. Inspect the staged BSDF shape, E520 draw-kind cap, Vis row identity, Tier 0 promotion path and pass budgets in engines/gates.json.

Rules

- Use `git cambium add -b loft/cap1c -name loft-cap1c -ephemeral ~/git/_wt/loft-cap1c origin/main`.
- Write the 10/100/1,000 cost model and author-facing example first. Iterate compile errors with `CARGO_BUILD_JOBS=2 tools/dev.sh check CRATE`. Use existing table and binding descriptors. Preserve kind identity and the authored CPU model.
- Own material layout/family planning, material lookup and generated family shading, associated table/texture binding and the fixtures for this lane. Keep the raster coverage contract shared with cap1a and the World hit interface shared with cap1d.
- Use `tools/gpu_lock.sh` for every GPU run. Measure quiet paired timings with a triangle p95 <0.15 ms, no concurrent builds and 95% intervals. Preserve reference pictures. Run focused checks and the workspace suite once.
- Preserve generated shaders, per-scene plans, cold keys and raw timings under the primary checkout's out/meta/cap1c/. Write agent_notes/2026-11-20_v1/lanes/cap1c.md. If two fixes fail without narrowing the cause, report the reproduction and both observations.
- Commit by path and push loft/cap1c for integration. End commit messages with `Co-Authored-By: GPT (codex) <noreply@openai.com>`. Preserve ignored evidence before safe Cambium removal.

Task

1. Render 10/100/1,000 unique procedural materials from parameter rows at fixed geometry, coverage and resolution. Also build the v1.5 200-material PBR/stylised fixture with ≤3 material pipelines. At 1,000, target ≤1 s cold load and whole-frame p95 within 10% of ten. Edit a row with zero pipeline creations and verify the targeted pixels change.
2. Force each budget to be exceeded independently. Automatically merge compatible material-only kinds before E520, preserve row/kind picking and motion lookup, and identify genuinely incompatible geometry or pipeline states. Print physical family, budget and remaining specialization in the plan. Prove bindless and portable atlas/array paths on sorted and shuffled IDs, including mip edges and paged textures. Verify source and bundle/host execution.
3. Compare per-kind specialization, a hand-written family shader, compiler-generated family shading and the supported dirty Tier 0 tape. Separate cold compilation from shade, classify and whole-frame p95. Target ≤0.30 ms family overhead against the hand-written four-family control at 1,000. Record unsupported per-kind counts as E520 and label microbenchmark extrapolations.
4. Trace a two-bounce path through two families with matching eval/sample/PDF. Exercise a structural graph edit and load one BSDF/light bundle through a versioned dispatch set or queued passes. Verify signature rejection, stable in-flight IDs and promotion. Keep the closed family case branch-free where its ID is already uniform.

Final message ≤120 words.

**R3 Variety Addendum (20 Nov, supersedes Task #2 auto-merge):**
Per `docs/V1_5_GOALS.md` R3: implement 6–10 explicit material-family uber-shaders (`StandardPbr`, `LayeredCoat`, `StylizedInk`, `DielectricGlass`, `Emissive`, `Foliage`) over a packed `MaterialTable` (64–128 B rows) plus bindless textures (with `texture_2d_array`/atlas fallback on web) at 0 ms compile per new material. **CUT** automatic compiler merging of arbitrary kinds under a pipeline budget — compiled custom `kind`s + pipeline cache stay beside uber-families for novel math. **REJECT** using Tier 0 VM for full-screen shading (Tier 0 is for live geometry previews only).
