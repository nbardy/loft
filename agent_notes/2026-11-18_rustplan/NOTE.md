# Compiler measurements, 18 Nov 2026

[Compiler proposal](../../docs/proposals/2026-11-18_compiler_speed.md). Baseline `26fa91a`, Bun 1.3.10, macOS
15.5, Apple M4 with 10 CPU cores and 24 GiB RAM. The GPU-backed `serve` parent drew one 64×64
frame at `t=0` per successful request, with one compile slot. The compile child ran with `LOFT_NO_GPU=1` and
`--confine` set to a measurement wrapper. The wrapper measures just the subprocess it launches. This run used no
Linux sandbox and makes no claim about its overhead. No compile time below includes a draw.

## The scene author still writes the same code

The measured PBR scene uses an ordinary function to replace one style part and a spread to replace another.
`film` writes its own expression over `v_h` and base `f0`.

```ts
const style: Style = { ...pbr({ fresnel: film(380) }), ink: () => vec3(0.05, 0.06, 0.08) };
```

| Label | Scene | Coverage |
|---|---|---|
| Owl | `examples/second/owl.ts`, `game` | Smaller creature, posed surfaces and expressive paint |
| Titan | `examples/bosses/reefback_titan.ts`, `game` | Large assembled creature and shared draw shaders |
| Film | `examples/materials/thin_film.ts`, `whole` | Thin-film PBR part, built-in PBR environment |
| Storm | `examples/storm.ts`, `three` | Gaussian blobs, fog layer/bank and soft lighting. Both CLI routes refuse the JavaScript fills for `storm_blobs` and `storm_clusters` at writing (exit 4). |
| Leap | `examples/second/heron_leap.ts`, `side` | Dynamic design build with the full authored heron `solve` |

## Phase data

