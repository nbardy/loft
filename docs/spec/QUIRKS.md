# Metal, Naga and wgpu

| Site | Contract |
| --- | --- |
| Normalized 16-bit pairs | Pack and unpack each halfword explicitly. Metal miscompiles `pack2x16unorm` and `unpack2x16unorm`. |
| Row fields | Mangle reserved names before WGSL emission, including `target`, `ref`, `mod`, `in` and `out`. Use the same name at every reference. |
| CPU twin | Preserve all 32 bits on i32 to u32 reinterpretation. |
| Pipeline creation | Metal lacks `PIPELINE_CACHE`. Keep shader text and pipeline creation order deterministic for driver cache reuse. |
| Shader emit | Iterate a `BTreeMap`, an insertion-ordered `IndexMap`, or a sorted sequence. Never derive emission order from `HashMap` iteration. |
| Shade loops | Use one call site per field or BSDF. Duplicated sites multiply inlining and cold compile time. |
| Bindless textures | Keep `binding_array` indexing at the texture operation. Passing an indexed texture directly as a WGSL function argument can crash Naga's SPIR-V backend (wgpu #6283 and #8520). |
