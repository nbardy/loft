# perf-precision: precision decided by role

Branch `perf-precision`. Owner decision: precision is decided by default from each value's role; the compiler forces
f32 where a role needs it; everything else is half where the device supports it; the author's only control is one
opt-out that forces f32 everywhere.

## 1. Role classification (design)

### What the tree already is (read before designing)

- `rgb`, `v3`, `vec3f` are the same type (`Ty::VecN(3)`, `crates/loft-parse/src/parser.rs:1791`). A colour and a
  position are indistinguishable by type. Only `p3`, `v3`/`d3`/`n3` (`Ty::Geo`), `Frame`, `Xform` and `Map` carry a
  geometric role. So a role cannot be read off a Loft value type today. Reading it off arithmetic would need a new
  colour type (a language change) or a dataflow analysis; neither is in this lane.
- A role IS known where a resource is created. `lower_program` creates five intermediate textures: visibility
  `Rg32Uint` (x2: view and shadow), depth `Depth32Float` (x2), HDR radiance `Rgba16Float`. Baked textures are
  authored with an explicit format (`r16float`, `rgba16float` in every scene in the repo; no scene uses `r32float`).
- The `luts` buffer is one `array<f32>` holding frames (origin, tangent, normal, binormal: geometry), world-table
  offsets read with `bitcast<u32>`, and baked scans of frames. All of it is position or integer data. Halving it is
  forbidden by the must-f32 list, and no sub-range is a colour or BSDF table.
- Kind rows are f32 words shared with the CPU twin and motion laws (state, positions, bit-cast ids).
- The runner has no f16 path: `GpuFeatures::F16` existed but nothing requested `SHADER_F16`.

So the storage that a role decision can change is the radiance texture, and it is already half. This lane makes that
a rule instead of a literal, makes the opt-out real (it stores radiance as `Rgba32Float`), and records the device
fallback. It adds no half arithmetic (section 4).

### Where it lives

`crates/loft-ir/src/precision.rs`, applied in `crates/loft-passes/src/bundle.rs::lower_program`, the one place that
creates intermediate textures and rewrites shader modules. Not in the parser or checker (no type carries the role) and
not in stdlib WGSL (another lane owns it).

- `Role { Radiance, Integer, Depth }` with `Role::format(Resolved) -> ImageFormat`. A resource is created through its
  role. `Integer` and `Depth` have no half arm, so the must-f32 rule is a property of the match, not something the
  author or a later edit has to remember. A new role is added when a resource needs it.
- Roles by owner list: Position, depth, ray parameters, SDF distances, time, ticks, law state, accumulators, atomics
  and integer data are f32 by construction (they have no half-capable storage path: rows, luts, params, atomics are
  untyped f32/u32 words). Colour, radiance, G-buffer colour, BSDF/material LUTs, noise and style LUTs, post are the
  half-eligible set: only radiance has a compiler-created resource today. Authored bake formats are the author's
  declaration and are never rewritten.
- Intended arithmetic rule (not implemented): a function may be emitted in half only if every parameter and the
  return type are in the radiance set and it keeps no accumulator. That needs a colour type to know; see section 4.

### Fallback: adapter lacks `SHADER_F16`

`Precision::resolve(features) -> Resolved`, three recorded variants:

| Resolved | Meaning |
|---|---|
| `Half` | `SHADER_F16` present: half storage, half arithmetic allowed |
| `HalfStorage` | adapter lacks `SHADER_F16`: half storage (needs no feature), arithmetic stays f32 |
| `F32` | opt-out |

`Loaded::precision` carries it, `loft profile --json` prints it as `precision`, `loft load` prints it. The native
device now requests `SHADER_F16` whenever the adapter offers it, so `GpuFeatures::F16` is true on the device.

### The opt-out

`--precision f32` on any in-process `loft` command (global; sets `loft_native::set_precision`). One name, one value;
any other value is a usage error. Per-value annotations do not exist. The default lives in one constant:
`loft_ir::precision::DEFAULT`.

Opt-out effects: radiance stored as `Rgba32Float` (shade module `hdr` store retargeted to `rgba32float`). Debug AOV
`overdraw` keeps the half texture (it counts layers, it is not radiance). Authored bake formats stay as authored.
`loft gate` and `loft controls` are exec'd into `loft-verify` and do not see the flag.

## 2. Prediction (written before any measurement)

At 1280x800, HDR is 1.02 M px: 8 B/px half, 16 B/px f32.

1. Default arm after change vs main: byte-identical. Default resolves to `Rgba16Float`, the shade WGSL is not
   retargeted, textures/passes unchanged. Expect 0 changed pixels on every gate scene and Crab Live, and identical
   module bytes.
