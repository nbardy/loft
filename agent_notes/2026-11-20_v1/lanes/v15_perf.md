# V15 performance

Base `5e252da` (unified execution). Target adapter Apple M4 Metal at 1280 × 800.
Hermit bakes 12 sky-patch radiances into a dirty cube and fetches them per
shade workgroup. The material quadrature still integrates the same 12 patches.
Shrine groups dense static analytic rows by world-space bounds and checks
each union before the primary per-row frustum tests.
Motion uses a 130-line contraction barrier in deterministic fixed ticks.

| Paired GPU p95, six A/B pairs | Base | Candidate | Paired 95% CI |
| --- | ---: | ---: | ---: |
| Hermit-20 | pending | pending | pending |
| Shrine | pending | pending | pending |
| 1,000-crab dispatch | pending | pending | pending |

CPU/GPU 481-tick comparisons, the WGSL/MSL leak check and both picture
tiers await the queued optimized lab verification. The earlier release
test build was stopped after a linked test exposed an arithmetic-lowering
parse error. The compiler now lowers after source provenance mapping.
