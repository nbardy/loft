# Phase 1.5 — E+ parser swap

Base 389da80 contains the syntax consolidation, proposed IR changes and the parser manifest. Its parser source directory is empty. The scientist's addendum and the three lead rulings govern the implementation.

## Predictions before implementation

| Instrument | Prediction |
|---|---|
| Phase 1 pictures, original approved RGBA hashes, three orbits × six cases | 18/18 |
| CPU/GPU twin, 60° off peak, material tolerance 2e-5 from ABI §5 | 36/36 |
| Frame p95 and recorded commands, original budgets | 12/12 on a quiet M4; 5 commands for blobs/capsules, 17 for hermit |
| Load, including identical module bytes at 1 and 20 | 33/33; hermit 8 modules, 13 pipelines, 11 distinct shapes at either count |
| Truth | 34/34 |
| Hermit warm front end + device load, cache primed | 45 ms at 1, 55 ms at 20 (budget 60 ms) |
| Hermit whole-file o200k_base tokens against the Phase 1 source | −36% (budget ≥35%); same tokenizer and treatment of comments for both |
| Normative Lark acceptance | 100% of v1 library, engine and bakeoff files |
| Lossless CST and semantic IR codec | byte-identical CST replay, canonical encode/decode/encode equality |
| Spec-only tasks, first successful compiler invocation | crab 3, reeds 2, heron 3 (cap 5); all 3 compile |
| Independent geometry checks | two distinct rays from reflected origins converge on the target; opposite signed turns on mirrored fish input |

The cache removes interval subdivision from the warm path. A placement changes the world bounds but preserves local derivations. A change to a called geometry helper, scan table, or a shape column must invalidate the corresponding derivation. The port must preserve arithmetic order, orbit parameters, row order, and layer order for exact reference hashes. Speed is measured on the device and all reference approvals remain those from Phase 1.

## Compiler feedback log

The first drafts are in commit a411e96. Each attempt below was a fresh invocation of the real Rust compiler with the shared library and Naga validation. Lark acceptance does not count as a compile.

| Attempt | Crab | Reeds | Heron |
|---|---|---|---|
| 1 | E001 at 28:25, a unit consumed the start of the next function name | E001 at 17:33, conditional expression in an argument | E001 at 4:19, parameter refinement |
| 2 | E405 at 16:13, AD through the named coil scan | E011 at 26:8, undeclared Sky | E003 at 7:10, angle parameter inference |
| 3 | Compiled and Naga-validated; 1 surface, 2 claws, 12 leg rows, 1 eye row, 1 sand row, 8 modules | Compiled and Naga-validated; 200 stalks plus water, 5 modules | E520 at 52:3, unlowered run_laws schedule entry |

Between attempts 2 and 3 the compiler acquired direct scan sampling, frame field names, contextual vectors and environment rows. The crab uses one lobe layout on both pupil branches and the prelude's fine value noise. The reed draft supplies its hash helper. The heron explicitly types its bearing, angle, eye and fish helpers.
