# C1b · Narrow physical registers for native compute edits

Base `173d6e9a2b4fa36fcb45c0957f75e9c44f8e6dfa` from fetched `origin/main`.

## Prediction before implementation

For the four-word promotion tape, each encoded register field is in 0–2. OR the words in
a tape of up to eight instructions and test the upper three bits of each five-bit register
field, including the low five bits of the immediate. A zero mask proves all four register
indices fit, including the eager reads of otherwise unused operands. A nonzero mask selects
32 registers. An immediate such as a swizzle or loop PC can conservatively select the wide
VM. The count and words come from the existing bucket buffer, so the choice is uniform and
takes effect on the first dispatch after upload. This makes at most eight extra word reads
in the short path and reduces the register array from 512 to 64 bytes per invocation.
Both widths share one WGSL instruction body. The tape keeps its count word, 56 opcodes,
logical register IDs 0–31 and existing host validation.

Predict **+0.250 ms** paired p95 overhead, a pass in each of three observations against
**+0.500 ms**, upload **≤2 ms**, and zero foreground pipeline creates. The archived del2
observations were **+0.581/+0.667/+0.796 ms**. C1's full bank measured
**+0.779/+0.545 ms**, the four-register control **+0.063/+0.033 ms**, and the combined
shader **+0.228 ms**. Each planned observation runs the original 140,000-point test,
4,375 workgroups of 32, 120 interleaved pairs with the first 20 discarded, and the
original paired p95 statistic and promotion assertions.

Predict CPU and compute field agreement within 1e-5 at the 4/5-register and 8/9-word
boundaries, register 31, loop execution and an immediate above 3. The non-sphere edit
will use a short box tape. Predict ≥99.9% of promotion pixels within 1/255.

[Archived C1 probes](../../../out/meta/c1/) · [Lane evidence](../../../out/meta/c1b/)

## Implementation and instrument

The compute entry takes the register-width decision from the C1 probe. Compile-time
assembly emits the same WGSL instruction body with arrays of four and 32 vec4s. The
authored interpreter uses the 32-register body. Its operation source compared byte
for byte with base. The mask admitted exactly 256 of 1,048,576 combinations of the
four encoded index fields. The runner's upload path keeps the existing count prefix
and validates the whole tape before rebinding the prebuilt pipeline and writing bytes.

The score uses source commit `b1c809729506b0f4f103f9d22b0d31fbd61b91b6`.
The all-target Rust check passed after correcting relative includes at the macro
call sites. One optimized Cargo build emitted the executables. All six focused
`loft-passes` tests passed, including Naga validation of both widths, all 56
dispatch IDs and the CPU opcode reference tests.

[Check logs](../../../out/meta/c1b/check.log) · [Cargo artifact JSON](../../../out/meta/c1b/build.jsonl) · [Executable hashes](../../../out/meta/c1b/executables.json) · [Mask check](../../../out/meta/c1b/mask-proof.txt)

## Native compute score

Apple M4 Metal, 1 ns timestamp period, default adapter selection and no device or shader
overrides. Each observation used the same executable and acquired `tools/gpu_lock.sh`.
PF1 held the lock when observation 1 queued, for a wait of about 178 seconds before its
test started. Observations 2 and 3 acquired the lock in their start second. Other
applications' GPU activity was unmeasured.

| Observation | Upload, ms | Paired p95, ms | Pixels over 1/255 | Exit |
| --- | ---: | ---: | ---: | ---: |
| 1 | 0.113666 | +0.243583 | 0/140,000 | 0 |
| 2 | 0.089291 | +0.256750 | 0/140,000 | 0 |
| 3 | 0.086875 | +0.230334 | 0/140,000 | 0 |

The worst lane observation was **+0.256750 ms** against **+0.500 ms**. The benchmark
promotion changed the pipeline after the first edited frame. The first edited frame
used the pre-upload VM pipeline, and the edit path has zero foreground pipeline
creation calls by source audit. Driver-level creation counts were unmeasured.
The timestamp and upload records above include every planned observation.
No test or GPU reruns were needed.

The pre-edit image differed from the edited image. The edited and promoted raw
float32 RGBA buffers matched byte for byte in each observation, with the same
SHA-256 `885f4be2eb6a54a38ad3e6c2807c2f1a8ee389b9aa78b3e6b7bd5475b5441a14`.
Raw timestamps, float32 readbacks and benchmark commands and exits are retained.
[Observation 1](../../../out/meta/c1b/promotion-1/test.log) · [Observation 2](../../../out/meta/c1b/promotion-2/test.log) · [Observation 3](../../../out/meta/c1b/promotion-3/test.log) · [Timing controls](../../../out/meta/c1b/timing-controls.json) · [Pipeline audit](../../../out/meta/c1b/pipeline-audit.txt)

## First-frame and validation controls

Each control ran two points through the CPU tape and the production GPU entry.
All 14 output vec4s matched bit for bit on the first frame after `hot_swap_tape`.
The preloaded legacy count-prefixed buffer also matched before any upload.

| Control | Words | Physical registers selected by the mask |
| --- | ---: | ---: |
| Four registers | 8 | 4 |
| Five registers, including a third-source operand in register 4 | 8 | 32 |
| Four registers | 9 | 32 |
| Register 31 | 2 | 32 |
| Legal two-trip loop | 5 | 4 |
| Swizzle immediate 27 | 3 | 32 |
| Box edit | 4 | 4 |

The host rejected an empty tape, premature return, reserved opcode and mismatched
loop with `RunnerError::AbiMismatch`. The previously uploaded box still produced
the same fields on the next frame. The pipeline identity held for all seven uploads.
[CPU log](../../../out/meta/c1b/cpu/test.log) · [Compute log](../../../out/meta/c1b/compute-controls/test.log) · [Lossless comparisons and hashes](../../../out/meta/c1b/comparisons.json)

## Remaining score

The measurements cover this native compute entry on Apple M4 Metal. Full-screen
authored VM timing, LUT and texture sampling parity, browser promotion and other
adapters remain unmeasured in this lane. The combined suite and five-gate run
belong to the integrator.

[Static checks and grammar evidence](../../../out/meta/c1b/static.log)
