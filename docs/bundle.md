# The bundle: `scene.loft`

A compiled scene is one file. A host loads it with no compiler, no JavaScript and no Loft source: Keel through
`include_bytes!`, the browser viewer through `fetch`, a training request by path. It replaces the directory artefact
of `artefact_format.md`, which the TS engine writes until the wave 3 cutover of
`proposals/2026-11-19_lean_rust_loft.md` (§1.9, §2.4). The reader and writer are `rust/crates/loft-ir/src/bundle.rs`;
the types below are its types, and that file wins where this one disagrees.

## Layout

All integers are little-endian.

| Bytes | Content |
|---|---|
| 0..4 | magic `LOFT` |
| 4..8 | `u32` format: `1` (`Format::V1`), the only one |
| 8..16 | `u64` length `n` of the JSON |
| 16..16+n | the serde JSON of `Bundle` |
| then, per section | zeros to the next multiple of 64 bytes, then the section's bytes |

Nothing follows the last section. The JSON gives each section's length (`sections: [len, ...]`); the bytes live only
in the binary part. Every float in the JSON is its f32 bits (`F32(u32)`), so the same scene writes the same bytes on
every platform.

## What the JSON holds

Every id is an index into one of the bundle's own lists (`names`, `resources`, `kernels`, `modules`, `media`, ...) or
into `sections`.

| Field | What it is |
|---|---|
| `format`, `compiler` | `V1`, and the Loft commit that wrote the file |
| `names` | every name a host may ask for: draws, cameras, params, inputs, entries |
| `resources` | `{ shape, init }`: `Rows { rows, lanes }`, `Texture`, `ViewSized`, or the per-frame uniform `Frame`; filled with `Zero`, a section's `Bytes` (filled at compile), or `By` a kernel |
| `kernels` | `{ writes, reads, body, when }`: a `Map`, `Scan` or author `Raw` body, run at `Load`, every `Frame`, or as `Steps { dt, from, count, keep }` (`Keep::Latest` or `Every(k)` history) |
| `modules` | `{ wgsl, origin, precision_alias, reads }`: the section of WGSL text, `Library` or `Author`, whether it uses the `lf` alias, and its `@group(1)` binding order |
| `draws` | `{ name, layer, cover, shade, raster }`: `Subject` or `Surroundings`; covered by `Raster` (grid, mesh, quads), `Ray` (a proxy and a hit or medium) or `Screen` |
| `passes` | `{ target, camera, draws, clear, compose }`: to the view, a texture or a shadow map, with outline and ground composed once |
| `media` | `{ transmittance, integrate }`: `March` or `Gaussians`; every veiled thing calls the one `transmittance` |
| `cameras`, `params`, `inputs` | named camera roots; params with default, min and max; inputs with type and default |
| `sites` | file, line and column of the author's code, for refusals mapped back from WGSL |
| `sections` | lengths of the binary sections |

## Precision

A module with `precision_alias` is written once with `lf` for its low-precision floats. At load the host prepends
`alias lf = f32;`, or `enable f16; alias lf = f16;` when mixed precision is asked for and the device has `SHADER_F16`.
Storage stays f32. What ran is recorded in the frame's identity, not what was asked.

## Identity

`Identity { build, frame, device }`, in `loft-ir/src/identity.rs`:

- `build` is the sha256 of the file. Variant and style change the file, so they need no field of their own.
- `frame` is the camera, the time's f32 bits, the size, each param's and input's f32 bits, and the precision that ran.
- `device` is the backend, adapter and driver.

The same identity gives the same frame bytes on that device.

## Refusals

A host refuses a file it cannot load with `Error::Load` (exit 3), by name: `NotBundle` (no magic),
`FormatMismatch { found, expected, compiler }`, `Truncated { need, have }`, `Trailing { end, have }`, `Json`, and
`Invalid` for a bundle that decodes but fails `loft_gpu::validate` (an id out of range, a size over the device's
limits, a library module Naga refuses) before any GPU object exists. There is no reader for older formats: an
incompatible change adds `V2` and deletes `V1` in the same commit unless a persisted consumer exists.
