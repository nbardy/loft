# Lean Rust wave 2 integration (19 Nov 2026)

Plan: `docs/proposals/2026-11-19_lean_rust_loft.md` §8.2, wave 2. Lane notes: `2026-11-19_lean_rust_lr2{1,2,3,4}.md`.

## Merged

All four lane branches were cut from 7b0cc53 (main at the time), so no main merge was needed. Order: 2.1, 2.2, 2.3,
2.4.

- **2.1 kernels** (`feat/lr21` 94d6af4). No conflicts. The integrator applied the lane's uncommitted registrations:
  the `kernel`/`layout`/`rate`/`schedule` modules, the `loft-ir` path dependency and the lock line.
- **2.2 emit** (`feat/lr22` de794bd). Conflicts in loft-core `Cargo.toml`, `Cargo.lock` and `tests/it/main.rs`; both
  sides were kept. naga's `=30.0.1` pin moved to `[workspace.dependencies]` beside wgpu's, so the two move together.
- **2.3 analyses** (`feat/lr23` 7187650). Conflicts in `Cargo.toml` and `Cargo.lock` (naga and sha2, both kept).
  Modules are sorted. The lane's `check.rs`, `cache.rs` and `tests/it/analyses.rs` failed check.sh's fmt gate and
  were formatted at the merge: 784 → 1,080 scc lines for check + cache, against 1,400 with `solve.rs` still to come.
- **2.4 library** (`feat/lr24` 93d3534). No conflicts. `dump_kernels.ts` and `dump_checks.ts` still matched on the
  deleted `cpu` table program (tsc does not cover `rust/`). They now name `DataTable` and `RecurrenceTable` as opaque
  fills.

## Measured on the merge (M4)

- Lane 2.1, on a fixture regenerated after 2.4:
  - tables: 4,020 ok (4,008 before 2.4), with 13 opaque (19 before);
  - frame dispatches: 215 against TS's 446;
  - Steps: 10/10;
  - const fills: unchanged at 79/160 over 1 ulp, worst relative 9.24e-6.
- Lane 2.2: 5/5 emit tests.
- Lane 2.3: claw Fails at -0.3499924 (sampled Holds); stone Holds at 0.0155646.
- Lane 2.4's touched TS tests, GPU ones included (compile.gpu, soft.render): 37/37.
- **Kill condition (§8.1):** R with graph-function bodies is Owl 21,721, Titan 638,090 and heron_leap 96,480. Build
  only, the counts are 12,243, 402,999 and 81,939. R_Titan is within 0.75M, so the condition does not fire.

## Not done in this wave, and why

- **No cutover and no deletions.** The plan switches `loft check`, `solve()`, `evaluate()` and serve's `--check` to
  Rust only once the lanes' checks pass. None of 2.1–2.3 passes:
  - `solve.rs` does not exist (lane 2.3);
  - real jobs do not decode until the 1.1/1.2 wire is reconciled (`feat/lr-wire`, not merged here);
  - there is no program driver.
  `interval.ts`, `ranged.ts`, the check bodies, `check_state.ts`, `mistakes.ts`, the solve numerics and the second
  `Verdict` stay. `LOFT_ENGINE` and `src/cli/main.ts` routing are unchanged.
- **Lane 2.1 check (4)** is red as written. Its test is `#[ignore]`d in the suite with the reason, and it still runs
  with `--run-ignored all`. The owner decides between `Tol::f32_rel(1e-4)` and a GPU-stage comparison.
- Lane 2.4's 13 measure checks are classified but not rewritten; that rewrite needs 2.3's solve and the driver.
- A GPU render of storm, open and stormfront for §2.2(a) has not been run. They are the sorted-medium tables that
  are now maps.

## Full gate

`tools/check.sh full` on 571c4df:
- static: 9/9;
- cargo nextest, workspace: 20 passed, 1 skipped (the ignored 1-ulp check);
- Bun: 310 passed, 1 failed.

The failure was `tests/bake.gltf.test.ts`, which also fails on `feat/lr24` alone. Lane 2.4's `recorded()` wrapper in
`src/draw/surface.ts` hid `Body` from the bake's `instanceof`, so every swept body morphed instead of being skinned.
5272308 fixes it: the wrapper takes the authored surface as its prototype. Job hashes, print equality and R are
unchanged for owl, Titan and heron_leap. The Bun suite was then re-run: 311/311.
