# Pictures, Speed and Load

Scene modes are declared in `tools/gates/pictures.json`. The existing
`engines/gates.json` still owns deterministic sample times, size, approved
beauty hashes and timing budgets.

## Deterministic pictures

```sh
uv run --script tools/gates/pictures.py deterministic MAIN_CAPTURES CANDIDATE_CAPTURES
```

Generate each capture from its checkout and lab build artifacts.

```sh
CARGO_BUILD_JOBS=2 tools/dev.sh build loft-cli --fast --bin loft \
  --message-format=json-render-diagnostics >out/build.jsonl
uv run --script tools/gates/pictures.py capture out/build.jsonl "$PWD" out/pictures
```

The instrument copies the native executor into scratch and enables copy and
retention on its original visibility and D32 depth attachments. It reads
RGBA8, the first u32 visibility word, and D32 depth from the same render.
It uses the compiler's indexed cull ordering to resolve equal-depth ties by
row, while keeping each arm's frustum calculation. The timing build uses its
normal cull ordering. Each directory records the adapter, scene settings,
sample times, and `SCENE_tTIME.{png,id.u32,depth.f32}`.

Against main, any RGBA channel change must be at most one 8-bit code and fewer
than 1 in 1,000 pixels may change. Exactly 1 in 1,000 fails. Both structural
planes must match byte for byte. The scorer also verifies the frozen PNG's
decoded RGBA SHA-256 and reports the frozen comparison for review. The native
gate continues to check the owner-approved look. Both gates read existing
approvals.

## Stochastic and temporal pictures

```sh
tools/gpu_lock.sh uv run --script tools/gates/pictures.py stochastic \
  BASELINE_CHECKOUT/target/lab/loft CANDIDATE_CHECKOUT/target/lab/loft out/path-check
```

The path-traced sphere uses the production estimator. The capture tool
specializes its secondary-ray RNG for each declared seed, while keeping
subpixel jitter and the camera fixed across the sequence. It renders two
independently seeded higher-sample observations using the baseline estimator.
The reference must meet the declared convergence PSNR at twice its sample
count. Candidate frames each need the declared RGB PSNR against the reference.
The scorer averages sample variance across pixels and RGB channels at the
static camera and checks the declared limit. A zero MSE appears as JSON
`null` for PSNR. The baked-in mutant scales estimator radiance to 10% and must
fail. Probe sources and PNGs go in scratch.

Score an existing capture for a declared stochastic scene with
`pictures.py sequence SCENE CAPTURES`. The directory supplies `reference.png`,
`convergence.png` and one `seed-SEED.png` per declared frame seed.

## Paired timing

```sh
uv run --no-project python tools/gates/timing.py \
  BASELINE_CHECKOUT/target/lab/loft CANDIDATE_CHECKOUT/target/lab/loft out/timing \
  --margin 0.05
```

Use prebuilt lab binaries kept in their respective checkouts. The command
reserves the GPU and both shared build slots, waits for active Cargo and rustc,
and polls for builds that bypass the slots. Any detected overlap invalidates
the timing session. The conductor uses the guard's admission-only `--wait`
before starting a step. The timing command holds the reservation throughout.

Each six-pair session alternates AB and BA ordering. Each arm records a triangle
control before its gate and the normal Speed control for each scene. Any
control above 0.350 ms makes that scene's timing verdicts unknown. Reports
retain both controls, frame batches, cold and warm loads, and binary hashes.
A Speed observation is a batch p95 across the manifest orbit. Cold and warm
Load observations come from the same batch, with the existing cold nonce and
warm cache procedure.

The scorer uses two-sided exact sign intervals for the median of each timing
metric and of its paired candidate-minus-baseline differences. With six
independent pairs the min/max interval has 96.875% coverage under continuous
sampling, the discrete interval covering at least 95%. The candidate interval
must fit the absolute budget and the difference interval must fit the
declared margin as a fraction of that budget. An interval spanning a budget
yields unknown. Unpaired CLI point estimates are diagnostics. The harness
retains the native non-timing checks. `tools/check.sh full` supplies paired
timing via `LOFT_BASELINE_BIN` and `LOFT_TIMING_MARGIN`. It uses
`LOFT_BASELINE_BUILD_LOG` for the baseline picture instrument and writes all
gate evidence below its new `out/check.*` directory. The scope of
`--only` is one or more budgeted scenes.

Replay the same six pairs with
`uv run --no-project python tools/gates/timing.py replay out/timing`.
The replay requires a successful guard exit and scores the original paired
measurements.

Run the focused scoring controls with
`uv run --script tools/gates/test_scoring.py`.
