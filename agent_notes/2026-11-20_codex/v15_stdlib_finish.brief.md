# Step v15_stdlib_finish — Recover & Finish Held Wave-X4 Branches (std1d + std1c + port1)

Read AGENTS.md, docs/V1_5_GOALS.md (G6.3, G6.4/R4, G6.5), and `out/meta/ix4/BLOCKED`.

All three Wave-X4 branches already did 90% of the work and only stalled because `std1c` and `std1d` were built in parallel without seeing each other, and `port1` had 3 grammar errors. Recover and merge all three cleanly onto `origin/main` (after `v15_exec`) in ONE pass so zero work is lost.

## Task

Work in `~/git/_wt/loft-v15-stdlib-finish` on branch `loft/v15-stdlib-finish` from `${LOFT_WAVE_BASE:-origin/main}` (use `git cambium add -b loft/v15-stdlib-finish -name v15-stdlib-finish -ephemeral ~/git/_wt/loft-v15-stdlib-finish ${LOFT_WAVE_BASE:-origin/main}`).

1. **Merge & finish `origin/loft/std1d` (`c61c9ba`) — Integrators + R4 Contract:**
   - Add the missing R4 physical material query `fn material(hit: SurfaceHit) -> Bsdf` alongside `fn shade_style(hit: SurfaceHit, light: LightSummary) -> rgb` (and compile-time refusal when an integrator lacks `LightSummary`).
   - Switch transport from `path_random` to the shared counter binding (matching `std1b`'s `D-HANDOFF.md`).
   - Fix the 128/256-spp Harbor depth NaN at `(0,0)`.
2. **Merge & finish `origin/loft/std1c` (`012391b`) — 7 Surface Styles + Harbor Showcases:**
   - Remove `std1c`'s temporary source-rewriting adapter now that `std1d`'s `stdlib/lighting/surface.loft` is right beside it.
   - Verify Harbor woodblock compiles cleanly without `E011` and all 7 styles + one-import swaps pass both recognizers and preserve Vis64/depth.
3. **Merge & finish `origin/loft/port1` (`3c6f053`) — 5-Crab Lineup, Crab Live & Hermit:**
   - Fix the grammar errors in `scenes/crab_lineup.loft`, `scenes/hermit.loft`, and `scenes/lineup/cast.loft` so **100%** of tracked `.loft` files pass BOTH Earley and llguidance grammar recognizers.
   - Preserve `crab_live`'s 480/480 CPU/GPU twin parity and the existing approval stills in `out/meta/{port1,std1c,std1d}/approval/`.
4. **Verify (Write ALL code first, compile rarely!):**
   - Do NOT compile in small edit-compile loops! Read all files first, perform ALL merges/edits across `stdlib/`, `scenes/`, and crates in one complete pass, and ONLY THEN run ONE `CARGO_BUILD_JOBS=2 tools/dev.sh check loft-cli`, fix every error at once, run focused tests + BOTH grammar recognizers (`100%` pass).
   - Write `agent_notes/2026-11-20_v1/lanes/v15_stdlib_finish.md` (≤ 30 lines) and push `origin loft/v15-stdlib-finish`.

Final message ≤150 words.
