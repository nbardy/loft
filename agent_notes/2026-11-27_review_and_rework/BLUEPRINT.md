# Loft 10k Meta-Engine Refactor Blueprint — 27 Nov 2026

> **Mode:** Code-only parallel refactor (`LOFT_NO_COMPILE=1`). Workers cut, reorganize, and reimplement cleanly in code without running `cargo`/`rustc`/`tools/dev.sh`; we compile and verify ONCE after assembling the refactored tree.

---

## 1. Where We Are vs. Where We Are Going

- **Today (`main` `f858d61f`):** **29,362 Rust core LOC** across 16 crates (`scc` under `crates/`, excluding `tests/`).
- **Target:** **~9,600–11,500 Rust core LOC** across **4 clean layers** (~60–67% reduction), plus an **uncapped, clean, composable `stdlib/`**.
- **Non-negotiable contract:** Keep 100% of existing `.loft` consumer rollouts (`out/spark_pairs/`, `corpus/`, `scenes/`, `engines/gates.json`, `tests/characters/`) compiling and rendering with identical consumer schedule verb signatures and strictly additive new language features.

| Target Layer | Today (LOC) | Target (LOC) | Net Cut | What Stays vs. What Moves / Dies |
|---|---:|---:|---:|---|
| **1. `loft-lang`** (`parse`+`check`+`eval`+`emit`+`passes`+`builtins`) | 15,116 | **~5,500** | **−9,600** | **Keep:** Pratt parser, `@Space` + `writes` + E504 checker, stage-0 `build` eval, sound interval/Lipschitz (`lip.rs`), clean AD (`ad.rs`), structured WGSL emit + generic pass-graph lowering. **Cut/Move:** `cst.rs`, `refine.rs`, `kernel_units.rs`, `sexp.rs`, `fusion.rs`, `bvh/twin.rs`, `path_twin.rs`, `check/bsdf.rs`, `defaults.rs` magic `woodblock`, 24-file `scope.rs` whitelist, hardcoded Vis64/shadow/shade_tiles/spectral_path/world_medium string surgery → move engines to `stdlib/render/`. |
| **2. `loft-runner`** (`std`+`wgpu`+`runner`+`native`) | 7,264 | **~1,800** | **−5,460** | **Keep:** ONE generic `wgpu` pass-graph executor (native + WebGPU, `Runner::attach` + `encode_into`, `Execution` v3 cull/blend/sampler/vertex state, timestamps, `StagingBelt`). **Cut/Move:** Delete `loft-native` (fold ~150 LOC headless driver into CLI), delete `loft-wgpu/src/web.rs`, move `compute_twin.rs` + `stepping.rs` to `tools/loft-verify/`, move `tests.rs` & `examples/` out of `src/`, collapse thin wrapper crates into ONE `loft-runner`. |
| **3. `loft-cli`** | 2,435 | **~1,200** | **−1,235** | **Keep:** `check`, `show`, `build`, `still`, `clip`, `profile`, `context`, `fork`. **Cut/Move:** Move `examples/` (624) and `diff.rs` (256) to `tools/loft-verify/`, delete stub `fmt.rs`. |
| **4. `loft-abi` + `loft-ir` + `loft-format`** (+ `loft-gltf`) | 2,063 | **~1,100** | **−963** | **Keep:** Generic wire structs (`BvhNode`, `LeafRef`, `VisRec`, `VerdictRecord`, `AdvanceArgs`, `det`), `Bundle` + `Execution` v3, `LOFTB001` + `.loftlog`. **Cut/Move:** Strip creature structs (`CreatureHeader`, `FootPin`, `ManipState`, `TrailNode`, `BodyRow`, `ChainRow`, `SegmentRow`) out of `loft-abi`; delete unused `Op::Intrinsic`/`IrModule`/`PassRegistry`; move `loft-gltf` out of core into `tools/` or optional feature crate. |
| **Core Total (`crates/`)** | **29,362** | **~9,600–11,500** | **~−18,000** | **~1/3 current size, 100% real meta-engine.** |
| **`stdlib/`** | ~5,500 | **Uncapped** | **Grows cleanly** | Reorganized into `stdlib/{render,engines,shapes,sdf,geometry,materials,lighting,sky,post,motion,noise,math,physics}/` — all engines & recipes readable, modular, forkable. |

---

## 2. Exact Cut List (What to Delete & Relocate)

