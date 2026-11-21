# w16_runner_trim

| Crate | Before | After |
|---|---:|---:|
| loft-wgpu | 2,798 | 2,588 |
| loft-runner | 2,178 | 2,155 |
| loft-std | 1,991 | 1,871 |
| loft-format | 740 | 690 |
| Total | 7,707 | 7,304 |

Counts use the scc Rust code filter from tools/lines.py, excluding tests directories.
The 403-line reduction includes relocated tests and falls short of the 500–1,000 target.
Shared WGSL validation preserves caller capabilities and diagnostic formats.
Shared descriptors preserve binding order, labels, pipeline cache and attachment behavior.
Removed the runner's legacy WebGPU aliases and the attachment test's shadow command model.
The transitive Cargo.lock graph and local manifests show zero loft-std wgpu dependencies.
Rust build, test, parity and performance verification are deferred to iv_final.
