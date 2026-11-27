# Lane r27-soundness: Compiler Soundness & Correctness Fixes

Effort: medium

## Mission
Fix the concrete compiler soundness and correctness bugs found in the Nov 27 audit while keeping 100% backward compatibility with all existing `.loft` rollouts (`out/spark_pairs/**/*.loft`, `corpus/**/*.loft`, `scenes/**/*.loft`, `stdlib/**/*.loft`, `engines/gates.json`).

## Strict File Ownership (DO NOT edit files outside this list)
- `crates/loft-eval/src/eval.rs`
- `crates/loft-eval/src/ad.rs`
- `crates/loft-eval/src/lip.rs`
- `crates/loft-eval/src/refine.rs`
- `crates/loft-check/src/check.rs`
- `crates/loft-passes/src/motion.rs`
- `crates/loft-passes/src/pipeline/stage0.rs`
- `crates/loft-format/src/bundle.rs`
- `crates/loft-abi/src/det.rs`
- Plus new unit tests inside those crates' existing `tests/` or `#[cfg(test)]` modules.

## Exact Fixes to Implement
1. **Unsound `map2` Lipschitz bound (`crates/loft-eval/src/eval.rs:1387-1395`):**
   - Currently `map2` discards componentwise `lip()` from `f(p, q)` and uses `la.hypot(*lb)` (falsely certifying vector `pow`, `step`, `atan2` as finite-Lipschitz, and omitting `sqrt(n)` on scalar broadcast).
   - Compute the resulting vector Lipschitz bound soundly from the componentwise results `c.iter().map(|s| s.lip())` (Euclidean norm of component Lipschitz constants, yielding `INFINITY` if any component is non-finite/unbounded), while preserving exact behavior for `Val::S` and `Val::V` when `f` is `min`/`max` (verify existing `interval_rules` and `lipschitz_*` tests pass!).
2. **`Stmt::For` mixed `i32`/`u32` bound swap (`crates/loft-eval/src/eval.rs:504`):**
   - Split `(Val::U(a), Val::I(b)) => (i64::from(a), i64::from(b), true)` and `(Val::I(a), Val::U(b)) => (i64::from(a), i64::from(b), true)` so `lo`/`hi` are never swapped.
3. **AD bugs (`crates/loft-eval/src/ad.rs`):**
   - Line 56: `cp()` must deep-clone `ExprKind::If`, `ExprKind::Unproved`, and `ExprKind::FiniteDiff` with fresh `ExprId`s instead of shallow-cloning.
   - Line 167: `BinOp::Rem` (`%`) has a jump discontinuity — return `E307` when either operand is active instead of silently returning `None` (zero derivative).
4. **Broken comparison operator typechecking (`crates/loft-check/src/check.rs:1619`):**
   - Restrict `Lt | Le | Gt | Ge` to numeric types (`num(x) && num(y)`) and `Eq | Ne` to comparable scalar/bool/row/vec types (NOT `Bsdf`, `Xform`, `Map`, `World`). Ensure all existing `stdlib/`, `scenes/`, `corpus/`, and `out/spark_pairs/` files still check cleanly!
5. **Transitive E504 determinism check on `Det` kernels (`crates/loft-check/src/check.rs:104-120`):**
   - Run the same transitive `MotionMath::Unavailable` + raw-WGSL check on `Det` `Item::Kernel` declarations that currently only runs on `p.laws.values()`.
6. **Sticky `NonDet` tier (`crates/loft-passes/src/motion.rs:60`):**
   - Replace `tier = law.tier;` with sticky `NonDet` propagation so a `Det` law after a `NonDet` law cannot erase `NonDet`.
7. **Duplicate `FieldMedium` emission (`crates/loft-passes/src/pipeline/stage0.rs:149-159`):**
   - Ensure `add_media` runs once, not once per `PointLamp`.
8. **`validate_wgsl` capabilities (`crates/loft-format/src/bundle.rs:214-231`):**
   - Enable the valid WGSL capabilities that Loft bundles legitimately emit (`SUBGROUP`, `SHADER_FLOAT32_ATOMIC`, `RAY_QUERY`, `FLOAT16`) in `naga::valid::Validator::new` instead of `Capabilities::default()`.
9. **`det.rs` vs `det.wgsl` parity (`crates/loft-abi/src/det.rs`):**
   - Implement `pub fn det_log(x: f32) -> f32` matching `crates/loft-abi/src/det.wgsl:64-80`.
   - Fix `det_sin` at `det.rs:20` to use WGSL `sign(r)` semantics (`if r > 0.0 { 1.0 } else if r < 0.0 { -1.0 } else { 0.0 }`) instead of `r.signum()`, so `det_sin(-0.0)` matches `det.wgsl` bit-for-bit.

## Rules & Verification
- **Write ALL code across all files BEFORE running your first compile.** Target 2–3 compiles total via `CARGO_BUILD_JOBS=2 tools/dev.sh`.
- Add focused unit tests covering every single fix above (e.g. vector `step`/`pow` Lipschitz rejection, mixed `i32`/`u32` loop, `Bsdf < Bsdf` rejection, `Det` kernel E504, sticky `NonDet`, `det_log` & `det_sin(-0.0)`, `validate_wgsl` with subgroups/ray_query).
- Run focused tests: `tools/dev.sh test loft-abi`, `tools/dev.sh test loft-eval`, `tools/dev.sh test loft-check`, `tools/dev.sh test loft-format`, `tools/dev.sh test loft-passes`.
- Commit cleanly on your branch with explicit paths. Do NOT push or merge to `main`.
- Keep final summary concise (<= 200 words) with exact files changed, tests run, and any edge cases found.
