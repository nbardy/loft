# Loft v1.5 closeout · 21 November 2026

The release gate rejected v15-perf on Hermit resource counts. Four implementation branches remain held. The reduced tree at `675040e` includes particle transfers, the FLIP showcase, binary corpus tables, lint and the x2c investigation. The execution refactor and Rust picture scorer were already on main. Release certification remains open.

## Branch decisions

| Branch | Result |
|---|---|
| v15-exec | Already merged at `5e252da`, including v15a/b/c. The incoming release held 19/19 pictures and 36/36 scene twin checks. |
| v15-perf | Held. The release added one module and one pipeline to both Hermit fixtures. The six-pair lane measured Hermit-20 at 3.569 ms p95 versus 3.666 ms, delta CI [−0.109, −0.066] ms. This misses 2.5 ms. Shrine measured 1.728 ms versus 1.807 ms, delta CI [−0.333, +0.355] ms. |
| cap1b | The lane's native 27-tap splash reached 7.054 ms p95 for 65,536 particles, under 16 ms. Chrome matched 1,572,864 sorted grid words. Browser speed and the spectral scene's eight-buffer acceptance case remain open. |
| v15-bloat | The 187-source corpus median fell from 96,240 to 3,983 tokens. The lane's 854 ordered pictures held, including exact visibility and depth. |
| v15-stdlib-finish | Held for 5,120 unwritten HDR pixels at 256 spp in Harbor, despite correct Vis64 and finite depth. Keep std1d, std1c and port1 with their parent. |
| cap1c | Held at 2,325 runner lines versus 2,200. The six 64-byte material families met the two-pipeline 200-material fixture and the cold budget at 827 ms. The corrected bindless and live-edit probes still need a scored run. |
| v15-native | Held at 2,248 runner lines versus 2,200. Cooperative f32 took 0.528 ms p50 against 0.238 ms historical. MetalFX reduced p95 from 8.891 to 5.319 ms at 42.161 dB. |
| x2b / x2c | The x2c binding investigation is merged. The perf branch supersedes x2b's soft-float implementation, while x2b retains its separate Det verifier and regression checks. |

## Scoreboard

| Metric | Target | Measured |
|---|---|---|
| Rust core, SCC 3.7.0 | ≤18,000 | Reduced tree 37,984 (+19,984). Tested candidate 38,249. Incoming main 36,303. |
| Runner / native CLI / loft-std wgpu dependencies | ≤2,200 / ≤450 / 0 | Reduced tree 2,178 / 0 / 0. |
| Grammar | 100% in both recognizers | Candidate 311/311. Includes the expected rejection. |
| Corpus median | ≤10,000 tokens | Reduced tree 3,983 across 187 sources, down from 96,240. p95 7,340. |
| Workspace native tests | 0 failures | Candidate 183 passed, 0 failed, 1 ignored. The ignored archive case then passed separately. |
| wasm32-unknown-unknown | Entire workspace compiles | Candidate passed with web-sys enabled. The web dependency tree contains zero Metal or objc packages. |
| Deterministic pictures | Exact ID and D32, ≤1 code on <0.1% of pixels | Candidate 19/19 vs incoming main. At most 7 of 1,024,000 pixels changed per frame. Bead 9/9. |
| Stochastic pictures | Frames ≥27 dB, reference ≥35 dB, variance ≤0.0025 | Candidate 4/4 at 41.725–42.069 dB, reference 49.833 dB, variance 0.0000449137. Gain mutant rejected at 6.744 dB. |
| Twin and Truth | 481/481 ticks per rig, all scene checks | Candidate 481/481 each for Crab, Heron and Boss. Twin 36/36 and Truth 34/34. |
| Speed and Load | Six quiet pairs, paired 95% CIs within budget | 1/6 pairs. The second candidate preflight reached 0.3745 ms against <0.150 ms. Both intervals unavailable. Single-run checks were 12/12 and 22/33. |
| Hermit modules / pipelines | 8 / 13 in each scene | Baseline 8 / 14, candidate 9 / 15 at both 1 and 20 groups. Reproduced in the paired run. |

## Measurement scope

The scored baseline CLI is the archived v15-exec release. The incoming main tree has identical runtime sources and gate inputs. The two-tier gate uses fresh candidate captures, archived ordered baseline planes, the production Rust scorer and the reference-sphere gain mutant. Speed and Load use the six-pair exact sign interval with zero allowed regression and triangle p95 below 0.150 ms. Earlier per-lane performance results in the table above are from their respective fixtures and sampling windows. [Lane audit and reproduction](lanes/iv15_2.md) · [Targets](../../docs/V1_5_GOALS.md)

The one native release was built at `283e0b6`. The wasm boundary repair was checked on the larger candidate. After excluding v15-perf, all production native Rust matches the previously tested v15-bloat tip after target selection. The added cap1b files are examples, fixtures and web assets. These checks establish which branch introduced the Hermit change. A combined native release and whole-workspace wasm verification on the reduced tree are outstanding. [Raw verification](../../out/meta/iv15_2/)

The reduced source tree was pushed to `origin/main` at `675040e`. After verifying ancestry on the remote, the merged `loft/cap1b`, `loft/v15-bloat` and `loft/x2c` branches were deleted locally and remotely. The four held implementation tips, std1d, std1c, port1 and x2b were retained. Captures, build logs, timing samples and the candidate binary were copied to the primary checkout before worktree cleanup.
