# Lane c4-stdlib-reorg: Organize & Grow Clean Composable `stdlib/` (Zero Rollout Breakage)

Effort: high
Mode: CODE-ONLY (LOFT_NO_COMPILE=1 — DO NOT run cargo, rustc, tools/dev.sh, or tools/check.sh! You MAY run `python3 tools/check_loft_grammar.py` for instant syntax verification.)

## Mission
Reorganize and strengthen `stdlib/` so it embodies `docs/PHILOSOPHY.md` — clean, well-organized, composable, forkable `.loft` libraries that cut token bloat for both cheap models and Opus, while keeping **100% backward compatibility** with all existing scenes, `out/spark_pairs/`, `corpus/`, and `engines/gates.json`!

## Strict File Ownership (ONLY edit/add files under these paths)
- `stdlib/`
- `scenes/showcase/std1e/` (ONLY deduplicating `gait.loft` / `tidal_fork.loft` cleanly)

## Exact Improvements to Implement
1. **Add reusable stock shapes (`stdlib/shapes/primitives.loft`):**
   - Provide clean, forkable `pub kind Capsule @World (...)` and `pub kind GlossBead @World (...)` matching the exact idioms used across `out/spark_pairs/` (so future rollouts can `use stdlib::shapes::primitives::{Capsule, GlossBead}` instead of repeating 26 lines per file!).
2. **Add missing core SDF primitives (`stdlib/sdf/solid.loft`):**
   - Add `pub fn ellipsoid[S](v: v3@S, r: v3) -> f` and `pub fn smax(a: f, b: f, k: f) -> f` (re-implemented 4× and called 212× across Opus character builds!).
   - Keep existing `box_sdf`/`round_box`/`capsule`/`sphere`/`smin`/`ssub` signatures 100% backward-compatible.
3. **Clean up `stdlib/` dead imports & misleading docs:**
   - Remove unused `use stdlib::materials::bsdf::{ggx}` in `stdlib/geometry/mesh.loft:1`.
   - Remove unused `agx_display` import in `stdlib/engines/spectral_caustic.loft:1`, and check `restir` unused imports in `reference.loft`/`spectral_caustic.loft` (keep only if required by compiler-generated WGSL — verify with `rg reservoir_ crates/` first!).
   - Fix `stdlib/README.md`: accurately document what `stdlib/core.loft` auto-imports, remove stale `## Contract for lane D` notes, and add a clean directory map separating:
     - `stdlib/engines/` (actual render engine schedules + note legacy gate scene paths kept for `engines/gates.json`)
     - `stdlib/shapes/` (stock drawable kinds)
     - `stdlib/sdf/`, `stdlib/geometry/`, `stdlib/materials/`, `stdlib/lighting/`, `stdlib/integrators/`, `stdlib/sky/`, `stdlib/post/`, `stdlib/styles/`, `stdlib/motion/`, `stdlib/noise/`, `stdlib/math/`, `stdlib/physics/`.
4. **Deduplicate `stdlib/motion/` & remove hardcoded crab anatomy from generic walker helpers (CAREFUL: preserve `crab_live`, `crab_rows`, `crossing`, `tidal`, `tidal_fork`, `motion_bodies/*`!):**
   - Read `stdlib/motion/{body,gait,legs,audit}.loft` and `scenes/showcase/std1e/{gait,tidal,tidal_fork,crossing}.loft` completely first!
   - Parameterize `swing_height` (or extract shared gait core helpers) cleanly so `scenes/showcase/std1e/gait.loft` does NOT need to be a 646-line verbatim copy of `stdlib/motion/gait.loft` just to change one constant (`0.115m` → `0.185m`)!
   - Note the span-hash warning comment at `stdlib/motion/gait.loft:648-649` — preserve exact behavior/signatures of `stdlib/motion/gait.loft` if any existing test pins its check hash, and document clearly what was deduplicated vs preserved.
   - Add clean `stdlib/motion/act.loft` and `stdlib/motion/roam.loft` (and keep thin `pub use` re-exports at `stdlib/engines/{act,roam}.loft` so `scenes/{crab,heron,boss}.loft` keep working with zero edits!).

## Rules (ZERO-COMPILE MODE)
- **DO NOT run `cargo`, `rustc`, `tools/dev.sh`, or `tools/check.sh`.**
- You MAY run `python3 tools/check_loft_grammar.py` on every `.loft` file you create or edit to guarantee 100% grammar validity!
- Keep `stdlib/engines/{capsules,blobs,hermit,coral_shrine,realtime,woodblock,reference,spectral_caustic}.loft` paths and behavior 100% intact for `engines/gates.json`.
- Commit cleanly on your branch by explicit path. Do NOT push or merge.
- Final report (<= 250 words): exact files added/cleaned, token savings enabled for new scenes, and grammar check results.
