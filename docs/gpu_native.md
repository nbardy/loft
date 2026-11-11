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

**The table contract.** `TableSource` gains a program, a sum type:

```ts
type TableProgram =
  | { kind: "map"; lanes: (i: F) => readonly Expr<"vec4">[] }     // entry i depends only on i, the frame and other tables
  | { kind: "scan"; init: State; step: (s: State, i: F) => { next: State; lanes: readonly Expr<"vec4">[] } }
  | { kind: "cpu"; fill: TableFill; why: string }                // not ported yet: named, counted, visible
```

- `map` and `scan` are written in Loft's own expressions (which already have loops and table reads and already
  compile to both JavaScript and WGSL). Their CPU fill is derived, not hand-written.
- `cpu` is the explicit unported state. `loft profile` lists every `cpu` table with its cost. It is not a fallback
  chosen silently.

**Scheduling.** Tables are sorted by what they read into levels. A level is one compute dispatch: a map table gets
one invocation per entry, a scan table one invocation that loops. Later levels read what earlier ones wrote.

**The host.** Per frame: write inputs, params and curves (as now); fill any `cpu` tables and upload them; run the
pose stages; draw. The dynamic buffers are bound writable to the pose stages and read-only to draws.

**What the client knows without reading pose.** Where a thing is in the world is a game input. How far it can reach
from there is a number fixed at build (its envelope). Level of detail and culling need only those two, so they do
not need pose on the CPU.

**Tools keep a CPU reading.** `Artefact.pose(state)` still returns every table, derived from the same programs. It
is for checks, baking and tests. A client never calls it once no `cpu` table is left.

PREDICTED, crab scene (measured today: 3.85 ms CPU pose and upload, 2.65 ms of it table fills, 0.80 ms of that
shadows): CPU pose under 0.4 ms; the pose stages under 0.1 ms of GPU time; pictures the same to within f32 rounding
(under 0.1% of pixels differing by more than 8/255).

Wrong if: the pose stages cost more than 0.3 ms on this machine, or more than a handful of tables cannot be written
as a map or a scan.

## Phase 2: the artefact as files, and a Rust loader

`loft build <scene> --out dir` writes a manifest (inputs, params, cameras, draws, pose stages, table layout), the
shader modules and the static tables as binary. A loader creates pipelines and runs frames. Two loaders: the existing
host, changed to read the files; and a small Rust crate on wgpu for the game.

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
