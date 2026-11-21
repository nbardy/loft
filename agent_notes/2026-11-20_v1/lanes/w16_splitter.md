# w16_splitter

Split from origin/main into five new workspace crates.

- `loft-check` takes `check.rs`, `source.rs`, `source/scope.rs`, `motion_source.rs`,
  `kernel_units.rs`, `plan_source.rs`, `paths.rs` records, `path_twin.rs`, `infer.rs`,
  `defaults.rs`, `world.rs`, `bsdf.rs` and `bsdf/`. It owns the extracted program
  index, shared records, bit-preserving word codec, reference walker, verdict reason and quant signatures.
- `loft-eval` takes `eval.rs`, `ad.rs`, `lip.rs`, `refine.rs`, `bvh.rs`,
  `import_table.rs`, solver initialization and the CPU side of `quant.rs`.
  Kernel argument folding and tiled World arena packing also run here.
- `loft-emit` takes `emit.rs`, `emit/`, `aov.rs`, `material_shape.rs`, `world/mesh.rs`,
  `bvh.wgsl`, `mesh.wgsl`, `world_path.wgsl`, `world_medium.wgsl` and `spectral_path.wgsl`.
  Path and solver WGSL, quant prelude, bake sampling, outbox WGSL and provenance markers live here.
- `loft-gltf` takes `gltf.rs` as `src/lib.rs` and uses the CPU BVH from `loft-eval`.
- `loft-tier0` takes `tier0.rs`, `tier0_author.rs`, `tier0.wgsl`, `tier0/` and
  `tier0_tests.rs` as `tests/tier0.rs`. Native and CLI link it directly.

`loft-passes` keeps Naga validation, provenance decoding, schedule and bundle orchestration,
the compiled-program BVH twin, and the plan and tiled World compiler tests.
The dependency graph is acyclic. Check, eval, emit and glTF have no Naga or wgpu dependency.
Expected generated WGSL is byte-identical. Source-based derivation cache keys change.
`git diff --check` is the lane check. Rust builds and tests are deferred to integration.
