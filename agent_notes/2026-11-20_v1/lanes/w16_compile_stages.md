# W16 compile stages

- Prediction. Identical packed data, emitted WGSL, pass order, and diagnostic order.
- `lib.rs` fell from 1,521 to 477 physical lines. The longest function across it and the pipeline is `stage0::derive_kind` at 102 lines. The relocated `AuthoredCompute::lower` is 111 lines.
- `compile_with` accepts `CompileOptions`. The six existing entrypoints forward with their original defaults.
- The driver checks and resolves the schedule, evaluates rows and bounds, lowers and validates shaders, then assembles the outputs.
- The direct Naga dependency remains. `loft-format` is not a `loft-passes` dependency, and moving the validator alone would leave the following Naga contracts for the integration lane.
  - `shader::naga_module` parses and validates generated WGSL. `provenance::Map::{origin, expressions}` consumes Naga spans and IR, and `program::ShaderModule::naga` stores serializable IR.
  - `authored_compute::{table_layout, link_raw_compute}` inspect IR for storage layout and raw bindings. `AuthoredCompute::lower` inspects generated texture bindings.
  - `renderer_bundle::{shader, replacement, lower_program, lower_twin}` and other bundle paths inspect, modify, or emit IR. `sexp` and `telemetry` also inspect or emit it.
  - `bake_tex::Bake::format` carries `naga::StorageFormat`. The `plan`, `sexp`, and `telemetry` tests use Naga directly.
- Rust syntax and whitespace checked. Rust compilation, tests, and output parity are deferred to integration.
