# w16_emit_wgsl

- Rust LOC in `crates/loft-emit` fell from 3,130 to 2,581. The crate-wide target was 850.
- `emit.rs` fell from 1,705 to 1,208 LOC. Including existing WGSL and the manifest, the crate fell from 3,766 to 3,191 LOC.
- `stdlib/core` gained 614 lines of WGSL. The emitter uses the shared builtin catalog for direct lowering and fixed motion math.
- Static checks resolved all 33 template includes and compared representative expansions byte for byte against the baseline.
- Rust compilation, tests, and the workspace lockfile refresh are deferred to integration.
