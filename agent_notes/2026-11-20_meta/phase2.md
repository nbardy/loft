# Phase 2 • Interactive Edit

Base bca209d, Apple M4 Metal. Main already ships the frozen 56-op encoding, a 32-register GPU interpreter and CPU oracle, a single dirty-kind route, an eight-entry promotion LRU, and E+ origins in the text and JSON forms of show. The legacy TS WGSL string parser has already been deleted. The row forecasts +150 Runner and +1,800 Passes lines from Phase 1.5. Report the incremental delta against this newer base.

## Predictions before building

| Observation | Prediction | Pass condition |
| --- | ---: | ---: |
| Authored source received through final queue write, radius / structural box / bounded-loop / clipped sphere at 1280×800 | 0.65 / 0.70 / 0.75 / 0.95 ms | each ≤2 ms |
| Metal pipeline creates during each edit | 0 | 0 (0 ms foreground compile) |
| Alternating paired whole-frame GPU p95 increase, same four edits, 120 pairs each | +0.18 / +0.22 / +0.18 / +0.35 ms | each ≤+0.50 ms |
| Tier 0 vs promoted RGBA, each edit | 0 of 1,024,000 px over 1/255 | at most 1,024 pixels |
| Trace twin at 2,197 points | 0 maximum error | ≤1e−5 |
| Embedded 140,000-invocation bucket | 0.025 ms upload, +0.25 ms paired p95, 0 pixels over 1/255 | same time and image caps |
| 400 ms quiet period, superseded generation, LRU reuse and eviction | no early or stale install, reuse a resident pipeline, evict the least recently used at entry nine | observe each transition |
| Declared E101–E503 diagnostics and show | all 20 snapshots match, kind WGSL maps to edited E+ expressions | exact diagnostics with author spans and fixes |
| Full native workspace | all tests pass | run once after focused tests |

The compute benchmark executes 4,375 uniform workgroups of 32. A uniform register mask on short tapes can reduce per-lane storage from 512 to 64 bytes without changing the wire ISA. Reuse the measured C1b implementation. On the authored path a primitive or CSG subtree selected outside the march avoids paying the full switch for each field visit. The clipped sphere exercises composition beyond a literal primitive. Source checking, conservative bounds and culling updates are included in the edit clock. The lossless frame comparisons precede conversion of evidence to WebP q95.

Use the existing public still --edit and runner entry points under the GPU lock. Compare against a fresh compile as well as the promoted output. Keep failures in the score. A pass on the 140k compute case alone cannot stand in for an authored edit. The full-screen 63-word stress case is a separate observation of how the cost grows with dirty coverage. Its previous >78 ms cost makes it a poor model for the single dirty bucket forecast. Predict it exceeds the two timing budgets if its expression remains interpreted at every march sample.
