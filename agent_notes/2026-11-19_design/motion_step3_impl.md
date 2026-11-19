# Step 3 motion implementation

Branch `feat/design-motion`, based on `5d94531`. The crab plays, scrubs and renders on the Rust path. Twenty machine checks have measurements. The four-second owner comparison is ready. The bake, timing tails, full-scene module reuse and two numerical predictions still have misses. Phase 2 has measured support, hop and gaze laws. [Phase plan and original predictions](../../docs/proposals/2026-11-19_motion_architecture.md#10-phased-plan) · [Owner comparison](motion_impl/comparison.mp4)

## What the consumer writes

```ts
import { body, foot, move, roam } from "loft/motion";
import { input } from "../../src/expr.ts";
import { stage, stick } from "./stage.ts";

const legs = [[1.35,2.8,6.6],[0.5,1.1,7.4],[-0.4,-0.9,7.3],[-1.3,-2.7,6.4]] as const;
const crab = body({
  trunk: { size:[4.8,1.05,5], height:2.5 },
  chains: [-1,1].flatMap(s=>legs.map(([x,fx,fz])=>({
    root:[x,0.1,2.5*s], lengths:[3,2.9,1.2], end:foot([fx,0,fz*s])
  }))),
});
export default stage(move(crab,roam(),{go:input("stick","vec2",[0,0]),script:stick}));
```

Removing `script:stick` makes the input live from the seed. A Rust host calls `loft.write_tick("stick", row, tick, &[x,z], None)?` before presenting a time. An instance has the same operation against its own log. An author can replace the arch, deck, step or presentation with WGSL, or supply the complete engine to `move`. `loft show … --thing stick` prints the emitted module with both entry points. The inspector labels the default take baked, 480 ticks at 120 Hz. [Crab](../../examples/motion/crab_live.ts) · [Explicit stage, draw geometry and input script](../../examples/motion/stage.ts) · [Motion library](https://github.com/nbardy/loft/blob/legacy/2026-11-19/src/motion)

BODY has 26 vec4s, STATE has 42, and POSE has 58 per actor. The first 50 POSE lanes are 25 rigid frames, translation followed by xyzw quaternion. Eight signed observation rows follow. The stage reads POSE for capsule legs, an ellipsoid shell and ornaments, coloured distal sites, and projected shadows on a marked ground. It issues three instanced grid draws. [Crab clip](motion_impl/scuttle.mp4) · [Reversal sequence](motion_impl/reversal.webp) · [Lower arch](motion_impl/low-arch.mp4)

## Core and runtime ownership

- The recorder writes rows, WGSL, an input source and the tick clock. The compiler validates the authored module in isolation before composing the generated entry. An invocation can only reach its own state and plan through arguments. The recorder fixture verifies typed refusals for module-private mutable state and invocation barriers, both of which can change results under fusion. Existing shared-row kernels in Rust bundles retain one dispatch per tick. The generic TS author-compute path still has its existing Rust lowering refusal. Old jobs keep their wire-4 bytes. A row step adds declaration 11 on wire 5. Existing Bundle V1 files decode with empty step declarations.
- Level and sampled-script sources supply one vec4 at each tick. Events supply a count and a bounded array in sequence order. Identical retransmission is idempotent. Wrong sequence reuse and over-capacity deliveries return typed refusals before any reader's log changes. The event check sends two impulses on the same tick, retries one, sends a zero-valued third impulse, then rewinds. Its other row stays at zero.
- A script bakes with the same `advance` entry used live. Its first host update resumes from a retained state. A level takes checkpoints every 60 ticks. Each instance owns its state, input changes and cursor. The take has an explicit finite horizon. Live uses a reusable 65-packet upload buffer per row and dispatches at most 64 owed ticks per batch. Host logs retain changes through the declared horizon for backward seeks.
- The frame consumes the newly presented rows before dependent cameras, frame kernels and draws. A trace records the actual observed-site margin, absolute witness tick, input digest, WGSL digest and evaluator. The stepped frame key contains checkpoint, prefix digest, tick and fraction. Public `loft serve` returns the per-frame history used in its identity hash. A two-frame CLI request returned ticks/checkpoints 60/60 and 120/120. [GPU host](https://github.com/nbardy/loft/blob/legacy/2026-11-19/rust/crates/loft-gpu/src/lib.rs) · [Tick scheduler](https://github.com/nbardy/loft/blob/legacy/2026-11-19/rust/crates/loft-gpu/src/ticks.rs) · [GPU trace](https://github.com/nbardy/loft/blob/legacy/2026-11-19/rust/crates/loft-draw/src/trace.rs)

## Instrument and baseline

Predictions were committed before the implementation. Count submitted frame kernels on the untouched lineup, expecting more than half of 4,905 compute pipelines to dispatch. The M4 Metal run at 96×64 found **4,880 frame fills**, four load fills and 21 named-camera pipelines. The selected named view adds two camera dispatches. Total build was 5,539 modules, 4,905 compute and 654 render pipelines. Two frames took 29.479 and 30.031 ms, including 13.678 and 13.799 ms in the kernel pass. Load took 154.023 s on this machine. The measured kernel-pass cost averaged 2.8 µs per fill, including its work. The existing 97.2 ms measurement was at 960×540. [Rust baseline](../2026-11-19_lean_rust_draws.md) · [Proposal predictions](../../docs/proposals/2026-11-19_motion_architecture.md#10-phased-plan)

The crab score compares all 481 state records, reconstructs each drawn distal site from the GPU joint quaternion and length, and injects the deliberate defect into the distal transform before observation. Performance uses GPU pass timestamps, eight warm-up frames and 56 measured two-tick frames. Whole-frame arms alternate order at 256×192, use matching fixed camera matrices, upload the matching recorded POSE at every tick, and compare RGBA at four times. Readback and CPU submission time are outside these GPU spans. The table uses the retained shader's acceptance run. [Harness](https://github.com/nbardy/loft/blob/legacy/2026-11-19/rust/crates/loft-gpu/tests/it/motion_live.rs) · [Fixture builder](https://github.com/nbardy/loft/blob/legacy/2026-11-19/tools/motion_fixtures.ts) · [Transform measurements](https://github.com/nbardy/loft/blob/legacy/2026-11-19/tools/motion_score.py)

## Phase 1 score

| # | Prediction and gate | Measured |
|---|---|---|
| 1 | 0/481 bit differences at 30, 60, 120 Hz and a six-tick stall | **Pass.** 0/481 in each. Dispatches/frames 120/120, 240/240, 480/480 and 238/238. Rewind to tick 177 also matched. |
| 2 | 22 releases, acceptable 20–24 | **Pass.** 22, distributed 2, 3, 2, 3, 3, 3, 3, 3. |
| 3 | First lifts at 1.042 s, site 7 one tick later, all by 1.392 s, ±2 ticks | **Pass.** First ticks by site 125, 152, 125, 167, 152, 125, 167, 126. |
| 4 | Input reversal 1.700 s, velocity zero crossing 1.900 s, next lift 2.283 s, ±3 ticks | **Pass.** Ticks 204, 228 and 274. |
| 5 | Friction clamp 5.886 m/s² and 32.0° lean at 2.025 s | **Pass.** 5.886002 m/s² and 32.0204° at tick 243. |
| 6 | Least share 0.060, hull margin 1.62 m, four planted, zero support refusals | **Pass.** 0.060538, 1.623218 m, four, zero. |
| 7 | Reach margin ≥0.17 m, leash never engaged | **Pass.** 0.172752 m, zero leash corrections. |
| 8 | Predicted ≤0.03 mm slide after IK, gate ≤0.1 mm, zero swinging neighbours | **Pass.** Worst reconstructed hold 0.000819 mm. Zero overlapping neighbours. All eight GPU trace checks held. |
| 9 | A 20 mm distal offset fails site 3 alone throughout stance | **Pass.** 19.99986–20.00011 mm across all 384 presented stance samples. Other sites ≤0.00082 mm. Trace exit 5, worst signed margin −19.9001 mm. |
| 10 | Final travel ≤0.65 mm horizontal, zero vertical, durations 0.120–0.334 s, heights 0.250–0.985 m | Horizontal 0.64850 mm. Touchdown height exactly zero. **Finite vertical travel missed** at 9.33536 mm on the last interval. The quartic's zero endpoint derivative allows nonzero travel in the preceding tick. Duration 0.12000–0.33415 s, height 0.25000–0.98462 m. |
| 11 | Final z −1.076 m, velocity −0.086 m/s, ±0.02 | **Pass.** Integrated z −1.075583 m and velocity −0.086431 m/s. |
| 12 | Two openings followed by the same input from 1 s yield up to 0.26 m separation, distinct keys. Identical histories give identical keys and RGBA. | Identity and RGBA **pass** for both comparisons, including baked vs live. The proposal leaves the second opening unspecified. Holding still through tick 119 gave 1.633205 m world separation, 0.869567 m root-relative, and 457/481 different states. **Magnitude missed** for that opening. |
| 13 | Urgency-scaled lower arch changes one module, preserves lift ticks, scales swing heights | **Pass.** Exactly one of five shader texts changed. Zero differing modes. Greatest height reduction 0.116625 m, worst scale error 0.000828 mm. |
| 14 | One dispatch on measured spans. Solo and 256 p95 ≤0.065 ms, 1,000 predicted 0.03–0.09 ms with ≤0.2 ms gate | One dispatch per ordinary frame and stall. Solo **0.0701 ms**, 256 **0.1007 ms**, both missed the tail gate in this run. 1,000 **0.08625 ms**, passed. Solo and 256 medians were 0.0402 and 0.0470 ms. |
| 15 | Same draws with recorded POSE within 0.065 ms | Paired difference **0.04446 ms median**, **0.09029 ms p95**. Median passed, tail missed. Other matched-camera runs gave p95 0.0600–0.0973 ms. Equal RGBA on every comparison. |
| 16 | 480 ticks in one dispatch, predicted ≤3.6 ms, gate ≤5 ms | One dispatch, **5.3052 ms**. Bake time missed. |
| 17 | Identical hashes for one and five bodies, one motion module/two pipelines, ≤8 modules, frames within 0.065 ms, setup ≤0.45 s | Both built **5 modules, 3 render and 3 compute pipelines**, including the same motion module/two entries. Cold first load of the final five-module shader set was **0.310 s**, warm around 0.014 s. Two draw hashes changed. Whole-frame medians 0.10646 and 0.28963 ms. **Full-scene reuse and frame-delta gates missed.** |
| 18 | STATE 672 B, POSE 928 B, retained take 323 KB | **Pass.** 672 B, 928 B, 481 × 672 = 323,232 B of retained state. Slot zero, input storage, GPU alignment and staging are additional. |
| 19 | Crab 235 tokens, gate ≤250 | **Pass.** 216 `o200k_base` tokens for `crab_live.ts`. The reusable stage and shared stick script are separate from that count. |
| 20 | No motion-domain terms in Rust crates | **Pass.** The proposal's case-insensitive whole-word search returned zero matches. |
| 21 | Owner sees weight at least as clearly as panel (c), planted braking and no moonwalk | **Owner verdict pending.** The comparison shows original (a), original (c), then Rust, in sync at 24 fps. [Compare all three](motion_impl/comparison.mp4) |

## What the misses taught us

The first correct implementation baked in 14.31 ms. The support mask changes only on lift and touch. Caching the hull order and affine load solve at those boundaries cut the first measured bake to 6.80 ms and the warm bake to about 5.3 ms. The revised 6–10 ms prediction held on the initial rerun. A further trial deferred edge projection until outside the hull and reused the lean quaternion across all roots. Its 4.7–5.1 ms prediction failed. Eight alternating bakes gave baseline median/p95 5.313/5.351 ms and trial 5.325/5.356 ms. The branch retains the earlier code. A workgroup-per-actor lowering now has an empirical baseline.

A first implementation extrapolated the previous aim across a retarget. The last f32 instant before an integer tick differed from the next sample by up to 8.150 mm at a distal site. The retained implementation evaluates the selected tick's start velocity, acceleration, attitude endpoints and changing swing aim. Across all 480 joins the actual differences fell below the predicted 0.01 mm, to 0.00073 mm at the root and 0.00664 mm at a distal site. The corrected near-vertical quaternion also took the initial 5.3 mm plant error below one micron. [Tick presentation](https://github.com/nbardy/loft/blob/legacy/2026-11-19/src/motion/wgsl/present.wgsl)

## Phase 2

| Law | Prediction | GPU measurement |
|---|---|---|
| Four-pad load | At (0.15, 0), positive-x/positive-z pad can carry [0.25, 0.50], least-norm 0.375 | **[0.25, 0.50], 0.375**. The query checks feasible singleton, segment and triangle supports. |
| Moving support | A 0.5 m/s deck carries a held site 0.500 ±0.001 m in 1 s | **0.50000000–0.50000003 m** over eight sites. Worst observed residual in deck coordinates 0.000480 mm. |
| Strength-limited hop | 0.770 s stance, 0.590 m crouch at 0.513 s, peak push 2.02 g at 0.583 s, ballistic flight | **0.770112 s**, **0.589525 m** at the nearest sampled minimum (0.516667 s), **2.02154 g** at 0.583333 s. Maximum flight position difference from the f64 parabola 0.000000574 m. |
| Grounded gaze | 1 rad/s target gives 1.58 rad neck, zero error, trackable through 1.73 rad/s | **1.57947 rad**, max error 1.2×10⁻⁷ rad. At 2 rad/s the body saturated at **1.72691 rad/s**, with 0.69964 rad remaining error. With zero support the yaw stayed zero and final error was 2.100 rad. Zero flips at rear crossings. |

The hop and gaze measurements use headless row steps and require zero new Phase 2 Rust primitives. The drawn heron, head-position follower, fixed-length S-neck, finite support yaw torque, head pitch and jerk remain open. No claim is made for the neck, skull or jerk gates. Phases 3–5 remain open. [Hop law](https://github.com/nbardy/loft/blob/legacy/2026-11-19/src/motion/wgsl/plan.wgsl) · [Gaze law](https://github.com/nbardy/loft/blob/legacy/2026-11-19/src/motion/wgsl/head.wgsl)

## Size and merge plan

Net `scc` code lines against `5d94531`, counting changed files on both revisions: IR +81 of 120, compiler +114 of 180, GPU host +848 of 300, TS recorder +21 of 60, body/move/index +77 of 400, library WGSL +261 of 650 (includes the Phase 2 laws), example engine +77 of 190, scene plus stage +88 of 120. GPU integration checks added 391 lines against 250, including the paired timing instrument, event replay and Phase 2. The GPU overrun comes from the generic fused timeline, retaining independent history for shared-row kernels, bind-group setup and validation. Additional code is +131 in the draw executable, +80 in CLI/frontend glue and +181 in the scoring tools. Bring the host overrun to integration review.

1. Integrate the two lanes on a review branch. At inspection, render was `48ba85a`. Its wire-5 feature is a twelfth span for ordered passes. Motion uses tag 11 in the declaration span. Preserve wire-4 byte identity when neither feature appears and accept the two wire-5 extensions together. Reconcile the TS packer, Rust decoder, native stamp and test fixtures in the same revision.
2. Preserve motion's per-instance state/input prefix and render's previous-frame reads in the composed identity. Run steps before any ordered pass or camera that reads POSE. Keep render's actual pass graph, attachment handling and provenance when resolving common changes in the GPU host, CLI and bundle.
3. The existing grid draws read static geometry tables whose offsets and bounds specialize two shader texts when the row count changes. Bind the geometry dimensions as data in the render lane's per-kind draws. Consume the same 32-byte joint frames and the post-IK observation table. Repeat checks 13–17 with both lanes and compare the picture before retiring the grid path.
4. The owner decides check 21 from the comparison. A visual rejection reopens the load and release law before further picture-dependent work. The integrating reviewer owns the combined gate. This branch has only focused runs and stays local.

## Reproduce the focused score

```sh
tools/dev.sh build loft-node --fast
tools/dev.sh build loft-draw --fast
bun tools/motion_fixtures.ts out/motion-score
env LOFT_TAKE_DIR="$PWD/out/motion-score" tools/dev.sh test loft-gpu --fast --retries 0 \
  --run-ignored ignored-only -E 'test(recorded_take_score)' --no-capture
uv run --no-project python tools/motion_score.py out/motion-score
LOFT_ENGINE=rust bun src/cli/main.ts build examples/motion/crab_live.ts --out out/motion-crab
bun src/cli/main.ts check examples/motion/crab_live.ts
bun src/cli/main.ts show examples/motion/crab_live.ts --thing stick
rust/target/lab/loft-draw trace out/motion-score/broken.loft 0 '[{"label":"toe 3","word":212,"negative":213}]'
```

The last command should exit 5. The IR decode/refusal, bundle round-trip and identity checks passed, as did the live rewind and instance-draw checks. TypeScript typechecking and crate-local Rust lint passed. The three clips contain 96 AV1 frames each at 24 fps (SVT-AV1 preset 10, CRF 42). The 1536×192 WebP strip was encoded at quality 95.
