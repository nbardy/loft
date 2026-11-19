# LW · Artefact to manifest

Prediction before implementation: 302/302 entries in the scoped lower gate once SW and LIB are connected, with zero differing LW sections. The corpus contains 132 examples and 170 pinned seeds. The harness uses f32, so it should write no mixed overlay.

## Translation

| TypeScript source | Rust destination | Rust lines |
| --- | --- | ---: |
| `src/artefact.ts` types and `tools/port/sections.ts` typed-array encoding | `artefact.rs` | 595 |
| `src/host/lower.ts` artefact checks, format version and limits | `lower/check.rs` | 259 |
| `src/host/lower.ts` setup, buffers, layouts, pose dispatches and manifest assembly | `lower/mod.rs` | 668 |
| `src/host/lower.ts` build, load, frame and step compute | `lower/compute.rs` | 415 |
| `src/host/lower.ts` shader modules, render pipelines and deduplication | `lower/render.rs` | 308 |
| `src/host/lower.ts` view and off-screen passes, depth resolve and clearance | `lower/passes.rs` | 542 |
| `src/host/detail.ts` `gridBounds` and `slotsUsed` | `lower/detail.rs` | 35 |
| `src/host/manifest.ts` descriptors in actual writer key order | `lower/manifest.rs` | 514 |

The compile dump flattens recurrence views. The table accessor in scene WGSL gives the static buffer and vec4 offset, which lets LW place the supplied bake bytes exactly. The four recurrence examples' blobs matched, including checkpointed replay. LW calls SW for sharing, chunk names, JSON and the bundle writer. It takes the prelude, depth resolve and clearance text from LIB.

## Verification

The unfiltered `bun tools/port/diff.ts --mode lower --scope LW --run lw` exited 1. It logged 82 failures for 302 entries, giving **220/302 passing**. The first failure, `examples__serpent_clip`, crashed on the TS side before comparison while reading `scene.style.post`. The shared volume filled during the seed run. Eighty TS entries and one Rust entry then failed while writing. The Rust write failure omitted its first LW section, `files.manifest.inputs`. The full run reported no measured byte differences. The disk also prevented writing `report.json`; the console log is `out/lw/gate.log`.

That run used a local SW adapter forwarding the existing TS share and JSON operations, plus shared text from LIB. Clippy passed with the adapter. After SW published, a separate native check linked LW with SW commit `8db7407` and the LIB text from `2815a7f`. All 161 cached built examples and seeds matched in scope across the focused and cached checks. The cached batch had one disk-write failure on `creep`, whose focused native check matched. The mixed-precision example's full native bundle matched the TS mixed bundle byte for byte.

`serde_json`'s default decimal reader changed detail radii by one ULP in the pool and mixed precision samples. The verification builds enabled `serde_json/float_roundtrip`, which removed those differences. Enabling that feature by default requires an edit to the crate manifest, outside this lane's file list.

## Remaining

Rebase after SW and LIB land on main, enable the exact decimal reader in the normal build, and rerun the gate after disk space is available and the upstream `serpent_clip` crash is resolved. `manifest.authored` can be emitted when the recorder supplies the authored-text registry. Current `compile.json` omits that registry, so these SW-owned manifest bytes may differ.
