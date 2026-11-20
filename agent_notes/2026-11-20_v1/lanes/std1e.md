# Std1e · Deck-local gait laws

Base `82d9019e2f2ae1510c09a6d6e32af153a3a6a6ad` descends from scored Std1a merge `f8fc6121cf84e8b11ddb4d3162f1b0367a4c03a7`. Scored implementation `d6b0013745f6316dc160c3771e3cc84fb2ffeea9`. Decoder fix `60bee88d7ee919ca5a860111c9f2816fca124817` changes comments, source spans and verdict stamps. [Program comparison](../../../out/meta/std1e/grammar-program-equivalence.log)

Four two-link chains share a typed deck, bounded rest plans and one fused Walker dispatch. Rosters cache their hull until lift or land. Load, strain, dwell and the remaining CoP margin govern exchange. The fixture preserves area with at most one swing. Roam drives the gaze handoff and Act drives the ferry signal. Out-of-range rest plans raise E602. [Rest-plan diagnostic](../../../out/meta/std1e/unreachable.log)

The only new public function, stage-0 `walker`, replaces a 476-token block with 91/80 tokens including imports at two independent call sites (5.23×/5.95×). The draw and planted check read the same solved toe. [Export counts](../../../out/meta/std1e/exports.txt)

At 120 ticks/s over 480 ticks, Tidal/Crossing finished 10/8 steps with at least three planted toes. Preflight → final drift was 976.3022 → 0 mm / 356.58817 → 0 mm. Crossing latched at tick 90, released at 247 and relatched at 262. Its maximum airborne reach overshoot was 0.0581 mm after storing pose. Logical pose is 68 B per actor and the packed row is 392 B. [Prediction](../../../out/meta/std1e/prediction.md) · [Trajectories and constraints](../../../out/meta/std1e/constraints.json)

CPU and Apple M4 Metal matched every sampled row and verdict word at 30/60/120 Hz with and without a six-tick stall. Walker groups were 120/240/480 and 120/238/475. Crossing also submits one Signal dispatch per group. Both native logs replayed to the same state and RGBA. [Tidal trace](../../../out/meta/std1e/tidal.trace) · [Crossing trace](../../../out/meta/std1e/crossing.trace) · [Replay RGBA](../../../out/meta/std1e/rgba.json)

The crossing check held flight, point, segment and area projections and both share fits. `share_range` and `seams` stay tagged comparison sources. [Legacy share comparison](../../../out/meta/std1e/tagged_support.json) · [Reference index](../../../stdlib/REFERENCE.md)

| Tidal actors | Steps | Dispatches | Drift (mm) | Motion p95 (ms) | Legacy p95 (ms) |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 10 | 480 | 0 | 0.091334 | 0.070 |
| 256 | 2,560 | 480 | 0 | 0.114875 | 0.101 |
| 1,000 | 10,000 | 480 | 0 | 0.108667 | 0.086 |

The legacy eight-foot plan took 22 steps per actor with 0.0008 mm drift. The lane used 30 warmups and 450 timed ticks per count. At 1,000, the Walker pass first returned zero timestamps at tick 1. The retry cloned the motion seed across 1,000 rows and resolved timestamps after submission. Triangle p50 ranged from 0.575 to 1.717 ms, leaving timing regression unresolved. [Timing](../../../out/meta/std1e/timing.log) · [1,000-row timing](../../../out/meta/std1e/timing-1000-separated.log)

The `self::gait` fork raises the authored lift from 0.115 to 0.185 m. Peak toe height rises from 0.1317 to 0.2119 m. Only lift and toe-Y words differ across all 481 states. The posed lift frame changes 6,736 pixels. Crab, Heron and Boss match in serialized programs, native replay state and the 0.5 s RGBA controls. [RGBA](../../../out/meta/std1e/rgba.json) · [Fork source](../../../out/meta/std1e/fork-source.diff)

Review stills are WebP q95 from posed snapshots. Camera, time, command, source SHA, adapter and lossless stance/lift/land controls are recorded alongside them. [PBR](../../../out/meta/std1e/approval/pbr.webp) · [Woodblock](../../../out/meta/std1e/approval/woodblock.webp)

CLI check and six focused tests passed. Static shell, pins and corpus checks passed. The decoder score combines the unchanged 283-file base inventory with 6/6 new inputs. The workspace suite, integration and picture approval belong to the lead. [Static](../../../out/meta/std1e/static.log) · [Decoder coverage](../../../out/meta/std1e/decoder-coverage.log)
