# Loft v1.0 specification

Written by the research scientist on 19 Nov 2026 (bundle: `docs/handoff/2026-11-20_syntax/research_scientist_spec_bundle.md`). This folder is the source of truth for v1.0; `LOFT_V1_SYNTAX.md` (the implementer's consolidation) must agree with it.

Where the documents overlap, precedence is **[CUTS_2026-11-23](CUTS_2026-11-23.md) > 07 > PATCH_V1_2 > 06_ADDENDUM > 00–05**. The cuts list constructs removed on 23 Nov 2026 and what replaces each.

- `00_OVERVIEW_AND_DECISIONS.md`
- `01_LANGUAGE_V1.md`
- `02_CORE_ABI_AND_LOWERING.md`
- `03_SUB_ENGINES_AND_STDLIB.md`
- `04_ENGINES_AND_SCENES.md`
- `05_NUMBERS_AND_BUILD_ORDER.md`
- `06_ADDENDUM.md`: the scientist's answers to our final request (19 Nov): contradictions settled (layered `+`, owner-approved references, `waive` keyword, normative `loft.lark` with patch), schedule IR, query types and `shade_fn`, full typed `std`, motion table layouts and `body`, `plan`, determinism rules, units, `std::rand` (Philox), the 56 Tier 0 opcodes, the `.loftb` bundle, lossless CST and `loft fmt`, CLI exit codes and the training contract, glTF import.
- [07_PLATFORM_POWER_EXTENSIONS.md](07_PLATFORM_POWER_EXTENSIONS.md) specifies packed storage and `std::quant`, the shared ABI, host interop, live parameters, GPU events and picking, texture baking, diagnostics and replay. Batched multi-world execution is reserved for Phase 5.5 after v1.0.
- [CUTS_2026-11-23.md](CUTS_2026-11-23.md): features removed by the 23 Nov meta-engine review, and what replaces each.
- `PATCH_V1_2.md` (20 Nov): the meta-engine ABI patch. Bind groups are derived per pass (only `@group(0)` is fixed), with expanded HostCaps features, table lifetimes and runtime row arenas, `step_pass_cpu`, `unproved {}`, typed `kernel allow(...)`, `mix_bsdf` and a rebindable `Radiance`.

## Lead rulings

- Use `emit_event(...)` for the outbox. `emit` remains the build-stage row constructor.
- `E501` is used twice (undeclared field write, and the schedule DAG's read-before-write). The DAG error becomes `E505`.
- ~~Det motion `wgsl` blocks use the keyword spelling `wgsl det (...) -> T { ... }`.~~ Removed 23 Nov 2026 ([CUTS_2026-11-23.md](CUTS_2026-11-23.md)): a Det law cannot reach raw WGSL; mark it `nondet`.
- `--max-frame-ms` on a device without timestamp queries (some browsers) exits `6` with the reason, never a guessed time.


## Lead ruling, 20 Nov 2026: exactness is not a goal (owner)

Wherever this spec says bit-identical or bit-exact (for example decision 15, the Twin gate and E504), read it as an **opt-in determinism contract**: a scene or library that needs exact replay or training reproducibility declares it, and only then do E504 and bit-exact twin checks apply. Everywhere else, pictures and the twin are judged by perceptual and statistical tolerances. Stochastic, temporal and race-tolerant GPU techniques (Monte Carlo with temporal reuse, float atomics in physics) are allowed outside the opt-in.

## Lead ruling, 20 Nov 2026: physics tiers

- Kernels and motion laws default to **Det**. Gameplay and rollback rigs use this tier.
  Integer or fixed-point deposits can be Det when each contribution and destination
  is deterministic, every partial sum fits, and results are read in a later pass.
- Write `kernel name(...) nondet` or `law name(...) nondet` for race-tolerant physics.
  A Det consumer of a NonDet table gets E504. Keep NonDet laws on separate row tables.
  Untyped whole WGSL passes carry NonDet provenance for tables they write.
- A NonDet kernel declares its hardware permissions with `allow(...)`. For float
  `atomic_add`, supply `fixed_scale` on the scheduled `compute` call. The linker uses
  float atomics when enabled on the device and signed fixed-point accumulation
  otherwise. Each completed pass publishes f32 values. The scale is a positive
  power of two up to 2²⁴. Keep the sum of the absolute initial value and contributions below 2³¹
  per cell after scaling and rounding each term. Nonfinite or overflowing fixed
  cells publish NaN. The caller checks finiteness.
- NonDet updates run outside the exact CPU twin and exact replay. Check them
  against an independent statistical twin with named seeds, initial states,
  physical error limits and conservation budgets. Without observations, report
  Unknown. A passing fixed-point result leaves the NonDet tier unchanged.