`bench.ts` launches real CLI processes. It times spawn to exit and gets user + system CPU and peak RSS using
`Bun.Subprocess.resourceUsage()`. It hashes each successful directory outside the timed region, including names,
lengths and contents of `manifest.json`, `bin/`, and `wgsl/`. The timed wrapper for `serve` checks that the driver's
reply agrees with the child's exit and verdict. The parent `ms.compile` includes wrapper overhead and is logged
separately for successful requests. [Bun process usage](https://bun.sh/reference/bun/Subprocess/resourceUsage)

`instrument.ts` adds probes to a private source copy. It takes `process.uptime()` and `process.resourceUsage()`
snapshots on entering and leaving nested scopes, clipping out child scopes so they are exclusive. Start is time to
the first probe dependency, including early entry loading. Import includes dependencies' top-level execution,
including the beetle imported by thin film, plus the later host imports. The five scenes get a marker after their
static imports so their own top-level work lands in graph. Graph further includes variant construction and deferred
draw/style callbacks, excluding scoped compiler stages. Lower groups bind/fold/rate/rewrite work, final assembly
and host `lower`/`share`. WGSL covers `emitWgsl` including its traversal/library collection. Table time includes
declare/classify, pack/schedule, the CPU twin for fills, rest evaluation and grid measurement, excluding nested
lower/emitter scopes. Write includes delivery to the file system without an `fsync`. Storm stops at writer
validation. Solve is separate. CLI includes command parsing and residual logging/exit. Instrumented storm reports
an extra probe stack location. Uninstrumented builds retain the exact original verdict.

Each phase cell is **wall ms / CPU ms / max cumulative RSS MiB / added RSS high-water MiB**, median of three
fresh processes. Totals give wall / CPU / peak RSS. A phase can follow a phase that already grew RSS, so cumulative
RSS is an upper bound on what was resident at that phase, and only the last number records *new* high-water growth
within its spans. Allocation ownership is outside the resolution of this measurement. The phase medians are independent. The machine
was heavily shared, with 1-minute load 30–41 during build and 33–57 during serve measurements. Use the two paths for
the phase breakdown, with no platform-independent wall claim.

### `loft build`

| Scene | Bun start | Import | Graph | Lower | WGSL | Tables | Write | Solve | CLI | Total |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Owl | 48/48/44/44 | 89/107/142/26 | 84/168/127/36 | 87/111/152/17 | 16/31/133/5 | 144/153/142/25 | 11/11/153/0 | — | 2/2/153/0 | 477/627/153 |
| Titan | 57/51/45/45 | 229/445/443/115 | 1129/1274/360/123 | 1159/1181/507/110 | 99/114/409/17 | 821/853/443/78 | 22/18/507/0 | — | 3/3/508/1 | 3523/3872/508 |
| Film | 44/42/43/43 | 58/89/108/27 | 34/60/101/21 | 70/58/117/15 | 7/11/105/2 | 25/34/108/7 | 4/3/117/0 | — | 1/2/117/1 | 239/302/117 |
| Storm | 49/52/43/43 | 121/197/227/67 | 210/257/199/40 | 642/370/244/54 | 87/55/227/3 | <1/<1/244/0 | — | 3/3/245/1 | 1661/1298/245 |
| Leap | 46/50/43/43 | 48/85/995/27 | 357/228/995/5 | 280/151/998/3 | 114/41/995/0 | 314/219/995/2 | 11/7/998/0 | 4970/3323/984/916 | 2/2/999/1 | 6169/4108/999 |

### `loft serve` compile child

| Scene | Bun start | Import | Graph | Lower | WGSL | Tables | Write | Solve | CLI | Total |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Owl | 56/51/44/44 | 58/110/144/27 | 94/132/127/35 | 99/97/154/17 | 45/30/136/5 | 184/154/144/25 | 5/5/154/0 | — | 4/3/154/1 | 510/576/154 |
| Titan | 47/54/44/44 | 329/491/400/112 | 2774/1280/357/126 | 1652/998/487/106 | 156/123/357/30 | 1288/870/399/83 | 36/20/488/0 | — | 4/4/488/1 | 6305/3791/488 |
| Film | 33/45/44/44 | 68/95/105/26 | 80/65/95/20 | 61/52/114/13 | 8/13/101/3 | 42/45/105/7 | 18/3/114/0 | — | 2/2/114/1 | 442/322/114 |
| Storm | 49/49/43/43 | 182/220/221/70 | 227/248/188/35 | 767/395/246/65 | 120/51/221/2 | 743/389/221/32 | <1/<1/246/0 | — | 4/3/246/0 | 2600/1364/246 |
| Leap | 59/53/44/44 | 55/99/997/24 | 748/359/991/1 | 1050/293/999/2 | 185/69/992/0 | 1443/476/996/2 | 68/13/999/0 | 18395/4739/977/909 | 4/3/999/0 | 22096/6154/999 |

### Interning within phases

Interning is called throughout graph, lowering and tables and during `solve`. Per-call timers would perturb its tiny
operations. Two separate `bun --cpu-prof --cpu-prof-interval=500` runs per scene/route attribute main-thread leaf
samples in expression interners, including native calls whose first attributable JS parent is an interner. The
weighted wall estimate assigns each profile interval to its sample and averages the two runs. The sample share
is an estimate of the main-thread CPU share, covering JSC execution rather than process-wide CPU. Large stalls can
inflate the wall estimate. The rough CPU proxy scales the separately timed total CPU by the sample share. It
assumes unsampled threads have the same share and is unsuitable for a precise savings estimate. A separate probe
takes the process RSS high-water every 4096 interner entries. Its max
therefore includes earlier work and can miss the last calls' allocations. Node counts are exact for the fresh
process. Neither measure implies the interner owns all of those resident pages. [Bun profiler](https://bun.sh/docs/project/benchmarking)

| Scene | Build samples | Wall / CPU proxy ms | Intern RSS MiB | Serve samples | Wall / CPU proxy ms | Intern RSS MiB | Interner entries / created nodes |
|---|---:|---:|---:|---:|---:|---:|---:|
| Owl | 49/1070 (4.6%) | 17/29 | 132 | 94/1829 (5.1%) | 46/30 | 123 | 176,401 / 16,397 |
| Titan | 1100/11100 (9.9%) | 488/384 | 343 | 1683/17553 (9.6%) | 687/363 | 406 | 6,275,823 / 182,894 |
| Film | 13/466 (2.8%) | 63/8 | 76 | 29/1206 (2.4%) | 13/8 | 100 | 30,172 / 4,270 |
| Storm | 328/2560 (12.8%) | 383/166 | 209 | 401/6569 (6.1%) | 161/83 | 196 | 457,142 / 63,903 |
| Leap | 128/9179 (1.4%) | 540/57 | 986 | 469/20988 (2.2%) | 201/138 | 1040 | 448,710 / 39,750 |

The same profile points to repeated arithmetic operand validation/lifting, the graph walks, table fills and rate
analysis, and `host/share.ts` token classification. In heron the prominent frames are `solve` row evaluation and
its numeric kernel. The 16 Nov optimisation already uses numeric interning keys and reusable walk stamps, so a
Rust comparison should start from today's compiler. Interner time is a subset of the phase timings above.

## TypeScript trials

- Kept the index of surviving module-level constants in shader sharing, keyed by token offset. The prediction
  was that building the index once would replace an allocation and linear scan at each token, while selecting
  the same declaration at each offset. Over six alternating measurements of `lower` on the same already-compiled
  Titan artefact, median CPU fell from **545 to 403 ms (26%)** and median wall from 1762 to 1016 ms. An earlier
  replay gave 464 → 251 CPU ms. These are host-lowering results, with 78 output modules in each run.
- Reusing the WGSL emitter's topological list removed one dependency walk per emission. In a six-pair replay of
  1,045 Titan programs, median CPU was 205 → 201 ms, with four paired regressions and wall at 799 → 1066 ms.
  Removed the change because the result gave weak evidence for keeping it.
- Tried validating associative operands once and avoiding repeated literal lifting. The three-change trial produced
  identical bytes, but on Titan its first four build pairs spent 4.59–7.31 CPU seconds in the candidate against
  3.15–5.30 in the base, with three candidate regressions. Removed the arithmetic change before the final comparison.

The uninstrumented build pairs below are full process measurements of the retained shader-sharing change,
with one pair per scene and 1-minute load 47–57. Median improvements above apply to the isolated `lower` call.
The end-to-end pairs mainly establish artefact parity and show the effect of system contention on timing.

| Scene | Build wall ms | CPU ms | Peak RSS MiB | Output |
|---|---:|---:|---:|---|
| Owl | 1342 → 964 | 628 → 677 | 129 → 150 | Identical |
| Titan | 10848 → 9000 | 4468 → 4871 | 374 → 346 | Identical |
| Film | 903 → 880 | 408 → 422 | 95 → 104 | Identical |
| Storm | 4992 → 4740 | 2004 → 1972 | 223 → 210 | Identical exit 4 and verdict |
| Leap | 17459 → 16122 | 6080 → 6107 | 906 → 926 | Identical |

The same comparison through two real `serve` parents used one compile slot each, one pair per scene, and
1-minute load 34–51. All four SHA-256 directory digests agree across the two paths, including the phase runs.

| Scene | Serve child wall ms | CPU ms | Peak RSS MiB | Output |
|---|---:|---:|---:|---|
| Owl | 2735 → 2904 | 1125 → 930 | 90 → 121 | Identical |
| Titan | 48325 → 14234 | 9382 → 5194 | 234 → 350 | Identical |
| Film | 1710 → 1861 | 594 → 498 | 86 → 102 | Identical |
| Storm | 2952 → 2605 | 1411 → 1395 | 210 → 231 | Identical exit 4 and verdict |
| Leap | 11230 → 17725 | 4786 → 5700 | 933 → 907 | Identical |

## Verification

`pnpm typecheck` and the nine focused file/serve tests pass with the retained change. Owl's CLI check passes both
checks. Its build writes 20 files. Its 128×128 two-sample `game` still has RGBA hash `0ea00c36bf1f9cc1`.
The full suite was run after the initial focused tests and owl smoke checks, while screening the compiler trials.
The process ended with status 143 before Bun printed a final tally. The all-examples GPU pipeline test exceeded
its own 180-second limit, including when retried alone with the retained change. That test uses `compile()` and
hands its shaders directly to Dawn. Shader sharing runs later in `lower()`.

## Reproduce

Run from the worktree root. Prepare `out/compiler-speed/base` with the baseline `src/` and `examples/`, and
`out/compiler-speed/share-only` with the candidate files, each with a `node_modules` symlink to this worktree's existing
dependencies. Use new labels for each invocation. Keep the Bun transpiler cache local and hold it fixed across arms.

```sh
notes=agent_notes/2026-11-18_rustplan
run="$PWD/out/compiler-speed"
mkdir -p "$run/bun-cache" "$run/profiles-build" "$run/profiles-serve"
export BUN_RUNTIME_TRANSPILER_CACHE_PATH="$run/bun-cache"
bun "$notes/instrument.ts" "$run/base" phase-base
bun "$notes/bench.ts" build phase-build 3 "$run/phase-base"
bun "$notes/bench.ts" serve phase-serve 3 "$run/phase-base"
bun "$notes/summary.ts" timers phase-build phase-serve
LOFT_BENCH_PROFILES="$run/profiles-build" bun "$notes/bench.ts" build sample-build 2 "$run/base"
LOFT_BENCH_PROFILES="$run/profiles-serve" bun "$notes/bench.ts" serve sample-serve 2 "$run/base"
bun "$notes/summary.ts" profiles sample-build sample-serve
bun "$notes/instrument.ts" "$run/base" intern-base --intern-rss
bun "$notes/bench.ts" build intern-build 1 "$run/intern-base"
bun "$notes/bench.ts" serve intern-serve 1 "$run/intern-base"
bun "$notes/lower_bench.ts" "$run/base" "$run/share-only"
bun "$notes/bench.ts" build share-build 1 "$run/base" "$run/share-only"
bun "$notes/bench.ts" serve share-serve 1 "$run/base" "$run/share-only"
bun "$notes/summary.ts" paired share-build share-serve
```

Measure on the Linux training host next. The plan lists the pass/fail gate, port estimates, and workloads a warm
worker, graph snapshot or exact cache actually removes. No public API or artefact format changed.
