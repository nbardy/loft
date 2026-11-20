# Build1 · Cargo dependency reuse

Apple M4, rustup-managed rustc 1.99.1, sccache 0.18.0, two jobs, incremental off. Cold base `origin/main` at `1fd161f`, seeded base `0350445`. Both compile locks had zero wait.
CPU columns give user + system seconds from `/usr/bin/time`. Rust compiles spawned by the sccache server are outside that accounting.

| `tools/dev.sh` command | Cold wall (s) | Seeded wall (s) | Cold CPU (s) | Seeded CPU (s) | Third-party units rebuilt |
| --- | ---: | ---: | ---: | ---: | ---: |
| `check --workspace` | 175.74 | 15.17 | 112.11 | 1.60 | 107 → 0 |
| `build loft-cli --fast` | 157.72 | 102.90 | 58.22 | 40.61 | 107 → 0 |

Predicted and observed zero third-party rebuilds, so the dependency compilation wall-time target (≤20% of cold) was met. Each seeded command rebuilt nine Loft crates. Total wall was 118.07 s versus 333.46 s cold (35.4%).

| sccache server counters | Requests | Rust misses | Hits (all C/C++) |
| --- | ---: | ---: | ---: |
| Cold before → after | 870 → 1183 | 550 → 777 | 4 → 6 |
| Seeded before → after | 1303 → 1324 | 879 → 897 | 6 → 6 |

An isolated sccache probe hit on the second identical Rust invocation, then missed when only its working directory changed.
Cambium's layer receipts carry metadata. The per-toolchain seed store was empty until publication from the primary checkout.
`tools/dev.sh seed` warmed and published 261 debug and 180 lab dependency fingerprints. Fresh lanes cloned those, including WGPU and Naga, into private targets with zero Loft fingerprints before Cargo ran.
Cambium compares full compiler identity. Homebrew Rust 1.99.1 with LLVM 23.1.3 refused the seed built by rustup's LLVM 23.1.1. The conductor puts rustup first on PATH. For manual lanes use `PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH" git cambium add …` when creating a lane.
Refresh after dependency, profile, or toolchain changes. One publisher supplies an immutable snapshot and each concurrent lane writes to its own target. A held Cargo build lock refused publication and preserved the existing seed.

Keel's lane script explicitly applies a registry-only seed while Room Runners skips both target trees in its Cambium config. Loft uses Cambium's native seed policy.
Raw timings, cache counters, seed records and test logs are in `out/meta/build1/`.
