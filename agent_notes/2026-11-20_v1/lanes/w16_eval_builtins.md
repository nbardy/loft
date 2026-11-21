# w16 eval builtins

- Concrete and interval evaluation share catalog dispatch and width-independent lifting.
- AD uses catalog IDs and preserves authored-call fallback and kink diagnostics.
- Net crate LOC deleted: −23 (206 insertions, 183 deletions). The base already shared the interval evaluator and vector widths.
- Source inventory covered all 79 variants in both dispatchers. `git diff --check` passed.
- Rust compilation, behavioral parity checks and the workspace lockfile update are deferred to integration.
