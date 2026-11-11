# GPU-native Loft: the design (2026-11-11)

Status: design, being built in phases. Decided with the owner: not three.js; Loft's own runtime, moved onto the GPU,
with asset compilation that loses neither formula motion nor exact paint.

## The one idea

Loft already separates what a thing is from how it is drawn. This extends the same split to everything else that
happens per frame or per build:

> Every quantity is a function. The author writes the function. The compiler decides where and when it is
> evaluated: once at build, once per frame on the GPU, per vertex, per pixel, or read from a table it built. The
> choice never changes the answer beyond a stated tolerance, and it is never forced by a file format.

Three consequences.

1. **Pose is a program on tables, not JavaScript.** A per-frame table is described as a map (each entry a function
   of its index) or a scan (entries in order, carrying state). From that one description the compiler derives the
   compute shader that fills it at run time and the CPU function that tools use. No table is described twice.
2. **The asset is the program.** A built artefact is files: shader code, tables, a draw list, compute stages.
   Formula motion survives as a compute stage and exact paint as a fragment function, because the asset carries
   code. Nothing in it is JavaScript, so a client in any language can load it.
3. **Baking is an optimisation with a certificate.** Any function may be replaced by a table or a texture when that
   is cheaper. Linear interpolation of a table with spacing h is wrong by at most `h^2 max|f''| / 8`, so the compiler
   can pick h for a tolerance, or keep the formula. Standard files (glTF) are a separate, lossy export for other
   tools.

## Phase 1: pose on the GPU

**The table contract.** `TableSource` gains a program, a sum type (as built, `src/expr.ts`):

```ts
type TableProgram =
  | { kind: "map"; runs: readonly { count: number; lanes: (i: F) => readonly Expr<"vec4">[] }[] }  // entry i: i, the frame, other tables
  | { kind: "scan"; first: number; steps: number; init: State; step: (s: State, i: F) => { next: State; lanes: readonly Expr<"vec4">[] } }
  | { kind: "cpu"; why: string; prepare(p: TablePrep): TableFill }                                // not ported yet: named, counted, visible
```

A map is a list of runs laid end to end, each a closed form in its own index (`mapOf(n, lanes)` is one run;
`listOf(entries)` is runs of one). Changed from the first draft because about half the tables the code fills are
lists of unrelated expressions (each rope's two ends, each listed pose, each shadow joint), which a single closed form
in i could only write as a chain of selects that evaluates every entry in every invocation. A table with no program
(`prepare` alone, as in `examples/bosses/tithe/kit.ts`) is read as `cpu` at the compiler's boundary. Static tables
(filled once at build) keep `prepare`.

- `map` and `scan` are written in Loft's own expressions (which already have loops and table reads and already
  compile to both JavaScript and WGSL). Their CPU fill is derived, not hand-written.
- `cpu` is the explicit unported state. `loft profile` lists every `cpu` table with its cost. It is not a fallback
  chosen silently.

**Scheduling.** Tables are sorted by what they read into levels. A level is one compute dispatch: a map table gets
one invocation per entry, a scan table one invocation that loops. Later levels read what earlier ones wrote. A `cpu`
table cannot wait for the GPU, so a map or scan that a `cpu` table reads is filled on the CPU too, and is listed as
`cpu` with why "read by ...".

Two things the first draft did not foresee, both from measuring the crab scene (`src/compile/tables.ts`):

- *Divergence.* Invocations of one SIMD group (32 on Apple GPUs) that take different branches run one after another.
  Tables of a level, and runs of a list, are different branches; laid side by side, a group ran up to 32 scans or list
  entries in turn. Each table, and each run of a table of several runs, now starts a SIMD group of its own.
- *Shared work.* On the CPU one compiled function shared subexpressions across a table's entries; on the GPU each run
  is a branch, so every rope end recomputed the ship's sea-following frame (2 MB of WGSL for the crab scene). Nodes
  that two or more runs read are hoisted into a table of their own (`<name>_common`, one invocation, a lane per value,
  one level earlier). And a list of joints no longer evaluates the sea under each joint in its own branch: the joints
  are a list, the sea under them a map.

**The host.** Per frame: write inputs, params and curves (as now); fill any `cpu` tables and upload them; run the
pose stages; draw. The dynamic buffers are bound writable to the pose stages and read-only to draws.

**What the client knows without reading pose.** Where a thing is in the world is a game input. How far it can reach
from there is a number fixed at build (its envelope). Level of detail and culling need only those two, so they do
not need pose on the CPU.

*Not met in phase 1.* The level of detail as built (`Detail` "grid", `src/host/detail.ts`) reads one vec4 per grid
instance each frame, `bound_<name>`: the surface's anchor (the grid's centre on the posed surface, or a copy's origin)
and the copy's scale. Those anchors ride the pose (a jaw's teeth on a head on a neck), and Loft has no notion of a
thing's input-driven root to measure an envelope from, so the rule cannot be applied without one. Phase 1 makes the
bound tables ordinary maps filled by the pose stages and reads them back: `frame` (headless) waits for this frame's
bounds after the pose submit; `draw` (interactive) never waits and plans with the last bounds read (one or two frames
old; the first frame draws every grid at its finest). No CPU pose is involved, but it is a GPU-to-CPU read per
frame. Phase 4 (level of detail written as indirect draw arguments by a compute stage) removes it.

