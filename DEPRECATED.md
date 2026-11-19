# Deprecated trees (19 Nov 2026)

Loft v1.0 is the meta engine in `crates/` (runner, IR, passes, parse, CLI), `lib/` and `engines/`, written in the
`.loft` v1.0 surface (`docs/spec/`). The trees below are deprecated: nothing new is built on them, they are kept
until Phase 5 of `docs/spec/05_NUMBERS_AND_BUILD_ORDER.md` regenerates the corpus and re-pins training, and then they
are deleted. The last state of the old product is the tag `legacy/2026-11-19`.

| Tree | What it is | Fate |
|---|---|---|
| `src/` (TypeScript, ~50k lines) | Old authoring API, TS compiler, Dawn host, library, styles, motion (`gait`, `stance`), solve | Deleted in Phase 5. Ideas carried into the spec; no code ported. Motion pieces retire in Phase 3, `src/soft` and `src/style` in Phase 4. |
| `rust/crates/loft-loader` (`loft-view`) | Old Rust runtime for TS artefacts | Deleted in Phase 5. Its measured pose batching and shader sharing are the reference for the new lowering. |
| `rust/crates/loft-core`, `loft-ir`, `loft-lower`, `loft-node` | The abandoned lean Rust port (format, kernels, emit, checks, solve, Node addon) | Deleted in Phase 5. Mine for reference only (naga emit, interval arithmetic, solve). |
| `rust/crates/loft-gpu`, `loft-draw` | Old wgpu host and draw executor (Bundle V1, Keel instances, pin checks) | Deleted in Phase 5. The Keel embedding (drawing on a host-owned device, wgpu/naga pin check) is re-done in the new runner (`Runner::attach` / `encode_into`, `docs/spec/02`), not ported as a library. |
| `tools/parity`, `tools/port`, `tools/rust_compare.ts` | Old parity and port tooling | Replaced by the new five gates. |
| `examples/` (125 TS scenes), creative seeds (170) | The old corpus | Regenerated in `.loft` in Phase 5. |

There are no separate wgpu libraries in v1.0. The only hand-written wgpu code is `crates/loft-runner` (≤1,000 lines);
every GPU program (shapes, materials, light, post, motion laws, engines) is `.loft` source compiled through naga.
