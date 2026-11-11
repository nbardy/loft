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