**Tools keep a CPU reading.** `Artefact.pose(state)` still returns every table, derived from the same programs. It
is for checks, baking and tests. A client never calls it once no `cpu` table is left.

PREDICTED, crab scene (measured today: 3.85 ms CPU pose and upload, 2.65 ms of it table fills, 0.80 ms of that
shadows): CPU pose under 0.4 ms; the pose stages under 0.1 ms of GPU time; pictures the same to within f32 rounding
(under 0.1% of pixels differing by more than 8/255).

Wrong if: the pose stages cost more than 0.3 ms on this machine, or more than a handful of tables cannot be written
as a map or a scan.

MEASURED (2026-11-11, phase 1 as built; M-series GPU shared with other workers, load average 10 to 40; readings
swing 2x and more). CPU pose: `FrameTimings.pose` (frame state, `cpu` tables, uploads, the pose submit and the
level-of-detail plan), best of 40 frames per round, three rounds alternating the baseline commit (fbe6242) and this
one. GPU pose: `tools/pose_time.ts` (40 repetitions in one timed pass after a warm-up, least of 8), three rounds.
Draw calls are those the frame issued, identical before and after in every scene.

| Scene (view) | CPU pose before, ms | CPU pose after, ms | GPU pose stages, ms (levels) | Tables map / scan / cpu | Draw calls |
|---|---|---|---|---|---|
| serpent (three) | 0.83-1.39 | 0.20-0.86 | 0.14-0.17 (10) | 26 / 9 / 0 | 34 |
| brig (three) | 0.56-1.53 | 0.18-0.69 | 0.12 (5) | 48 / 13 / 0 | 48 |
| the_strike (game) | 2.99-6.35 | 0.27-0.50 | 0.16-0.24 (10) | 105 / 33 / 0 | 82 |
| maelstrom_wyrm (game) | 2.02-3.51 | 0.28-0.43 | 0.16-0.25 (8) | 54 / 17 / 0 | 76 |
| reefback_titan, the crab (game) | 1.92-4.00 | 0.31-0.57 | 0.07-0.27 (8) | 119 / 40 / 0 | 109 |
| thundermaw (game) | 1.55-2.49 | 0.17-0.44 | 0.15-0.17 (10) | 51 / 14 / 0 | 58 |
| tithe_fleet (game) | 0.76-1.57 | 0.53-0.76 | 0.13 (5) | 47 / 6 / 6 | 52 |
| wreckers_lamp (game) | 0.68-1.58 | 0.17-0.40 | 0.14 (6) | 50 / 17 / 0 | 57 |

Against the prediction for the crab: CPU pose 0.31 ms at best (under 0.4; medians 0.47-0.80 under load), pose stages
0.07-0.27 ms (the prediction of under 0.1 ms holds only at the best reading; the failure line of 0.3 ms was not
crossed in any warmed reading; an unwarmed reading reached 1.2 ms), pictures: 0.063% of pixels differ by more than
8/255 (all eight scenes under 0.1%; the baseline renders byte-identically run to run, so this is f32 against f64: band
edges keyed to the rest arc length move by under a pixel). One table pattern of the eight scenes is not a map or a
scan: the tithe fleet's six `pieces_*` tables, hand-written in the example (not edited in phase 1). The serial scans
are short (at most 48 steps); the crab's cost is code volume in its first levels (about 900 KB of WGSL each).

Costs moved elsewhere: the stage modules add WGSL (crab 1.4 to 4.7 MB, strike 1.3 to 7.5 MB), so a host's load is
slower (crab 2.3 to 2.9 s, strike 1.3 to 3.2 s, serpent 0.35 to 1.0 s), while compiling is faster (CPU fills are
compiled only when read: crab 3.5 to 2.1 s, strike 5.8 to 2.6 s). A headless frame waits for its level-of-detail
bounds (one extra submit and map).

## Phase 2: the artefact as files, and a Rust loader

`loft build <scene> --out dir` writes a manifest, the WGSL and the static tables as binary. A loader creates pipelines
and runs frames. Two loaders: the existing host, changed to read the files; and a small Rust crate on wgpu for the
game (not written yet; see the end of this section). The format is `docs/artefact_format.md`.

### The design (written before building, 2026-11-11)

