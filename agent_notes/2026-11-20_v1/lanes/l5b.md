# L5b · Host embedding

## Prediction

A host frame with one compute pass and one render bundle records one upload to the runner's uniform, one compute dispatch and one render pass. The CPU proof should report zero caller clears, zero readback commands and zero submissions. Both depth conventions should pass the near sample, reject the far sample and reject equal depth against the same host depth. Depth-testing-only draws should leave host depth intact. Shader entry points used by the proof should pass Naga validation.

The existing ABI supplies one caller color view and optional depth. This lane uses the existing indirect table and schedule, and takes the comparison direction from `HostCaps`.

## Build and proof

`Runner::attach` retains the caller's device and queue. `encode_into` prepares compute and render passes together, then records them in bundle order on the supplied encoder. Writable attachments use `Load`/`Store`. Read-only depth and stencil use wgpu's read-only attachment operations. Depth pipelines use strict `Less` for forward Z and strict `Greater` for reverse Z.

Both commands passed on the first run.

- `CARGO_BUILD_JOBS=2 tools/dev.sh check loft-runner`
- `CARGO_BUILD_JOBS=2 tools/dev.sh test loft-runner -E 'test(embed::tests::)'` (4 passed)

The tested production command iterator yielded one uniform upload, one compute pass and one render pass, with zero readback or submission commands. The CPU attachment twin consumed the same operations as the render descriptor. It found zero clears and preserved the host color on occluded pixels and host depth on read-only passes, with 1 and 4 samples in both directions. Naga validated the test WGSL and reflected a shared read-only binding in both raster stages. The tests created no device or pipelines and ran no GPU work.

## Files

- `crates/loft-runner/src/embed.rs` implements host attachment validation, render-bundle caching and CPU proofs.
- `crates/loft-runner/src/encode.rs` links render and compute passes against the existing bind-group ABI and records the command sequence.
- `crates/loft-runner/src/lib.rs` passes the host views and caps to the existing encoder entry point.

## Integration

Load a `loft_ir::Bundle` with a host-facing `RenderBundle` after attaching to the Keel or room-runner device. Use one logical color ID and one optional depth ID throughout the host pass sequence. The views must use their base formats and matching 1× or 4× sample counts. The caller initializes attachments, supplies a bundle whose vertex or fragment depth matches its projection, resolves multisampled color if needed, and submits. Geometry can use vertex pulling from storage and optional U16 or U32 index buffers. The pass ABI has no vertex attribute layout, so declarations requesting vertex input buffers return a typed refusal.

The render cache re-records on replacement of bound resources, index buffers or indirect argument buffers. A changed attachment format or depth convention rebuilds the pipelines. View swaps with the same formats use the current views without rebuilding. The serialized integrator owns live device validation and the downstream consumer smoke test. The runner's inherited group 3 restriction still applies. The runner was already 1,003 lines excluding its top-level tests on the freeze commit and grows with the render path. Resolving the overall layer budget requires work across lane ownership.

The branch changes no frozen ABI, golden vectors, dependency or CLI hub entries.
