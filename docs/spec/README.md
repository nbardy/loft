# Loft v1.0 specification

Written by the research scientist on 19 Nov 2026 (bundle: `docs/handoff/2026-11-20_syntax/research_scientist_spec_bundle.md`). This folder is the source of truth for v1.0; `LOFT_V1_SYNTAX.md` (the implementer's consolidation) must agree with it.

- `00_OVERVIEW_AND_DECISIONS.md`
- `01_LANGUAGE_V1.md`
- `02_CORE_ABI_AND_LOWERING.md`
- `03_SUB_ENGINES_AND_STDLIB.md`
- `04_ENGINES_AND_SCENES.md`
- `05_NUMBERS_AND_BUILD_ORDER.md`
- `06_ADDENDUM.md`: the scientist's answers to our final request (19 Nov): contradictions settled (layered `+`, owner-approved references, `waive` keyword, normative `loft.lark` with patch), schedule IR, query types and `shade_fn`, full typed `std`, motion table layouts and `body`, `plan`, determinism rules, units, `std::rand` (Philox), the 56 Tier 0 opcodes, the `.loftb` bundle, lossless CST and `loft fmt`, CLI exit codes and the training contract, glTF import. The addendum overrides earlier files where they differ.

Lead's notes on the addendum (decided without the scientist; small):
- `E501` is used twice (undeclared field write, and the schedule DAG's read-before-write). The DAG error becomes `E505`.
- Motion `wgsl` blocks must be marked deterministic; the spelling is `wgsl det (...) -> T { ... }` (a keyword, not an `@` attribute).
- `--max-frame-ms` on a device without timestamp queries (some browsers) exits `6` with the reason, never a guessed time.
- `PATCH_V1_2.md` (20 Nov): the meta-engine ABI patch. Bind groups are derived per pass (only `@group(0)` is fixed), with expanded HostCaps features, table lifetimes and runtime row arenas, `step_pass_cpu`, `unproved {}`, typed `kernel allow(...)`, `mix_bsdf` and a rebindable `Radiance`. Precedence: PATCH_V1_2 > 06_ADDENDUM > 00–05.
