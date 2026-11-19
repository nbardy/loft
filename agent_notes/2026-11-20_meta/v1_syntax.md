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

