# Training-host contract: what changed in LOFT

Branch `training-host-contract`, from `main` at `a2d82a2`. Asked for in
`creative-render-backend/agent_notes/2026-11-13-render-backend/CONTRACT.md`: a model prompted with the unedited
`loft context` and its full `scene({...})` module drawn by LOFT's own CLI. That study found four gaps; this branch closes three
of them as asked. The fourth (a batch command) is not added because the measurements don't support it. A different change, measured, gives
the speed-up instead.

## What a caller types now

```sh
# 1. Lossless frames: --out ending in "/" writes dir/0.png, dir/1.png, ... (in --times order), each at --size,
#    exactly the RGBA drawn (8-bit RGBA PNG, alpha 255). A .webp --out is unchanged, byte for byte.
loft frames JOB/examples/scene.ts --times 0,0.37,0.91,1.53 --view whole --size 512x512 --out JOB/frames/
loft render JOB/examples/scene.ts --views whole,focus --size 512x512 --out JOB/views/      # dir/0.png, dir/1.png in --views order

# 2. whole always draws (render --views, frames/clip --view). The scene's own camera "whole" if declared; otherwise one
#    derived from the bounds of its things, and stderr says so, e.g.:
#    camera "whole": the scene declares none, so it was derived: framing 23 things (shell, leg0_0, tip0_0, leg0_1, ...)
#    within [-1.8, -0.1935, -1.0732]..[1.8, 1.9926, 1.5097] over t=0,0.5,...,4: eye [5.4441, 4.0753, 7.477], target [...], fovY 0.6
#    To tell declared from derived, match that line on stderr.

# 3. Budgets, before any GPU work (render, frames, clip; either limit alone is allowed):
loft frames JOB/examples/scene.ts ... --budget triangles=120000,draws=96
#    over: exit 4, last stderr line
#    loft: scene: --budget: given 234786 triangles (limit 120000); expected at most 120000 triangles and 96 draws.
#      Fix: lower a strand's stations or detail ({ along, around }: the defaults 32 and 96 x 24 are the most a strand
#      costs) on the heaviest things (triangles: shadows 22816, shell 9216, leg0_0 9216, ...)

# 4. Batch: nothing new to type. The same per-scene command is now ~1.6-1.75x faster (below).
```

The counts are LOFT's own `artefact.stats` (`maxTriangles`, `draws`), the same fields the training hosts check today
(`training/loft_crab/render.ts:30`, `frozen-host/render.ts:40`), so the limits mean the same thing in both places.

## What changed

| File | Change |
|---|---|
| `src/cli/args.ts` | `Out` (`sheet` \| `pngs`) from `--out`; `--budget` parsed to `Budget` (Infinity where not given); usage text |
| `src/cli/image.ts` | `writePngs(dir, frames)` (the existing `encodePng`, one file per frame) |
| `src/cli/whole.ts` (new) | `deriveWhole`: bounds on the CPU over fixed times, a perspective camera framing them |
| `src/cli/load.ts` | `openScene` keeps the styled scene and its `compileScene` (whose `.artefact` is what `compile` returned) |
| `src/cli/main.ts` | budget check between compile and GPU load; `cameraOf` (whole or resolve); `writeOut` |
| `src/cli/cameras.ts` | an absent camera is a `UsageError` (exit 2), as README's exit table always said; it was exit 1 `internal` |
| `src/host/host.ts` | pipelines created concurrently in one error scope (`scopedAll`), not awaited one by one |
| `tests/cli.host.test.ts` + `tests/fixtures/host/` | 8 tests (below) |
| `tools/render_stages.ts` | the stage timer used for the table below |
| `README.md`, `docs/errors.md` | the new commands and refusals |

No new dependency, no new export from `src/index.ts`, and `loft context` is byte-identical (sha256 `f34053e5…` before
and after).

### How `whole` is derived, and its limits

- **Measured:** each thing's bakeable surfaces, sampled on a 16 x 8 grid (the same sampling `loft bake` does at higher
  density), plus field meshes' vertices and every shadow chain (joint ± radius). The shadow chains are how traced
  fields and `draw.copies` are known on the CPU. The scene's own sky, ground, shadow and light passes are skipped.
- **Times:** the bounds cover 9 fixed times: one period of a looping scene, else 0 to 4 s. They never use the times
  requested, so the camera is identical for every call on one scene at one aspect ratio. The training contract's
  one-call-per-time alternative therefore gets no camera motion.
