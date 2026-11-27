# Working on Loft

Read [PHILOSOPHY.md](docs/PHILOSOPHY.md) (especially §6 Architectural Red Lines) and [CORE.md](CORE.md)
before touching Rust. Authors start with [AGENT_CARD.md](AGENT_CARD.md). Language and ABI
decisions live in [the v1 specification](docs/spec/README.md). Current rework plan:
[BLUEPRINT.md](agent_notes/2026-11-27_review_and_rework/BLUEPRINT.md).

## Architectural red lines (read before every change)

- **Keep the compiler (~5.5k target) + ONE generic `wgpu` runner (~1.8k target); delete baked engines.**
  Total Rust core (`crates/`) target is **~10k LOC**. **`stdlib/` is uncapped** — grow `stdlib/` cleanly
  as composable, forkable `.loft`/`.wgsl` modules instead of adding Rust special cases.
- **NEVER** hardcode renderers (`Vis64`, shadow, tiled shade, woodblock, path tracer, caustics),
  scene/library/creature names (`CreatureHeader`, `"sun_sky"`, `"woodblock_pigment"`, `"spectral_dielectric"`),
  or WGSL string `.replace(...)` surgery into `crates/`.
- **NEVER** weaken `@Space`, module privacy, Lipschitz/interval soundness, determinism checks, or gate
  contention handling to make a benchmark or rollout look green.
- **Keep existing consumer rollouts backward-compatible** (`out/spark_pairs/`, `corpus/`, `scenes/`,
  `engines/gates.json`): demote hardcoded verbs to `stdlib/` functions with identical signatures; make
  new language ergonomics strictly additive.
- Put tests in `tests/` and verification tools/examples in `tools/loft-verify/`, not inline in `crates/*/src/`.

## Build and check

- **Write all code first, compile rarely (MANDATORY):** Compilation is the scarce resource on this machine. Read all relevant files, types, traits, and call sites upfront, design the complete change, and **write ALL code across all files in one big coherent pass BEFORE running your first compile.** Never iterate edit-one-file-then-`cargo check`. Aim for **2–3 total compiles per lane**: (1) one `CARGO_BUILD_JOBS=2 tools/dev.sh check <crate>` after all edits are written (fix every reported error before compiling again), (2) one focused test run, (3) one final verification/release build only when everything is ready.
- **Code-only refactor mode (`LOFT_NO_COMPILE=1`):** During large multi-crate refactors, lanes may run with zero `cargo`/`rustc`/`tools/dev.sh` invocations — read all types/imports/call sites carefully, write 100% compiler-ready code in one pass, commit cleanly, and let the single downstream integration pass compile once.
- Work from the root Cargo workspace. Use `tools/dev.sh check|lint|test|build|run CRATE`.
  `--workspace` selects every member. `--fast` selects the optimized lab profile.
  The wrapper uses the pinned toolchain, checkout-local Cargo outputs and the shared compile lock.
- Run `tools/check.sh static` for corpus provenance, both grammar recognizers, core vocabulary,
  shell syntax and the host GPU dependency pins when the host lock is available.
  `tools/check.sh plan` shows which crates `quick` will test.
- Serialize local GPU work with `tools/gpu_lock.sh COMMAND`. For example,
  `tools/gpu_lock.sh target/lab/loft profile stdlib/engines/capsules.loft --frames 300`.
  Record the adapter, scene, overrides, frame count, p95 and contention.
- Integrators run `tools/check.sh full` once per combined revision. The CLI writes five-gate
  measurements and per-frame RGBA hashes to `out/meta/gate.json`. Compare hashes to the
  previously scored main. Frozen image approvals are owner decisions.
- Use a Cambium workspace for a lane. Preserve ignored evidence under the primary checkout's
  `out/` before running `git cambium remove --safe NAME` from that checkout.

## Changes

Predict the measurable result before implementation. Keep shared contracts in the workspace
crates and scene-specific behavior in authored libraries. Run focused checks on the edited
crate. Keep the Phase 5 source archive in `examples/` read-only.
