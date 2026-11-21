# w16_check_builtins

- The checker reads builtin names, arities and signatures from `loft-builtins`.
- Declaration-aware calls and stage/effect checks keep their existing dispatch rules.
- Fixed signatures share argument diagnostics. Packed integer literals and BSDF weights keep their contextual types.
- Net 48 lines deleted in `crates/loft-check` (123 added, 171 removed).
- Source review and `git diff --check` only. The integrator owns the lockfile refresh and Rust verification.
