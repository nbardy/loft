# Lane r27-gates-and-grammar: Honest Verification Gates, CLI Parity & Grammar Sync

Effort: medium

## Mission
Close the verification gate loopholes, fix `loft render --time` / quantile inconsistencies, and bring `grammar/loft.lark` into sync with what `parser.rs` actually accepts — 100% additive and non-breaking for all existing scenes and rollouts.

## Strict File Ownership (DO NOT edit files outside this list)
- `tools/loft-verify/src/suite.rs`
- `tools/loft-verify/src/controls.rs`
- `crates/loft-cli/src/render.rs`
- `crates/loft-cli/src/lib.rs`
- `crates/loft-cli/src/profile.rs`
- `crates/loft-cli/src/clip.rs`
- `grammar/loft.lark`
- `docs/spec/loft.lark`
- `tools/check_loft_grammar.py`
- Plus tests in `tools/loft-verify/tests/` or `crates/loft-cli/tests/`.

## Exact Fixes to Implement
1. **Stop `suite.rs` from silently exiting 0 when Speed gate is contended (`tools/loft-verify/src/suite.rs:752-762`):**
   - Currently `failed_in` and `failed` filter out `!c.contended`, so a contended Speed gate returns `Ok(())` (exit 0)!
   - Fix `result()` so contended checks return `Err(Failure::Device(...))` (exit 6 — matching the documented device/contention exit code) instead of `Ok(())`, while keeping real failures (Truth / Parity / Load / Budget) taking precedence over contention!
   - Add a unit test in `tools/loft-verify/src/suite.rs` (`#[cfg(test)]`) proving `result()` returns `Err(Failure::Device(_))` when any check is `contended: true` and all others hold.
2. **Fix `controls.rs` contention probe and skipped-shadow timestamp bug (`tools/loft-verify/src/controls.rs`):**
   - At `controls.rs:743-745`, `whole_frame_ms` computes `frame.ticks.iter().step_by(2).min()` without filtering out `0` ticks spliced in when a static shadow pass is skipped, corrupting `whole_frame_ms`. Filter out zero pairs `(s, e)` where `s == 0 && e == 0` (matching `loft-native/src/lib.rs:253-269`) and add a unit test!
   - Replace the deprecated 1-triangle contention probe in `controls.rs` with `gpu.load_control()` run both **before and after** each block (matching `crates/loft-cli/src/profile.rs:91-104` and `loft-native/src/lib.rs:313-315`), flagging contended blocks honestly.
3. **Fix `loft render --time T` missing motion sampling (`crates/loft-cli/src/render.rs:79-119`):**
   - Unlike `source_still` (`render.rs:13-20`) and `clip.rs:156-174`, `loft render --time T` does not sample motion dispatches into `compile_snapshot_aov` when the scene has motion laws. Wire up the same `sample_motion` -> `compile_snapshot_aov` path so `loft render` at `--time T > 0` matches `loft still` and `loft clip`.
4. **Unify quantile helper across `loft-cli` (`crates/loft-cli/src/lib.rs`, `profile.rs`, `clip.rs`):**
   - Replace the inconsistent `round()` vs nearest-rank `div_ceil` percentile math with one shared, unit-tested `pub fn percentile_nearest_rank(sorted: &[f64], p: f64) -> f64` helper in `loft-cli/src/lib.rs` used by `profile.rs` and `clip.rs`.
5. **Sync `grammar/loft.lark` + `docs/spec/loft.lark` with `parser.rs` (additive only):**
   - Add missing operators/forms already accepted by `parser.rs` and used in real scenes:
     - Bitwise operators (`|`, `^`, `&`, `<<`, `>>`) in expression precedence.
     - Integer suffixes `u`/`i` on `NUMBER` (`0u`, `3i`).
     - Optional `(...)` on fieldless `kind` (`kind Sand @World { ... }`).
     - Update `tools/check_loft_grammar.py` so leading-operator line continuations (`+`, `*`, `/`, `%`, `==`, `!=`, `<=`, `>=`, `&&`, `||`) match `parser.rs:35-38`, and fix the `"wgsl"` identifier trigger bug (`check_loft_grammar.py:70`) so it only triggers raw-block mode for `wgsl(` (not arbitrary identifiers named `wgsl`).
   - Keep `grammar/loft.lark` and `docs/spec/loft.lark` **byte-identical**.
   - Verify `python3 tools/check_loft_grammar.py` passes 100% of fixtures AND all `.loft` files in `out/spark_pairs/`!

## Rules & Verification
- **Read all owned files + `crates/loft-parse/src/parser.rs:25-45, 825-845, 1650-1675` completely BEFORE editing.**
- **Write ALL code before your first compile.** Target 2–3 compiles total via `CARGO_BUILD_JOBS=2 tools/dev.sh`.
- Run `python3 tools/check_loft_grammar.py` (instant, no compile), `tools/dev.sh test loft-verify`, `tools/dev.sh test loft-cli`.
- Commit cleanly on your branch with explicit paths. Do NOT push or merge to `main`.
- Keep final summary concise (<= 200 words) with exact files changed and tests run.
