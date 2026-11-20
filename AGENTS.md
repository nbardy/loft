# Working on Loft

Read [CORE.md](CORE.md) for priorities and [OPEN.md](agent_notes/2026-11-20_v1/OPEN.md) for
the measured state of main. Authors start with [AGENT_CARD.md](AGENT_CARD.md). Language and ABI
decisions live in [the v1 specification](docs/spec/README.md).

## Build and check

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