- **Camera:** perspective, `fovY 0.6`, looking at the bounds' centre from the front-right and above (direction
  `(0.6, 0.35, 0.8)`, as in the docs' own `whole` cameras). The distance puts the bounding sphere at 1/1.15 of the
  narrower half-angle (fitted to `--size`'s aspect).
- **Not measured:** things with no shape on the CPU (glows, rain, soft media, lightning) are named on stderr as not
  measured. With nothing measurable at all, the command refuses (exit 4): `camera "whole": given a scene with no camera
  "whole" and no thing whose shape Loft can measure on the CPU (spark); ... Fix: add cameras: { whole: ... }`.
  A built directory (`--artefact`) or `--fixture` without `whole` gets exit 2: it has no scene to measure.
- **Cost:** 0.15 to 0.41 s of CPU (crab 0.19 s, serpent 0.41 s), paid only when the scene declares no `whole`.
- **Never** substitutes another named camera.
- **Weakness:** a scene with one huge thing far from the subject (a distant backdrop strand) is framed whole, so the
  subject comes out small. The fix in that case is the scene's own `whole` camera, which `loft context` already asks for.

## What I chose not to do

- **No batch command (item 4).** The measurement below shows the per-scene cost is compiling that scene's new pipelines
  (about 90% of the wall time), which a long-lived process does not avoid: each model-written scene has new WGSL. One
  process on one device for 8 novel scenes took 38.6 s, against 34.2 s for 8 separate processes. Batching pays only
  when scenes share shaders. On variants that changed constants in a few shaders it was ~0.53 s against ~0.8 s per
  scene, which is not the training case.
- **Instead:** `host.ts` now asks for every pipeline at once, and Dawn compiles them in parallel. This is not a new
  name or flag; the pixels are byte-identical and every caller gets it.
- **No `--budget` on a built directory:** it refuses with exit 2. The counts are in its `manifest.json` under `stats`.
- **No change to `strand` defaults (item 3, owner's call):** see the recommendation.
- `profile` and `variants` do not derive `whole`; they keep `resolveCamera`.
- `whole` is derived after the GPU load, not before it. A scene with nothing to measure is refused after its pipelines
  are built (seconds wasted on a rare failure). Moving it before the load is a small follow-up if wanted.

## Tests

- **Before**, at `a2d82a2`: `bun test --timeout 120000` gave **215 pass, 0 fail** (197.8 s). `pnpm test` does not run
  on this machine: pnpm reports `packages field missing or empty` for `pnpm-workspace.yaml`, on `main` as well, so I ran
  its script directly. That run shared the worktree with my first edits (`args.ts`) part-way through. Clean before-state
  pixels and timings come from a detached checkout of `a2d82a2` (`~/git/_wt/loft-base-a2d82a2`).
- **After**, on the branch: `bun test --timeout 120000` gave **223 pass, 0 fail** (215 + 8 new; 105 s). The whole suite
  is faster mostly because of the concurrent pipelines, though machine load differed between the two runs.
  `tsc --noEmit` (`./node_modules/.bin/tsc`) is clean, and `bun tools/check_loader_closure.ts` passes: no compiler
  module in the file loader's closure. `bun tools/check_snippets.ts` cannot run here, on the branch or at `a2d82a2`,
  because it calls `pnpm exec tsc`, which hits the same pnpm workspace error.
- **New tests** (`tests/cli.host.test.ts`, each runs the real CLI in a fresh process):
  1. `--out dir/` and `--out f.webp` parse; `.png` refused naming both forms.
  2. `--budget` parses either limit alone; a bad one is refused with an example.
  3. `frames --out dir/` writes one PNG per time at the size asked. Each decodes to exactly the RGBA whose sha256
     the CLI prints for that frame.
  4. A scene with no `whole` camera (`no_whole.ts`, which has a different camera `tip`): exit 0, stderr says
     derived, both strands are inside the bounds over the fixed times, and the picture's centre is not ground.
  5. A scene with its own `whole`: nothing is derived.
  6. Nothing measurable and no `whole`: exit 4 with the exact refusal and fix.
  7. Over budget: exit 4 before the GPU (no `load` line printed), with both counts, both limits, the heaviest things
     and both fixes. Under budget, it draws.
  8. Absent camera: exit 2, `loft: usage: no camera "nope": ...`.
- **Byte-identical default outputs**, base checkout against branch: `render` (default 4 views, 640x400) and `frames`
  (4 times, 480x300) of `shell`, `brig`, `pearls`, `coral`, `serpent`, `storm`. **All 12 WebP files are identical**
  (`cmp`), and so are all 48 per-frame RGBA hashes.
- **Concurrent pipelines under stress** (`tools/render_loop.ts`, as `docs/reliable_render.md` did, at load average
  10 to 30 from other agents): reefback crab 4x3 with the collector forced, **0 of 12 failed**; shell 4x12 forced,
  **0 of 48**; reefback crab 6x8, **0 of 48**. No load retries in any of them.

## Where the time goes (item 4)

M-series Mac, Metal through bun-webgpu/Dawn, 512x512, `frames` at 4 times. In-process stage times are from
`bun tools/render_stages.ts <scene> [--again]`, in ms. Bun's own start is about 10 ms (`bun -e 0`), and importing the
library about 25 to 35 ms. The training host's TypeScript screener adds 0.25 s to import `typescript.js`, outside LOFT.

| Stage (ms) | shell (27.7k tri, 4 draws) | serpent (57k, 26 draws) | maelstrom (191k, 100 draws) | novel 20-strand scene (19.8k, 43 draws), cold |
|---|---|---|---|---|
| import library | 29.5 | 26.6 | 33.4 | ~25 |
| import scene module | 2.7 | 15.6 | 162 | ~5 |
| compile (CPU) | 7.7 | 264 | 2,195 | 150 |
| device | 36.5 | 39.3 | 40.0 | ~37 |
| WGSL modules | 6.0 | 110 | 479 | ~100 |
| pipelines (before: one at a time) | 39.7 | 132 | 4,218 | **3,720 to 3,970** (load total) |
| 4 frames: pose + encode + GPU + readback | 18.8 | 19.3 | 37.8 | ~18 |
| PNG encode of sheet + `cwebp` | 81 | 88 | 142 | ~55 |
| in-process total | 273 | 794 | 7,556 | ~4,200 |

shell and serpent were probably already in Metal's on-disk shader cache (other agents render them often): a second
process on a scene it has seen builds its pipelines in about 150 ms, not seconds. Model-written scenes are never in
that cache. The last column is the training case: 20 strands with random motion expressions, so every strand's shader
is new. In it the GPU's draw work is under 0.5% of the time, and pipeline compilation is about 90%.

### The speed-up, 8 fresh novel scenes each, `frames` at 4 times, 512x512, separate processes

| | before (`a2d82a2`) | after (branch) | speed-up |
|---|---|---|---|
| sequential, one process per scene | 3.64 to 4.13 s each, **30.9 s** total; load 3.36 to 3.83 s | 2.09 to 3.06 s each, **18.9 s**; load 1.81 to 2.62 s | 1.64x |
| 4 processes at once (as training runs them) | **26.0 s**; load **10.1 to 14.0 s** per scene | **14.9 s**; load 6.0 to 7.3 s | 1.74x |

The second row probably explains the 12 to 21 s per process seen in training. Parallel renders queue on the GPU
stack's shader compiler, so each one's load grows from about 3.5 s alone to 10 to 14 s. That would also explain why a
rented NVIDIA GPU was not much faster: the GPU was never the limit. Each scene had new shaders, so no cache helped
either side. Draw time was unchanged at about 18 ms for 4 frames.

What would cut further, not done: fewer distinct pipelines per scene. Each strand gets its own shader because its
`spine` is compiled into WGSL, at about 90 ms of compile per strand per process. Raising the budget's draws has a
real cost here, and triangles do not (next section).

## Defaults and limits (item 3: reported, not changed)

`strand` defaults (`src/geom/strand.ts:148-149`): `stations: 32`, `detail: { along: 96, around: 24 }`.

| | declared triangles | draws |
|---|---|---|
| empty scene (sky, ground, shadows) | 34 | 3 |
| one default strand | 10,208 (surface 4,608 + outline 4,608 + shadow 992) | +2 |
| 11 default strands | ~112,300 | 25 |
| **12 default strands** | **122,498: over 120,000** | 27 |
| 20 default strands | 204,162 | 43 |
| crab-like scene, 22 default strands (`out/timing/crab_default.ts`) | 234,786 | 49 |
| one strand at `stations 8, detail 16x12` (what the old hand-written prompt asked for) | 960 | +2 |
| one strand at `detail 48x16` | 4,032 | +2 |

**12 default strands is the first count over 120,000 triangles**, while 96 draws allows 46 strands. Under the current
limits, a model that follows `loft context` (which shows `strand` without `stations` or `detail`) fails the triangle
budget on any creature with more than 11 parts. The context gives no triangle cost, so the model cannot know this
before it is refused. The refusal now says what to lower.

These are declared counts, not drawn ones. The 234,786-triangle crab drew 80,226 at 512x512 (level of detail picks
coarser grids), in about 6 ms of GPU time per frame. Triangles cost almost nothing at 512x512. Each thing costs one
shader to compile, so the compile cost tracks the number of things, which the draws count measures.

**Recommendation: keep the defaults. Raise the training limit to 250,000 declared triangles and keep 96 draws.**
250,000 fits 24 default strands (a full crab at 22) with no change to any existing scene's pixels. Draws are the limit
that tracks cost: about 90 ms of cold compile per strand per process. 96 draws is about 46 strands, roughly 4 s of
compile alone, so I would not raise it, and lowering it to about 64 is the lever if renders are still too slow.
Lowering the defaults (for example to `detail 48x16`, 4,032 per strand, 29 strands under 120,000) also works, but it
changes every existing scene's pixels and is a decision for the owner.

## What a model following `loft context` would get wrong

1. **Triangle budget at defaults:** above. With 12 or more parts it is refused, and the context says nothing of cost.
2. **`strand` without `paint`** crashes as `loft: internal: TypeError: Spread syntax requires ...iterable not be null
   or undefined` (exit 1) at `src/geom/strand.ts:147`, not as a refusal that names the fix. The context's API map
   writes `paint` without `?`, so it is required, but nothing says so when it is missing. Not fixed here: outside the
   four items, and a one-line `SceneError` in `strand` if wanted.
3. **An absent camera** was exit 1 `internal`, though README promises exit 2; **fixed** here. A model asked for
   `--view whole` on a scene without one now gets a derived camera instead of exit 1.
4. **`whole` framing** depends on the model. `loft context` asks for "the whole with room round it", and a model that
   writes a tight `whole` gets exactly that; LOFT uses a declared camera as written.

## Commits

Listed in the answer to the request; `git log a2d82a2..training-host-contract`.

## Follow-up: two refusals moved earlier

Asked for after the report above. Two changes, each with a test. No default, limit or existing picture changes.

1. **`strand` without `paint`** is now a scene refusal (exit 4), raised by `strand()` where the scene wrote it
   (`src/geom/strand.ts`):
   `loft: scene: strand "bare": given paint undefined; expected paint: a colour [r, g, b] in 0..1, or a function of
   the body coordinates returning vec3. Fix: paint: [0.8, 0.4, 0.3] (at tests/fixtures/host/no_paint.ts:7)`.
   Before, it was the spread's TypeError: exit 3 (`load: ... Spread syntax requires ...iterable`) on the new fixture, or
   exit 1 (`internal`) on the path the report above hit. Only values that spread cannot take are refused (absent, null,
   not iterable), so every scene that drew before draws the same.
2. **`--view whole` is resolved before the GPU load.** `open` in `src/cli/main.ts` now resolves every requested view's
   camera between the budget check and `loadedWithRetries`, so an absent camera (exit 2) and a `whole` with nothing to
   measure (exit 4) are refused before any pipeline is built. `examples/pearls.ts` is a real case: it used to print
   `load 90 ms` and then refuse; now it refuses after `compile`. The derived-camera note on stderr now comes before the
   `load` line instead of after it. A built directory (`--artefact`) still resolves after its load, because its loader
   reads and loads in one step; its refusal is a cheap usage error.

Verification, at `dea7ed4` (changes stashed) and after:

- `bun test --timeout 120000`: before **223 pass, 0 fail** (79.1 s); after **224 pass, 0 fail** (78.2 s). The new test
  is `a strand without paint ...`; the `whole` refusal test now also asserts no `load` line. Both fail on the old
  source (exit 3 instead of 4; `load` printed). `tsc --noEmit` and `tools/check_loader_closure.ts` pass.
- `render` (default views), `frames` (4 times, 480x300) and `frames --view whole` (2 times, 256x256, PNGs) of shell,
  brig, pearls, coral, serpent and storm: all 22 picture files are byte-identical (`cmp`), and so are all 58
  per-frame RGBA hashes and the derived-camera notes. `loft context` is byte-identical (sha256 `f34053e5…`).
  Script: `out/followup/snap.sh` (git-ignored).