What is special here: the asset is a program, every per-frame quantity is already a GPU program (phase 1), and a
frame is a pure function of (time, inputs, params, camera, view size, curves), with no state carried between frames
except the host's level-of-detail hysteresis. So the file does not need a scene graph, meshes, materials or
animation channels (glTF's model), and it need not describe "a scene" at all. It describes **what the host does**.

1. **The file is the lowered host (mine).** The phase-1 host made decisions at load from the `Artefact`: the blend
   factors behind `"alpha"`, plain or split passes, which shader is compiled with which host WGSL in front of it,
   pipeline deduplication, buffer sizes and usages, the bind-group layouts. A second loader would have to copy those
   decisions as code. Instead they are made once, by a pure function `lower(artefact)`, and written as WebGPU
   descriptors: buffers, textures, samplers, bind-group layouts, pipeline layouts, modules, pipelines, bind groups,
   and a frame recipe (writes, one compute pass, render passes with attachments and commands). The TypeScript host
   becomes an executor of that value, and runs from it whether the value came from a compile or from files: one
   host, not two, so the pictures cannot drift between the paths. Descriptor fields are WebGPU's own names
   (inherited: the spec's dictionaries, which wgpu mirrors), so a loader maps them one to one.
2. **A module is a list of content-addressed chunks (inherited idea: content addressing; its use here is mine).**
   The host prepends `PRELUDE_WGSL`, a variant's WGSL and the scene's shared WGSL to every module; written whole,
   that text would be stored once per module. A module is the concatenation of its chunks, in order, with nothing
   between them; a chunk is stored once, named by a hash of its text.
3. **The per-frame CPU work is a closed list of named writes (mine).** Frame uniform (from the camera), inputs,
   params, the level-of-detail slots. Each is a variant of `FrameWrite` with its algorithm in the format document.
   Everything else per frame is a GPU command. The level-of-detail plan is the one per-frame algorithm on the CPU;
   it is data (`Detail` per draw, measured at build) plus a documented procedure, and it reads bounds the pose
   stages wrote, through a readback listed in the manifest. Phase 4 moves it into a compute stage.
4. **What cannot be data is refused by name.** A `cpu` table is JavaScript. `loft build` refuses a scene that has
   any, listing them; the manifest's table list still records every table's fill (`map`, `scan`), and the build and
   `loft render --artefact` print the count of each. The executor takes the `cpu` fills as a separate argument, a
   sum type (`none` from files, `js` from a compile), so nothing in the file pretends to hold them.
5. **Versioned from day one.** `format: "loft-artefact", version: 1`. The preludes are chunks in the file, so a
   loader carries no copy of them.

### Inventory: what an `Artefact` and the host's frame need at run time

| Item | Today | In the file artefact |
|---|---|---|
| `shaders`, `sceneWgsl`, stage WGSL, preludes | strings, assembled by the host at load | chunks; modules list them |
| draws: blend, cull, depth, constants, pass | enums the host translates | full render pipeline descriptors |
| plain / split schedule, depth resolve, post ping-pong | host code | textures, bind groups and render passes |
| `inputs`, `params` (defaults, ranges), `inputFloats` | data | data, plus the `inputs`/`params` writes; the `time` input is filled from the view's time (a named rule) |
| `curves` | Float32Array | binary blob, uploaded at load; `setCurves` stays a loader call (no compiler needed) |
| `statics` | Float32Array | binary blobs, uploaded at load |
| `cameras` | data | data; camera to matrices is loader code (`src/host/camera.ts`, documented); the CLI's fallback cameras are a CLI facility |
| `Detail` per draw (grid levels measured at rest by a CPU pose at build) | data | data, unchanged; the plan is a loader procedure |
| level-of-detail bounds (read back each frame) | host code | a readback buffer and the copies into it, listed |
| `tables` (where each lives, how filled) | data | data, for tools (`readTables`, counts) |
| `stats` | data | data |
| `cpuPose` | closure | not data: refused by name (0 tables in the six scenes below) |
| `pose` (CPU reading of every table) | closure | tool-only: checks, baking and tests; a client never needs it |
| `Compiled.frame`, `cpu`, `poseProfile`, `table` | closures | tool-only |
| `--style`, `--outline` | change the scene before compiling | tool-only (a different build) |

### Predictions

Bytes (from the compiled artefacts of 2026-11-11; WGSL is the unique chunks: shaders that a draw or post pass uses,
stage modules, the scene's shared WGSL, the preludes 1.4 + 0.7 + 1.4 KB and the depth resolve):

| Scene | WGSL, MB | of which pose stages | blobs, KB | manifest, KB | written whole instead |
|---|---|---|---|---|---|
| maelstrom_wyrm | 4.11 | 2.85 | 9.2 | 60-100 | +1.0 MB |
| reefback_titan | 4.01 | 2.80 | 9.9 | 80-130 | +1.1 MB |
| thundermaw | 3.68 | 2.47 | 7.2 | 60-100 | +0.7 MB |
| tithe_fleet | 4.11 | 2.16 | 0 | 60-100 | +0.7 MB |
| wreckers_lamp | 3.00 | 2.04 | 15.7 | 60-100 | +0.6 MB |
| grab | 2.58 | 1.70 | 0 | 40-70 | +0.4 MB |

So the WGSL is about 97% of the bytes, and chunking saves 15 to 25% against whole modules. Deduplicating further
(accessors, library functions shared between stage modules) is worth little: splitting maelstrom's 46 modules at
top-level declarations, 4.08 MB holds 4.02 MB of distinct text (1.4% repeated). The volume is single functions:
the largest is a 252 KB hoisted `_common` pose function and 130 to 190 KB surface functions. That is phase 1's
code generation (expression trees emitted without sharing across lanes), not a format question.

Load (phase-1 host, this machine under load, 640x400): compile 1.0 to 5.3 s, GPU load (modules and pipelines)
0.9 to 4 s, once 17 s. From files: reading and parsing about 4 MB, under 50 ms; then the same modules and pipelines,
so the same GPU load within the machine's noise. Files save the compile and nothing else; to load faster the WGSL
must shrink (above) or pipelines be cached.

Pictures: byte-identical. Three frames per scene (t = 1.0; t = 2.5; t = 2.5 with one input changed, or a param where
the scene has no input that shows) through the phase-1 host (baseline SHA-256 of the RGBA, recorded before any
change), through the new host from a compile, and through `loft render --artefact` from files. The modules are the
same text and the descriptors the same values, so any difference is a bug in the lowering, not rounding.

Wrong if: any frame's hash differs; a file loader's import closure reaches `src/compile` or `src/expr.ts`; files
load more than 100 ms slower than the GPU load from a compile.

### Measured (2026-11-11, as built; machine shared and heavily loaded)

`bun tools/files_proof.ts` (pixels, bytes) and `bun tools/files_proof.ts --time 3` (load times: files and scene paths
alternated, each a fresh process, three rounds; least and most).

Pictures: **identical**, as predicted. All 18 frames (six scenes, three frames each, 640x400) have the same SHA-256
of RGBA through the phase-1 host, the new host from a compile, and `files_frames` from files, in a process that
loaded no compiler module (Bun's module registry, checked at the end of the run).

| Scene | WGSL, MB (predicted) | blobs, B | manifest, KB (predicted) | saved against whole modules | read, ms | GPU load from files, s | compile, s | GPU load from a compile, s |
|---|---|---|---|---|---|---|---|---|
| maelstrom_wyrm | 4.11 (4.11) | 9,216 | 78 (60-100) | 1.01 MB, 19.7% | 18-26 | 4.6-10.5 | 7.7-13.3 | 6.2-20.0 |
| reefback_titan | 4.01 (4.01) | 9,936 | 120 (80-130) | 1.07 MB, 21.0% | 4-58 | 3.0-21.9 | 2.3-13.1 | 4.4-59.4 |
| thundermaw | 3.67 (3.68) | 7,168 | 78 (60-100) | 0.69 MB, 15.8% | 4-15 | 2.7-3.1 | 3.0-4.5 | 3.0-4.9 |
| tithe_fleet | 4.11 (4.11) | 0 | 71 (60-100) | 0.67 MB, 14.0% | 3-46 | 2.3-2.7 | 4.5-5.7 | 2.0-3.1 |
| wreckers_lamp | 3.00 (3.00) | 15,744 | 69 (60-100) | 0.59 MB, 16.4% | 6-11 | 1.8-2.2 | 1.5-1.6 | 1.6-2.4 |
| grab | 2.58 (2.58) | 0 | 54 (40-70) | 0.35 MB, 12.0% | 2-9 | 0.94-1.48 | 0.92-1.30 | 1.03-1.59 |

Against the predictions:

- Bytes: hit, every scene, to the hundredth of a MB (the prediction was a sum of the compiled texts, and the files are
  those texts). The savings of chunking came in at 12-21%, against 15-25% predicted: grab and the fleet, with fewer
  and larger modules, fall below the band. Measured before building, deduplicating repeated text inside the compiler's
  output would save 1.4%; the expectation from phase 1 that repeated accessors and table code dominate stage WGSL does
  not hold for these scenes. The volume is single large functions (phase 1's code generation).
- Reading the files: under 50 ms predicted; 2-26 ms in all rounds but two (46 and 58 ms, both in rounds where the
  same machine took 13 s to compile). Hit, with those two readings at the edge.
- GPU load from files against from a compile: predicted equal within 100 ms. Not resolvable on this machine today:
  one scene's load swung 4.4 to 59 s across rounds. The least readings, which load only adds to, differ by -1.6 to
  +0.3 s, files faster in the two largest scenes; no sign of a systematic cost on either side, which is what identical
  modules and descriptors imply. The prediction stands as reasoning, not as a measurement.
- What files save is the compile, 0.9 to 13 s here (1 to 5 s on a quieter machine, phase 1's figures), and nothing
  else, as predicted. Loading faster than that needs less WGSL or cached pipelines (below).

Failed once, not reproduced: a scene-path load of reefback (timing round, not the proof) failed in Dawn with "The stage
(Fragment) of the entry point "fs" isn't the expected one (Vertex)" for `surface.cutterDactyl`; the same load passed
in the proof run and in three rounds of the rerun. It matches the intermittent shader-module failure AGENTS.md records
for the phase-1 host; it was not investigated.

The no-compiler property is enforced three ways: `tools/check_loader_closure.ts` follows every runtime import from
the loader's entries (`src/cli/run_files.ts`, `src/host/files.ts`, `tools/files_frames.ts`: 14 files, packages
`bun-webgpu` and `node:*`) and fails on `src/compile/**`, `src/expr.ts`, `src/grad.ts`, `src/index.ts`, `examples/**`
or a dynamic import whose path is not a literal; `tests/files.test.ts` runs it, and runs it on `tools/pose_time.ts`
to show it can fail; and `loft render --artefact` checks Bun's module registry after drawing.

### What phases 3 and 4 need from this format

Phase 3 (textures and layers): `TextureDesc` gains a fixed size, mip levels and dimension (beside "view-sized"), and
an `init` per mip from blobs or from a compute stage run once at load; `LayoutEntry` gains storage textures and the
other view dimensions; a texture is bound by a view descriptor rather than its default view. A layer a style asks for is
then a texture, a bind-group entry and a chunk of WGSL: no new kind of thing for the loader. The version moves to 2.

Phase 4 (fewer draws, level of detail on the GPU): a `DrawCommand` variant `indirect` (buffer and offset: wgpu's
`draw_indirect`, or one `multi_draw_indirect` per pipeline), buffers with `INDIRECT` usage, and a second compute pass
after the pose that writes the arguments. Then `BoundsRead` is always `none`, the `grid` plan and the readback leave the
format, and the loader's one algorithm with them; `cellPixels` becomes a frame write. Shaders shared across scenes
fall out of content-addressed chunks: a shared directory of `wgsl/` whose names are hashes.

### The Rust loader (not written: a cargo build would cost gigabytes of a nearly full disk)

PREDICTED size: 900 to 1,100 lines of Rust on wgpu, by part, against the TypeScript that does the same:

| Part | TypeScript today | Rust, lines |
|---|---|---|
| manifest types (serde) | `manifest.ts` 171 | 220-260 (wgpu-types' serde names are WebGPU's, so most enums deserialize directly; the rest are small `match`es) |
| reading the directory | `files.ts` reading, 25 | 40 |
| creating the objects, view-sized textures and groups | `host.ts` `createObjects`, about 90 | 200-250 (descriptors spelled out, lifetimes of views) |
| a frame: writes, pose pass, bounds copy and map, passes | `host.ts` frame path, about 150 | 180-220 (async map through a callback and `poll`) |
| frame state and the frame uniform, camera matrices | `host.ts` 40 + `camera.ts` 110 | 130-150 |
| level-of-detail plan | `detail.ts` 216 | 170-200 |

No part interprets the scene: every number a pipeline needs is in the file. The plan (about a fifth of it) is the one
algorithm; phase 4 deletes it.

## Rates (2026-11-11)

A compiled scene was about 4 MB of WGSL and took 2 to 3 s to load. This section says why, from the graph, and what
lowering it by rate does.

### The idea

Every node has a rate, the slowest clock at which its value changes: build (a constant) < frame (time, inputs,
params, curve rows, a table entry at a fixed index) < copy < item (a table entry, a vertex, a pixel). A program runs
at the rate of its invocations: a map run of more than one entry, and every vertex or fragment function, faster than
a frame; a scan, a run of one entry (a list element) once per frame. A node slower than the program that reads it is
recomputed for nothing in every invocation, and in every program that reads it.

Lowering, per program (`src/compile/rates.ts`, classified once in `rates`):

1. **Fold build constants in f32.** Each node whose operands are all constants is replaced by its value, rounded to
   f32, computed by the operation the CPU backend already uses.
2. **Lift the per-frame frontier into frame values.** A per-frame node read by a node that stays is lifted when its
   program runs faster than a frame, or when two or more once-per-frame programs of the scene read it, and its
   sub-graph costs at least `LIFT_MIN` = 3 operations (a fetch replaces it: one storage read and an accessor's index
   arithmetic). Lifted values are evaluated by a frame-values table: one entry per value, all of them computed by one
   invocation that shares work between them (a new program kind, `values`), read back by a fetch at a constant index.
   Phase 1's `<name>_common` (work shared between the runs of one table) is the case "two programs of one table" and
   goes; the rule is now scene-wide.
3. **Where a value waits.** A value that reads a per-frame table must be computed after it. Values go to the
   frame-values table of depth d = 1 + the deepest table they read (0 if none), depth being measured on the tables'
   programs as authored. Why this cannot make a cycle: give a table of depth k the potential 2k + 1 and the
   frame-values table of depth d the potential 2d; every read goes from a higher potential to a lower one (a table
   that reads a value of depth d contained it, so its own depth is at least d; a value of depth d reads tables of depth
   at most d - 1).

What is inherited: constant folding, and rates themselves: the computation frequencies of Stanford's real-time shading
language (Proudfoot et al., 2001: constant, per primitive group, per vertex, per fragment) and Direct3D effects'
preshaders (uniform-only expressions taken out of a shader and run once per draw). What is mine: a frame's slow work
is itself a GPU program in the artefact (a pose stage writing a table), not CPU code beside it; one rule (frontier,
faster reader or two readers, cost) for shaders and pose programs alike, which subsumes `_common`; the scene-wide
sharing among once-per-frame programs; the depth assignment and its acyclicity argument.

### The census (before building)

`bun tools/rates_census.ts <scene>` walks every program the compiler lowers (each shader function and each run or
scan of a table, as authored: phase 1's `_common` tables are left out). Nodes are the statements `emitWgsl` writes
(everything but constants and variables), loop bodies included. "Build" are folded away; "frame" and "item" are by
rate (the remainder, a few thousand per scene, are loop bodies, not rated).

| Scene | nodes as authored | build | frame | item | phase 1 emitted (let lines) | frontier values | frame-values nodes | nodes after |
|---|---|---|---|---|---|---|---|---|
| maelstrom_wyrm | 122,015 | 39,939 | 53,285 | 25,930 | 91,781 | 310 | 4,719 | 42,971 |
| reefback_titan | 111,850 | 47,688 | 33,216 | 28,823 | 86,855 | 245 | 3,709 | 47,086 |
| thundermaw | 104,122 | 45,226 | 32,366 | 24,001 | 80,656 | 382 | 5,573 | 41,188 |
| tithe_fleet | 97,706 | 44,008 | 21,569 | 30,995 | 88,640 | 281 | 1,583 | 44,514 |
| wreckers_lamp | 86,266 | 47,239 | 15,852 | 20,816 | 66,701 | 125 | 1,104 | 30,415 |
| grab | 78,713 | 31,998 | 29,445 | 15,903 | 57,431 | 91 | 1,130 | 23,953 |

Where the volume is: build constants are 37 to 55% of the graph, the largest single cause. Per-frame work is 15 to 44%,
almost all of it in once-per-frame programs (reefback: 27,093 of 33,216), where it is not wasted GPU work but code
written several times: 1,415 distinct per-frame nodes are read by two or more such programs, 17,777 times. Per-frame
work inside faster programs, the waste the hypothesis named, is smaller: 1,906 to 8,942 nodes per scene.

The six large ship functions of reefback (2,216 to 2,945 nodes) are mostly build constants, not the ship's frame:
`bound_scaleShip_deck_0` has 2,835 build, 110 frame and no per-entry nodes; it is a run of one entry (a bound of one
instance), so it runs once per frame anyway. `body_scaleShip_railS_spine_0` (a map over the rail's stations) has
1,737 build, 88 frame, 399 per entry. The build part is the rest shape evaluated at fixed parameters (a spline's
weights at a fixed station). The ship's frame, 106 to 110 nodes, is the per-frame part each repeats; it goes to the
frame values once. So the hypothesis was right about the mechanism and wrong about the weight: folding removes
about four times what lifting does in these functions.

Per copy: shader functions with a copy argument (a surface's instance, `p_copy`, `copy_id`, `cell_id`, read as per
copy by the census only) hold 70 to 757 per-copy nodes of 20,000 to 44,000. Not worth a stage; not done.

### Predictions

Bytes are the unique WGSL chunks (as `loft build` prints, KB = 1024 bytes): the text that is not a node's `let` line
(preludes, raw WGSL of drawables, writes, declarations: measured per scene) plus 42.5 bytes per node (phase 1's mean
let line, 42.1 to 42.9 in all six scenes), plus a write per frame value. Range: ±8% on the node part (fetch lines are
longer, folded literals vary).

| Scene | WGSL before, KB | predicted after, KB | largest module before, KB | predicted largest after, KB | frame-values nodes |
|---|---|---|---|---|---|
| maelstrom_wyrm | 4,053 | 2,070 (1,930-2,210) | 906 (pose level 0) | 210-310 | 4,719 |
| reefback_titan | 4,074 | 2,410 (2,260-2,570) | 762 (pose level 1) | 245-430 | 3,709 |
| thundermaw | 3,607 | 2,000 (1,870-2,140) | 794 (pose level 0) | 200-305 | 5,573 |
| tithe_fleet | 4,019 | 2,170 (2,030-2,320) | 732 (pose level 3) | 360-520 | 1,583 |
| wreckers_lamp | 2,983 | 1,510 (1,410-1,610) | 806 (pose level 0) | 280-410 | 1,104 |
| grab | 2,526 | 1,160 (1,080-1,250) | 777 (pose level 0) | 225-310 | 1,130 |

The largest module after is a pose level: the low end is the largest level's nodes after lowering, at phase 1's
levels; the high end allows two of today's levels to land on one (tables that read a frame-values table move a level
later). The largest shader module (a ship's hull, about 200 KB in every scene) falls to about 155 KB.

Pose stage GPU time: **no material change**, least reading after within 0.75 to 1.35 times the least before, with 2 to
6 more levels. Reason: the time is the scans' serial steps (reefback's levels 1 to 5 hold its 33 scans and take most
of its 0.10 to 0.25 ms), which lowering does not touch; build constants were already free at run time (WGSL and Metal
fold a `let` of literals); what lifting removes from faster programs ran in parallel; what it adds is one invocation
per frame-values table (up to about 2,400 nodes, serially about 5 to 15 us) and the levels they insert.

Load (modules and pipelines from files, least of several rounds): taking 0.4 s as the cost that does not scale with
the text (about 100 pipelines), after = 0.4 + (before - 0.4) x (bytes after / bytes before). Least readings before
(this machine, load average 5 to 21): maelstrom 2.31, reefback 2.12, thundermaw 2.24, tithe 1.67, wreckers 1.44, grab
1.36 s; predicted 1.38, 1.42, 1.42, 1.09, 0.93, 0.84 s, ±25%.

Pictures: under 0.1% of pixels differ by more than 8/255 (f32 folding and moving evaluation between stages change
low bits only). The CPU reading still agrees with the GPU to 1e-4.

Wrong if: bytes after fall outside the ranges; the largest module is a shader, or a pose level above 520 KB; pose time
leaves 0.75 to 1.35x; load after is not below 0.8x before in least readings.

### The stronger formulation, measured

Programs that are the same graph up to constants (eight legs, 404 shadow points, 95 rope ends) could be emitted once
and driven by a table of their constants. From the census, after folding and lifting, the repeats beyond the first
hold (up to constants / up to constants, tables and curve rows): maelstrom 11,586 / 15,405 nodes, reefback 11,903 /
22,795, thundermaw 11,637 / 15,793, tithe 19,186 / 21,202, wreckers 8,997 / 10,758, grab 6,554 / 9,325: a further 25
to 50% of what remains. The largest single source is lists of isomorphic entries (`shadows_at`: 404 runs of 7 nodes
in reefback, each starting a SIMD group of its own: 16,096 invocations at its level). Turning such a list into one map
run over a table of constants removes the code and the idle invocations both. It should come next for the pose stages'
code, but not for frame time (below): the pose stages are 4 to 12% of a frame.

### Runtime per frame: the prediction (added before the after-numbers, 2026-11-11)

The owner re-aimed the work at per-frame cost while it was being built. The census weighted by invocations: the
per-frame nodes lifted out of draws are 106 to 133 per vertex in the ship's parts and sails (a few thousand vertices
each) and at most 41 per pixel (`colour_telegraph_*`, `colour_crash`, the fleet's wakes: small areas). Weighted
(`tools/rates_weigh.py`): 1.2 to 8.3 M operations per frame in vertices; in pixels at most 130 to 330 M if each such
draw covered the whole view, which they do not (a few percent of it). At the sea's measured rate (about 250 M
operations per ms, below) that is about 0.03 ms of vertex work and, at a few percent coverage, under 0.05 ms of pixel
work: under one tick of the GPU timer (0.0655 ms). (Correction, same day: the first draft of this paragraph gave the
per-pixel bound as 80 M, from one scene; the bound over all six is the one above.) Build constants
were already free at run time. So: **no measurable change** in draw time, pose time (within the band above) or CPU time.
The rate rule is worth its bytes, not its milliseconds, in these scenes.

### Measured (2026-11-11, merged with main c0959b4; before: main, after: this branch, both from built files)

Bytes and modules (`tools/wgsl_bytes.ts`), KB = 1024 bytes:

| Scene | WGSL before | after (predicted) | largest module after (predicted) | frame-values nodes (predicted) | pose levels |
|---|---|---|---|---|---|
| maelstrom_wyrm | 4,053 | 2,065 (1,930-2,210) hit | 262 (210-310) hit | 4,360 (4,719) | 8 -> 10 |
| reefback_titan | 4,076 | 2,524 (2,260-2,570) hit | 330 (245-430) hit | 3,961 (3,709) | 11 -> 17 |
| thundermaw | 3,607 | 1,967 (1,870-2,140) hit | 240 (200-305) hit | 4,776 (5,573) | 10 -> 13 |
| tithe_fleet | 4,019 | 2,136 (2,030-2,320) hit | 461 (360-520) hit | 2,360 (1,583) | 6 -> 6 |
| wreckers_lamp | 2,983 | 1,506 (1,410-1,610) hit | 244 (280-410) below | 1,290 (1,104) | 6 -> 6 |
| grab | 2,564 | 1,157 (1,080-1,250) hit | 181 (225-310) below | 1,185 (1,130) | 6 -> 6 |

Bytes: six of six. Largest module: four of six; in wreckers and grab no level after lowering holds as much as the
largest level of phase 1 did, because the frame-values depths split the levels the low end assumed whole. Frame-values
nodes: within 15% in five scenes, tithe 49% over: the census placed values by phase 1's levels (with `_common`), not
by the authored depth the compiler uses, and tithe's values split into two tables whose work overlaps. Extra levels:
0 to 6 (predicted 2 to 6; three scenes added none). Compile time fell too (reefback 3.0 to 1.4 s) once folding was
memoized per node; unmemoized it was 63 s, from refolding shared sub-graphs and rebuilding loops per program.

Per frame, 1280 x 800, game view, t = 1 (`tools/rates_bench.ts --profile`, three rounds alternating before and
after). "Normal frame" is the new `GpuLoaded.frameGpu`: timestamps at the start and end of the unmodified frame's pose
pass, and at the start of its first render pass and the end of its last, the two spans summed (the CPU's wait for the
level-of-detail bounds between the two submits is not counted). Timestamps on empty passes read garbage on this
adapter (Dawn on Metal), so they are written on the real passes. The timer's tick is 0.0655 ms. This GPU runs in two
clock states: in about one run in six every reading drops 2 to 2.5x together (reefback before, round 3: a 2.36 ms
frame and a 1.0 ms sea), so a least-of-rounds compares clock states, not code. The figures are the common state.

| Scene | normal frame before -> after, ms | its pose stages | CPU pose (least) | CPU encode (least) |
|---|---|---|---|---|
| maelstrom_wyrm | 6.23 -> 6.16 | 0.655-0.721 -> 0.655 | 0.34 -> 0.27 | 0.48 -> 0.46 |
| reefback_titan | 5.31 -> 5.37 (4.92 once) | 0.393 -> 0.459 | 0.29 -> 0.41 | 0.48 -> 0.57 |
| thundermaw | 5.31-5.37 -> 5.37 | 0.459 -> 0.524 | 0.22 -> 0.27 | 0.33 -> 0.37 |
| tithe_fleet | 5.83 -> 5.83-5.90 | 0.197 -> 0.197 | 0.38 -> 0.37 | 0.48 -> 0.42 |
| wreckers_lamp | 4.92-4.98 -> 4.98-5.05 | 0.197 -> 0.197 | 0.37 -> 0.43 | 0.49 -> 0.61 |
| grab | 3.87 -> 3.80-3.87 | 0.262 -> 0.197 | 0.45 -> 0.37 | 0.44 -> 0.50 |

The CPU readings (least of 30 frames per run; reefback and thundermaw lost one "before" run to a failed process) move
by up to 0.12 ms both ways with no sign common to the scenes; rounds of one scene spread by 0.1 to 0.4 ms. Lowering
adds no CPU work to a frame (frame values are pose stages; uploads and the plan are unchanged), so this is read as noise.

As predicted: no change beyond one tick anywhere. The pose stages move by one tick either way (reefback and
thundermaw one tick slower, with 6 and 3 more levels; grab one tick faster), inside the 0.75-1.35x band. Pictures:
the worst frame has 0.0063% of pixels over 8/255 (bar 0.1%; 18 frames: six scenes, the proof cases t = 1, t = 2.5 and t = 2.5 with an input or param changed); they are isolated pixels on paint bands and ink edges
(a fort's brick lines, coral tips) where an f32-folded threshold moved by a pixel. `tests/pose.gpu.test.ts` passes.

### What a frame is made of, and the sea

The normal frame is 3.8 to 6.2 ms GPU. Its pose stages are 0.2 to 0.7 ms (4 to 12%); the rest is the render passes.
Attribution from the per-stage profiling frame (which costs about 1.5x a normal frame, one pass per stage): the sea is
the largest draw in every scene, 1.86 to 3.95 ms; fixed stages (clears, depth and multisample resolves, the post pass)
about 1.0 ms; the sky 0.33 to 0.43; no creature draw reaches 1 ms.

The sea's fragment program, by rate (`tools/rates_census.ts --sea`; its `colour_sea` and the functions it calls):

| Scene | (a) per frame | (b) smooth in the water's point | (c) per pixel | library calls |
|---|---|---|---|---|
| maelstrom_wyrm (whirlpool) | 4 | 19 | 280 | contact band, footprint, noise x2, hash, fwidth x7 |
| the four others with swell | 3 | 27 | 108 | the same, fwidth x6 |
| grab | 2 | 19 | 46 | contact band, contact foam, footprint |

Priced by removing parts of the built WGSL (`tools/sea_variants.py`; reefback / maelstrom, least of three): as built
2.75 / 3.93 ms; contact band removed 1.44 / 2.43; its inner ring alone 0.07 / 0.20; a constant fragment (rasterising,
blending, multisampling: the floor) 0.46 / 0.48. So the depth taps of the contact band cost 1.31 / 1.50 ms, the style's
water and the swell 0.98 / 1.95, the floor about 0.47.

(a) is 3 or 4 nodes (time times each wave's frequency): nothing to lift. (b) is 19 to 27 nodes of 140 to 300, at most
about 0.13 ms, and it fails its certificate where it would pay: interpolated over the sea's cells (2.25 x 7 pixels),
the slope's error is h^2 A k^3 / 8, which, where the footprint fade begins (h about 0.6 of the shortest wavelength), is
about twice the slope itself. So the sea's cost is **(c), per pixel**, and lifting cannot cut it. What would:

1. The contact band (half the sea): 9 to 17 reads of the scene's depth, each unprojected with a 4x4 matrix, at every
   sea pixel, though almost all sea pixels are far from any body. A lower-rate pass: per 8 x 8 tile, whether any opaque
   surface lies within the band of the tile (one pass over the depth at 1/64 of the pixels), and the sea skipping the
   taps where none does. Expected: the band's 1.3 to 1.5 ms down to its share of the screen near bodies, plus about
   0.02 ms for the tile pass; about 1 ms per frame.
2. The style's water (0.98 to 1.95 ms): two value-noise evaluations and six or seven `fwidth`s per pixel, and
   maelstrom's whirlpool marks (280 per-pixel nodes). A texture of the noise (phase 3) would replace the noise.

## Phase 3: textures and layers

A texture is a table with two or three indices and mip levels. It is written as a function and baked by the
compiler, or loaded from an image. Layers a style can ask a surface for: ink index, lighting ramp, distance to the
nearest line (so lines stay crisp from a small texture), cavity, emissive, noise, flow, height. Paint gets a level of
detail: the formula up close, its baked texture far away, chosen by the pixel's footprint.

## Phase 4: fewer draws

One draw per kind of part per object, with the differences in per-instance rows (as the fleet's 22 draws for 4
ships). Shaders shared across scenes where only tables differ. Level of detail and culling written as indirect draw
arguments by a compute stage.

## Phase 5: export for other tools

`loft export gltf`: meshes, skins on the stations, sampled clips, baked textures. Lossy by nature and not used by
our clients.