### Immediate Pure-Slop Deletions (~5,000 LOC, zero architectural risk)
| File / Symbol | LOC | Disposition |
|---|---:|---|
| `crates/loft-parse/src/cst.rs` | 352 | **Delete.** Parser flattens CST to `Vec<Lex>` immediately and discards it. Replace with a ~40-line flat token slice for `loft context` / `loft lint`. |
| `crates/loft-passes/src/fusion.rs` | 201 | **Delete.** Doesn't fuse anything — inline the ~30 lines of motion bundle wrapping into `motion.rs`/`bundle.rs`. |
| `crates/loft-passes/src/sexp.rs` | 122 | **Delete.** Unused s-expression round-tripper. |
| `crates/loft-eval/src/refine.rs` | 310 | **Delete.** Swiss-cheese divisor checker bypassed by `.x` and loops; replace with sound interval check where needed. |
| `crates/loft-check/src/kernel_units.rs` | 97 | **Delete.** Fake unit checker (only runs in `kernel`s, bypassable via any `fn`). Revisit properly only if units become real everywhere. |
| `crates/loft-wgpu/src/web.rs` | 289 | **Delete.** Broken partial JS metadata dumper; `wgpu` already targets WebGPU directly. |
| `crates/loft-native/src/edit.rs` | 142 | **Delete.** Half-baked Tier-0 hot edit that still runs full `loft_passes::compile()` and refuses motion/bakes/path. |
| `crates/loft-native/src/shadow.rs` + `motion.rs` | 391 | **Delete / fold.** Shadow reuse belongs in schedule `on dirty(Casters)`; deduplicate `motion_layout`/`scalar_writes` into ~60 lines. |
| `crates/loft-abi/src/lib.rs` (creature structs) | ~150 | **Delete from `loft-abi`.** `BodyRow`, `ChainRow`, `SegmentRow`, `CreatureHeader`, `FootPin`, `ManipState`, `TrailNode` are game/stdlib structs, NOT core ABI! |
| `crates/loft-ir/src/bundle.rs` (`Op`/`IrModule`/`PassRegistry`) | ~20 | **Delete.** Unused stubs. |
| Dead AST/checker/builtins variants | ~180 | **Delete:** `Item::Unit`, `KindDecl.fns`, `Stmt::Assign.op`, `TyExpr::Generic`, `NumSuffix::Deg`, `WgslLowering::{Prefix,Infix}`, duplicate `MotionMath` in `references.rs`, duplicate `scan` check in `check.rs:359-366`, dead header in `world_medium.wgsl:1-16`, stub `loft-cli/src/fmt.rs`. |
| `crates/loft-runner/src/tests.rs` | 372 | **Move** to `crates/loft-runner/tests/`. |
| `crates/loft-cli/examples/*` + `crates/loft-runner/examples/*` | 864 | **Move** to `tools/loft-verify/examples/`. |
| `crates/loft-std/src/compute_twin.rs` + `stepping.rs` | 1,088 | **Move** to `tools/loft-verify/` (verification twin, not core runtime!). |
| `crates/loft-passes/src/bvh/twin.rs` + `loft-check/src/path_twin.rs` | 426 | **Move** to `tools/loft-verify/` (CPU verification twins, not compiler frontend!). |
| `crates/loft-cli/src/diff.rs` | 256 | **Move** to `tools/loft-verify/` (gate comparator). |
| `crates/loft-gltf/` | 372 | **Move** to `extras/loft-gltf/` or feature-gate out of core compiler. |

### Structural Meta-Engine Deletions (Phase 2, ~8,000–10,000 LOC)
| Subsystem | LOC Saved | How It Collapses |
|---|---:|---|
| **Runtime collapse:** merge `loft-std` + `loft-wgpu` + `loft-runner` + `loft-native` into ONE lean `loft-runner` | **~3,500** | Delete cross-crate forwarding wrappers, duplicate `Clock`, `loft-native` Phase-1.5 host layer, and string `.replace()` WGSL hacks in `initialize.rs` / `mips.rs`. |
| **Hardcoded renderers in `loft-emit` + `loft-passes` + `loft-check`:** `defaults.rs`, `shade_schedule.rs`, `emit/shade_tiles.rs`, `emit/paths.rs`, `material_shape.rs`, `check/bsdf.rs`, `spectral_path.wgsl`, `world_path.wgsl`, `world_medium.wgsl`, `scope.rs` 24-file whitelist | **~5,500** | Once `target` declarations + generic `FramePass` (M2) land, every stock renderer (`realtime`, `woodblock`, `reference`, `spectral_caustic`) becomes an ordinary `.loft` + `.wgsl` recipe in `stdlib/render/` with identical consumer schedule verb signatures! Delete all `.replace()` WGSL string surgery and literal scene/lobe/style name branches in Rust! |

