# D2A · Deterministic authored motion

## Predictions before the build

Base `6776d49`. The inherited controls isolate the first ordinary multiply/add errors in Crab x at tick 2 (`3c162fca/cb`), Heron score hip at tick 19 (`3f51b2fb/fa`), and Boss windup at tick 8 (`3c593be3/e1`). Separate rounding should select `3c162fca`, `3f51b2fb` on the old control score, and `3c593be3`. The live Heron hip now samples the solved curve and is expected to differ from the old score. The 0.60 s knot must remain `3f170a3d`. The tick-zero reachable-end reductions also need strict arithmetic. Expected packed verdict counts per tick are 2, 4, 2 at 32 B per record; the Boss outbox uses a 16 B header and 32 B records. Predict 0/481 differences on each bound table, including zero-tick checks, and 0/481 at every grouping/replay boundary.

The full target STATE/POSE layout is Crab 816/800 B, Heron 336/512 B, Boss 2,928/3,072 B per creature. BODY uses 112/112/48 B per body/chain/segment. At the starting head the drawable entries have only 60/60/76 B row buffers, including draw metadata. A solved Heron prior adds 4,688 B (32 + 97 × 48).

Integer significands give separate rounding without requiring a backend optimization switch. Add/sub use three low rounding bits, multiply a 48-bit product, and FMA aligns the exact product before the only rounding. Division can extract 28 quotient bits in four seven-bit integer chunks. Sqrt uses 27 fixed restoring steps. Dot goes from the first lane through the last lane; distance subtracts before dot. Row-local ownership avoids cross-workgroup reads of current-tick writes.

Expect Crab lift at tick 10, drift under 0.001 mm and the 20 mm mutant missed. Predict 0.18 ms p95 for the 1,000-row dispatch with software divide/sqrt and both toe checks, above the 0.15 ms budget. Expected Heron pitch ceiling is 9.65°, max neck error under 0.2 mm and zero rear flips. Exact knot 0.59 m remains, and a nearby strict Hermite sample can undershoot it. Boss should hit/slip at ticks 60 and 75, rise to about 0.69 m, and produce the same two event records with zero drops as the v2 control. Predict that the 0.5 s Heron frame differs from main due to the new non-knot hip, and that Crab and Boss can differ because their old Metal path contracted the motion operations. Required picture result is zero changed RGBA bytes each.

## One build and disposition

The scored source head was `a88a294abc522a9c666cdf5528ff5dfa40e65478`. The single `CARGO_BUILD_JOBS=2 tools/dev.sh build loft-cli --fast --workspace --all-targets` acquired the shared lock and exited 101 in `loft-passes`. The only build produced no D2A focused executables. Full stderr is preserved in the primary checkout under `out/meta/d2a/build.log`.

| Rust error | Smallest source operation |
| --- | --- |
| E0308 at `det_check.rs:102` | `let ty = \|n, scalar\| …` infers `&&str` from `ty(n, t)` in the zip, then rejects `ty(width, ret)` with `&str` |
| E0502 at `det_check.rs:552,554,556` | `let validate = \|m\| … validate(m)` retains the first immutable module borrow across mutations because the same inferred closure input lifetime is used again |

The failed source is preserved as scored. The compiler-level correction is an explicit `&str` parameter on `ty` and an explicit `&naga::Module` parameter on the validation closure. Neither correction was built.

**Numeric gate unscored.** Zero of the 481 candidate ticks were read. The candidate lowers the existing drawable-row laws/checks and dedicated row-local solvers through a typed Naga rewrite. It binds the Heron prior on the drawable path and has a source-evaluator LUT binding plus a lazy source fallback. It does not add independent BODY and POSE buffers or connect the dedicated solver to the three drawable scenes. The emitted event path still uses the standard atomic reservation, so multi-row event ordering remains open. The fixed polynomial chains are inherited from v2. Raw loops are refused, including fixed-bound raw loops, and the source loop proof accepts literal intervals. Runtime correctness of the strict helpers and rejection cases is unverified.

**Picture gate unscored.** The retained 1280×800 main frames decoded to 4,096,000 RGBA bytes each and matched these archived hashes. No candidate image was rendered and the retained inputs were untouched.

| Frame at 0.5 s | Retained RGBA SHA-256 |
| --- | --- |
| Crab | `9d18ab5033df547173114e67b8bee097523b66a308212c7c524ff5a8e66c5879` |
| Heron | `401553ba707cfd617ff085bfa346c272b40e8611160298ee206e0c24160c3858` |
| Boss | `a5d8f485d67c9a53068f7167656aefd42afc2bf7b16822c246a4e6644863590f` |

## Minimal motion reproducer carried forward

These are the retained v2 measurements. They were obtained before D2A and are independent of the Rust build failure.

| Entry and tick | Source operation | Ordinary CPU/Metal | Forced two roundings, both | Explicit FMA, both |
| --- | --- | --- | --- | --- |
| `Scramble.advance`, 2 | `x + dt * mid_v`, with `x=3b9d036b`, `dt=3c088889`, `mid_v=3f066667` | `3c162fca / 3c162fcb` | `3c162fca` | `3c162fcb` |
| `HuntingHeron.advance`, 19 | Inner `b + u*c` in `quintic_plan` feeding the old score | `3f51b2fb / 3f51b2fa` | `3f51b2fb` | `3f51b2fa` |
| `HermitBoss.advance`, 8 | Outer `a + u*(b + u*c)` in `quintic_plan` feeding windup | `3c593be3 / 3c593be1` | `3c593be3` | `3c593be1` |

Tick-zero verdict word 6 also differed in the ordinary v2 entries. Crab `38519d17 / 38516fb4`, Heron `3a0311ef / 3a030ac3`, Boss `3a030eaf / 3a030d55`. The first tapped reachable-end operations were Crab's hip-depth multiply feeding `( toe − hip ).z`, Heron's squared-length dot, and Boss leg 1's `delta.y / length`. The two v2 traces and one-site controls are preserved in `out/meta/v2/twin-final.log` and `out/meta/d2/{motion,variants}.log` in the primary checkout.

ESCALATE
