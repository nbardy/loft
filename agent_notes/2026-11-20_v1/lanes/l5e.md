# L5e · corpus batches 11–15

The manifest assigns 75 pinned crab-actions seeds to L5e. Each uses legacy strand sweeps. The migration samples the legacy CPU twin at the authored mesh ring coordinates and carries each ring's center and two section axes into E+ rows. The original grid has `along × around × 2` triangles per strand, so emitting one grid strip per adjacent ring pair preserves that triangle count. Section exponents and paint are row data. A distinct circumferential grid size needs one kind. Motion samples include the four R0 times and use linear interpolation between samples.

Prediction before building. All 75 seeds should have a source candidate. At the four R0 times, the reconstructed ring vertices should lie within 10 µm of the legacy CPU sweep at the original grid density. The E+ build must derive finite bounds and validate every shader with Naga. The integrator owns R0g and GPU speed. The frozen renderer's standard lighting and sea substitute can change pixel appearance.

## What landed

- `corpus/l5e/*.loft` holds 75 standalone E+ scenes. Replace each slash in the full manifest ID with `__` and append `.loft`. Each file names the source seed and labels the strips by the original part name.
- `corpus/l5e/regenerate.ts` reads the seed set at the pinned Creative commit, checks its SHA-256, and uses Loft's legacy CPU table and surface evaluators to produce the scenes and proof inputs. Run it with Bun from this workspace. It writes work files under `out/meta/p5/l5e/`.
- `corpus/l5e/cpu_twin.rs` is the focused Rust unit test. It checks one interior point on every generated strip against CPU-captured rest data and requires finite AD normals and lit materials. The test links the pre-existing main `target/lab/deps` copies of `loft_passes` and `loft_ir` with pinned Rust 1.99.1. Scene acceptance uses the requested main `target/release/loft`.

## Proof

| Check | Result |
| --- | --- |
| Manifest partition and scene identities | 75/75, exactly batches 11–15 |
| Legacy sweep extraction | 1,740,116 vertex samples over 22,867 strips and seven times. Max reconstruction error 3.731e−12 m before f32 serialization. Sampled paint was constant on every strip and at every sampled time. |
| E+ CPU twin unit test | 22,867 interior queries at rest. Max position error 4.768e−7 m against a 1e−5 m limit. All sampled normals and materials finite. |
| Frozen main `loft build` | 75/75. Every emitted shader passed `naga::valid::Validator`. Scenes emitted 8–13 modules. Final local process wall time was 26–67 ms with the derivation cache warm. |
| Compiler check | `CARGO_BUILD_JOBS=2 tools/dev.sh check loft-passes` passed. |
| Manifest unit test | `bun test corpus/manifest.test.ts --timeout 120000` passed. |
| Decoding grammar | Both Earley and llguidance accepted the reduced grammar probe containing every kind declaration and one emit per kind from train-15/2. |

The prebuilt scene compiler was the freeze binary at SHA-256 `6fd1e7d533140b20911bf2cd2a837c16b4f3c4cda0c846be9db1b8c8357179b2`. During source generation, the shared template hit E405 in the AD helper, E104 on space-tagged row arithmetic and E003 in the material signature. Inlining the axis expression, using untagged numeric columns and supplying the surface UV argument resolved those template errors. No scene produced an E211, E307 or E602 quarantine result. The workspace's `out/meta/p5/quarantine.md` records the status. Build and CPU evidence is in `out/meta/p5/l5e/`.

## Integration

- Use `seed__${file_stem}` to match each generated stem to the R0 entry and frame directory. The geometry carries the legacy camera yaw as a rigid rotation of the scene and sun, so the native camera can use its fixed initial azimuth.
- Motion is linear between 0, 0.25, 0.5, 0.75, 0.9375, 1.37 and 2.9 s. It holds after 2.9 s. The four R0 times land on samples. Intermediate training frames use interpolation. The original continuous motion functions and LOD selection were replaced by sampled motion and the authored grid.
- These candidates use GGX and the frozen realtime engine with a flat water surface. Legacy toon, woodblock and other style programs, outline, and sea shading require picture review. The CPU measurements cover geometry and finite shading values. R0g picture parity, training clip timing and the full decoding grammar corpus gate remain with the integrator.
- The tracked scene sources contain 18,270,714 bytes of row data and declarations. Integrate the lane paths as a group. The ABI, golden vectors, prelude, compiler, CLI, and manifest are untouched.