2. Opt-out cost: +8.2 MB written by Shade and +8.2 MB read by Present per frame. Bandwidth bound is 16.4 MB / ~100 GB/s
   = 0.16 ms; stores are partly hidden under march ALU. Predict **+0.03 to +0.12 ms GPU per frame**, Shade pass
   +0.02..0.08, Present +0.01..0.04, i.e. +1.5..6% of a 2 ms Crab Live frame.
3. Opt-out pixels: half radiance has 11 significant bits (rel. error 4.9e-4); an RGBA8 code step is 3.9e-3 of full
   scale, ACES slope <= ~1.2. Predict max channel delta = 1 code, changed pixels 1..5% (a rounding flip needs the
   value within 5e-4 relative of a code boundary), so the f32 arm exceeds the "<1 in 1000 pixels" picture tolerance
   against frozen pictures. That is the cost of exactness, reported, not a default change.
4. Half arithmetic: predicted saving zero to negative on M4 (18 Nov measurement: f16.final.md, tied medians; Shade is
   latency/ALU in the march on f32 positions, not radiance math). Not added.

## 3. Measured (Apple M4, Dawn/Metal via wgpu, 1280x800, lab build, `tools/gpu_lock.sh`)

Baseline = lab binary of main 34bd927 built before the change (`out/prec/loft-base`); candidate = this branch
(`out/prec/loft-new`), default and `--precision f32`. Scenes: the seven gate scenes at t = 0, 4, 9 and `scenes/crab.loft`
(Crab Live; `scenes/crab_live.loft` does not exist on main or any worktree).

### Pixels (RGBA8 PNG; changed = any channel differs, of 1,024,000 pixels)

| Scene (3 times) | default vs main | `--precision f32` vs main |
|---|---|---|
| capsules-1 / -20 | 0 changed, max 0 | max 1 code; 0.27-0.32% / 1.3-1.5% |
| blobs-1 / -20 | 0 / max 0 | max 1; 0.22-0.23% / 0.93-1.11% |
| hermit-1 / -20 | 0 / max 0 | max 1; 6.7% / 6.2-6.3% |
| crab (scenes/crab.loft) | 0 / max 0 | max 1; 4.6-4.7% |
| coral_shrine | 102-121 px, max 142-164 | ~4% (40-44k px), same 100-pixel tail as default |

Shrine's default-arm difference is main's own nondeterminism, not this change: two more runs of the unmodified main
binary differ from the first main run by 38 and 75 pixels, and from the new default by 73-84. Everything else is
byte-identical: no frozen picture changes in the default.

The opt-out exceeds the "fewer than 1 in 1000 pixels" tolerance on every scene (the smallest, blobs-1, is 0.22%): f32 radiance is not a drop-in for the frozen pictures. Max delta is 1 code on all scenes
(prediction 3 hit: max 1 code; changed 1-5% predicted, measured 0.2-6.7%, hermit above).

### Timing: UNKNOWN

Timing is not scored. The machine load average was 14-24 during the runs, another lane held the GPU lock before mine,
and the triangle control p95 was 0.08-0.96 ms (the gate needs < 0.150 ms). Frame p50s were 2-6x the known quiet
numbers and varied by 2x between repeats of the same arm (hermit-20 base 18.5/15.4/8.0 ms). The per-arm files are in
`out/prec/t-*.json` (ignored output). Prediction 2 (+0.03..0.12 ms for f32 radiance) is therefore neither hit nor
missed. It needs a quiet window: `tools/quiet_gpu.sh` with `tools/gates/timing.py` on the two lab binaries.

## 4. Half arithmetic: not added

No saving can be predicted: Shade is march/BSDF ALU on f32 positions and distances (must-f32 roles), radiance math is
a small share, the 18 Nov M4 measurement tied, and a per-function rule needs a colour type that does not exist
(`rgb` == `v3` == `vec3f`). `SHADER_F16` is requested and `Resolved::Half` is reported so a later lane can add it
behind `Role` without another device change. No stdlib WGSL was edited.

## 5. Changes and risks

- `loft-ir`: `precision.rs` (new), `ImageFormat::Rgba32Float` = wire code 3 (additive; default bundles are
  unchanged byte for byte, an f32 bundle fails to decode on an older reader), `GpuFeatures: Default`.
- `loft-passes/bundle.rs`: textures made through `Role`; `retarget_hdr`; `Options::{precision,features}`;
  `Prepared::precision`; `replacement(source, hdr)`.
- `loft-native`: requests `SHADER_F16`, `set_precision`, `Loaded::precision`, features reported to the runner.
- `loft-wgpu`: `Rgba32Float` in `storage.rs` and `web.rs`. `loft-cli`: global `--precision f32`, `precision` in
  `loft profile --json`, `loft load` line.
- Not covered: web host (`loft-runner` embedded path) uses the default; `loft gate`/`controls` do not see the flag.