---

## 3. Execution Plan: Code-Only Refactor Waves (`LOFT_NO_COMPILE=1`)

### Base Setup (Before Launching Code-Only Lanes)
1. Create clean integration branch **`rework/meta-10k`** off `main` (`f858d61f`).
2. Merge **`loft/integrate-1125`** (`a66e04b4` — 0 file conflicts!) + audited Wave 1 commits (`rework/2026-11-27-soundness` `e9629283`, `rework/2026-11-27-gates-and-grammar` `3ae80a14`, `rework/2026-11-27-meta-passgraph` `8fac0434`, plus ONLY the ~25-line `Inputs` dirty fix from `r27-perf`).
3. Prune the 4 dead prunable worktrees + 7 orphaned `.muse/worktrees/subagent-v2-*` worktrees.

### Code-Only Wave A — Slash & Reorganize (4 Parallel `gpt-6.2-sol` `LOFT_NO_COMPILE=1` Lanes)
Strictly disjoint directories/crates, **zero `cargo`/`rustc` calls per worker**:
1. **`c1-cut-frontend`**: Delete `cst.rs`, `refine.rs`, `kernel_units.rs`, dead AST/checker/builtins/IR items, `loft-abi` creature structs, `scope.rs` 24-file hardcoded whitelist (replace with real filesystem module resolution under `stdlib/`), move `check/path_twin.rs` + `bsdf.rs` out of `loft-check`, move inline `#[cfg(test)]` blocks out of `src/` into `tests/`.
2. **`c2-collapse-runtime`**: Delete `loft-wgpu/src/web.rs`, `loft-native/src/edit.rs` & `shadow.rs`, move `compute_twin.rs` + `stepping.rs` + `loft-runner/src/tests.rs` + `examples/` out of core runtime, wire `Execution` v3 overrides cleanly in `loft-runner`, collapse thin forwarding wrappers between `loft-std`/`loft-wgpu`/`loft-runner`/`loft-native`.
3. **`c3-cut-passes-and-cli`**: Delete `fusion.rs`, `sexp.rs`, `loft-cli/src/fmt.rs`, move `bvh/twin.rs` + `loft-cli/examples/` + `loft-cli/src/diff.rs` to `tools/loft-verify/`, harden `authored_compute.rs` `append_to` (filter `DESCRIPTOR`, remap `DispatchExtent`, merge `resets`).
4. **`c4-stdlib-reorg`**: Reorganize `stdlib/` cleanly (uncapped, composable): add `stdlib/shapes/{capsule,bead}.loft`, add `ellipsoid` & `smax` to `stdlib/sdf/solid.loft`, deduplicate `stdlib/motion/{gait,legs}.loft` and `scenes/showcase/std1e/gait.loft`, move `act.loft`/`roam.loft` cleanly (with compatibility re-exports so zero scenes break!), clean up dead imports & `stdlib/README.md`.

### Code-Only Wave B — True Meta-Engine + Token-Bloat Killers (3 Parallel `gpt-6.2-sol` `LOFT_NO_COMPILE=1` Lanes)
1. **`m2-targets-and-passgraph`**: Implement `target Name: Format Extent [msaa N]`, generalize `FramePass` to bind ANY declared `target`/`table` and render to ANY `target`, remove hardcoded `FRAME_STORES` / 9-name `FrameView` ceiling.
2. **`m3-demote-engines`**: Move `orbit`, `find_shadow_map`, `shade_tiles`, `outline_and_paper`, `screen_pass`, `woodblock(...)`, `spectral_path.wgsl`, `world_path.wgsl`, `world_medium.wgsl` out of `loft-emit`/`loft-passes`/`loft-check` into `stdlib/render/` with identical consumer signatures; delete WGSL `.replace(...)` string surgery and hardcoded scene/style/lobe names from Rust.
3. **`l1-token-ergonomics`**: Implement strictly additive token-bloat killers from `ROLLOUT_TOKEN_BLOAT.md`: `v3(...)@Space` / `p3(...)@Space`, `match`, `if` over `emit` in `build`, `struct` returns + small fixed arrays `[f; N]`, `with_normal` lobe combinator, and fix imported parameterized `schedule` functions (`E505 duplicate cull`).

### Single Final Integration & Compile Pass
Assemble Wave A + Wave B onto `rework/meta-10k`, run ONE cohesive compile-and-fix sweep (`tools/dev.sh check --workspace`), run `tools/lines.py` (target **~10k–12k Rust Core LOC**), `tools/check.sh static`, `out/spark_pairs/` (111/111), and one locked `tools/check.sh full` GPU gate!
