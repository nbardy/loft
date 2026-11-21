# W16 gates

- Moved controls, the five-gate suite, trace replay, and the particle FLIP benchmark to `loft-verify`.
- CLI compatibility forwards gate, controls, trace flags, and the profile Speed gate to the sibling executable.
- Both binaries share authored verdict decoding, PNG output, and bundle lowering. Still rendering stays in the CLI.
- Full checks use the verifier for candidate scoring and timing while retaining support for older baseline CLIs.
- Expected at integration: identical gate verdicts and RGBA hashes, with roughly 4,200 physical lines outside the core scan.
- Rust builds and GPU runs are deferred to the integrator per the zero-compile directive.
