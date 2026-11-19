# Loft: a third-party code review

Reviewer: Claude Fable 5.2, acting as an outside engineer choosing a 3D library for a new project.
Date: 2026-11-19. Tree: `main` at `77e80b5`.
Rules followed: code only (`.ts`, `.rs`, `.wgsl`, build files, tests, examples). I opened no Markdown or text
document, and I did not read the other reviews in this folder. Section 12 says what I ran and what I did not check.

## 1. Verdict

I would not build a new general 3D project on Loft today. I would use it, with my eyes open, for one narrow kind
of project: batch generation of stylised, fully procedural stills and short clips, where every shape and motion is a
formula written in code (by a person or by a language model), and where repeatable pixels, measured checks and precise
error messages matter more than assets, interaction or load time.

The reasons, in order of weight:

1. **It is two engines in mid-migration.** The TypeScript compiler and host draw everything. A Rust rewrite exists
   beside it (19k lines) and cannot yet draw any scene the library produces: `loft build` under `LOFT_ENGINE=rust`
   prints "not drawable yet: 0 draws" (`src/cli/main.ts:248`).
2. **Load time grows with the number of things in the scene.** Each drawn thing gets its own generated shader module.
   I measured 0.4 s to load a lantern, 9 s for two ships and a serpent, and 55 s for five crabs (section 4).
3. **There is no way to bring anything in.** No mesh import, no image textures, no skinned characters, no picking.
4. **The core is welded to its content.** A scene must have a ground, a sun and a style; every lit surface shader
   calls `sea_height` (`src/draw/surface.ts:319`); a style author must write water, shadow, ground and sky functions.
5. **It is nine days old, has one author, no licence, no package, no CI, and an 800-name API.**

Against that, the core idea is good and the core engineering is better than most renderers I have read. Section 5
lists what I would be glad to inherit.

## 2. What it is and how it works

Loft's own `package.json` describes it as "3D bodies as coordinate systems, authored as code, compiled to WGSL +
tables for any WebGPU host". That is accurate. It is a compiler, not a scene graph.

**A scene is a TypeScript module.** Its default export is `scene({ ground, things, style, sun, cameras })`
(`src/scene.ts:15`, `:75`). The smallest example is 48 lines (`examples/first/lantern.ts`):

```ts
export const lantern = strand({
  name: "lantern",
  spine: (m) => vec3(0, add(0.04, mul(m, HEIGHT)), 0),
  girth: radius,
  tips: "open",
  paint: (m) => paper(m),
});
export default scene({ ground: plane, things: [lantern], style: toon(), sun: SUN, cameras });
```

Nothing here computes a number. `add`, `mul` and `vec3` build nodes of a typed expression graph
(`src/expr.ts:249`). Equal expressions are one shared node (`src/expr.ts:191-209`). The same graph is compiled twice:
to WGSL for the GPU (`emitWgsl`, `src/expr.ts:1043`) and to JavaScript for the CPU (`compileJs`, `src/expr.ts:1356`).
The CPU copy is what makes checks, baking and tests possible without a GPU.

**A shape is a function, and its signature says how it is drawn** (`src/map/types.ts`). There are thirteen ways to
draw, all members of one object, `draw` (`src/draw/index.ts:79-134`):

| Shape | Drawn as |
|---|---|
| `(u, v) -> point` surface | a grid evaluated in the vertex shader (`draw.surface`) |
| `point -> distance` field | sphere tracing per pixel inside a box (`draw.field`), or a baked distance brick, or a mesh extracted once |
| author's triangles | vertex pulling from tables (`draw.mesh`) |
| a `hit(ray)` function | per-pixel intersection (`draw.traced`, `draw.copies`) |
| a density | closed-form integration of Gaussian puffs (`draw.density`) or a ray march (`draw.volume`) |
| points, curves, relief, raw shaders | instanced quads, ribbons, shell marching, the author's own WGSL |

There are no vertex buffers anywhere. Every draw is "V vertices times I instances"; the vertex shader derives
everything from the vertex index and from tables (`src/artefact.ts:12-13`).

**`compile(scene)` returns an `Artefact`** (`src/compile/index.ts:47`, type at `src/artefact.ts:461`): WGSL modules, a
draw list, and "tables" (arrays of `vec4`) with a program for filling each one. Three things in the compiler are
worth naming:

- *Tables are filled on the GPU.* A table's program is a `map` or a `scan` written in expressions
  (`src/expr.ts:112-120`). The compiler sorts tables into levels by what they read and emits one compute dispatch per
  level (`src/compile/tables.ts:11-13`). The lantern needs five such stages; a serpent-and-ships scene needs thirteen.
- *Rates.* Each node is classified as changing at build, per frame, per copy or per vertex/pixel
  (`src/compile/rates.ts:19-20`). Per-frame work found inside a per-pixel program is lifted into a one-entry table
  computed once per frame (`src/compile/rates.ts:7-12`). This is automatic and I have not seen it in three.js or Bevy.
- *Packing.* WebGPU guarantees eight storage buffers per shader stage. Loft uses three for inputs, params and curves,
  and packs every table into the remaining five (`src/compile/tables.ts:15-19`, `:39-41`).

**`lower(artefact)` turns the artefact into a `Manifest`**: a plain-data list of every buffer, texture, bind group
layout, pipeline and render pass, described with WebGPU's own field names (`src/host/manifest.ts:185-248`). `loft
build` writes it as `manifest.json`, `wgsl/` and `bin/`.

**The host executes the manifest and decides nothing** (`src/host/host.ts:1-4`). It runs in the browser and in Bun
through `bun-webgpu` (`src/host/headless.ts`). A Rust crate, `loft-loader`, executes the same manifest on `wgpu`
(`rust/crates/loft-loader/src/lib.rs:1-12`), which is how a game written in Rust would embed a Loft scene.

**On top of the core sits a large content library**: creatures and seas (`src/geom`, 6,155 lines), boss-fight
choreography (`src/spacetime`, 2,464 lines: `attack`, `telegraph`, `routine`), clouds, glows and rain (`src/soft`),
seven styles (`src/style`: toon, woodblock, biolume, papercraft, sketch, neon, pbr), a constraint solver for motion
curves (`src/geom/solve.ts:689`), and checks that are proved over a whole time range by interval arithmetic
(`src/ranged.ts:121`, `src/interval.ts`).

**Who it is for.** The examples are 64% "bosses" by line count: game enemies with attacks, telegraphs and group
dances. The CLI has a JSON-lines `serve` mode with compile sandboxing, deadlines and budget exit codes
(`src/cli/serve.ts`), a `context` command that prints a prompt-sized API summary, and every refusal is written as
"given X; expected Y. Fix: Z" (`src/errors.ts:9-15`). I read this as a tool built for language models to author
stylised game creatures at volume, with a human art director comparing variants. A general 3D library it is not, and
it does not claim to be.

## 3. How big it is

Measured with `git ls-files` and `wc -l` on tracked files.

| Area | Files | Lines |
|---|---|---|
| `src/` TypeScript | 208 | 47,499 (31,678 code, 12,558 comment-only, 3,263 blank) |
| `examples/` | 125 | 16,477 |
| `tests/` | 129 | 9,193 |
| `tools/` (TS, shell, Python, one Rust file) | 84 | 8,574 |
| `rust/` (six crates, plus 13 WGSL templates) | 65 | 19,268 |
| `viewer/` | 2 | 303 |
| **Code total** | | **about 101,000** |
| Markdown (not read) | 123 | 2.3 MB |

Inside `src/`, by directory: geom 6,155; draw 4,736; host 4,226; cli 4,094; style 3,823; soft 2,518; spacetime 2,464;
text 2,312; compile 2,299; map 2,109; lib 1,432; record 1,206; bake 949; frame 593; relief 557; and 7,920 lines in 26
top-level files (`expr.ts` alone is 1,632).

My own split: the engine proper (expressions, gradients, intervals, map algebra, draw, compile, host, WGSL text
front end) is about 21,000 lines. The rest of `src/`, about 26,000 lines, is content, checks and CLI.

Rust, source only: loft-core 6,186; loft-loader 3,272; loft-ir 2,935; loft-gpu 2,089; loft-draw 708; loft-node 197;
tests 3,531.

Other measures:

- **Public API:** the single entry (`package.json` exports only `"."`) has 451 runtime exports (410 functions, 39
  objects) and about 350 exported types. I counted the runtime exports by importing `src/index.ts`.
- **Dependencies:** no runtime npm dependencies at all. Dev dependencies are TypeScript, `bun-webgpu` 0.1.7 (patched),
  Playwright and WebGPU types. The Rust lock file holds 151 packages; `wgpu` and `naga` are pinned to `=30.0.1`
  (`rust/Cargo.toml:15-17`).
- **History:** 781 commits from 2026-11-10 to 2026-11-19 under one author name; 423 of them in the last week. The
  first commit was 1,202 lines in 8 files, so the tree was not imported whole.

One hundred thousand lines in nine days is the most important number in this section. Everything else in the review
should be read with it in mind.

## 4. How fast it is

All numbers below are mine, measured today on an Apple M4 (Metal, through Dawn in `bun-webgpu`), at 1280 x 800,
with `loft render`, `loft show` and `loft profile`. Outputs went to a scratch directory outside the repository.

| Scene | Compile | Load | Shader modules | WGSL lines compiled | Draws | Triangles drawn | Frame, with readback |
|---|---|---|---|---|---|---|---|
| `examples/first/lantern.ts` | 41 ms | 397 ms | 9 | 2,074 | 5 | 10,210 | 15 ms |
| `examples/serpent.ts` | 257 ms | 389 ms | 30 | 28,899 | 26 | about 50,000 | 8 to 22 ms |
| `examples/materials/spheres.ts` | 119 ms | 741 ms | 19 | 21,262 | 25 | 109,474 | 8 to 19 ms |
| `examples/scenes/maelstrom.ts` | 1,107 ms | 8,964 ms | 74 | 120,920 | 100 | about 166,000 | 13 to 31 ms |
| `examples/bosses/crab_lineup.ts` | 4,566 ms | 54,861 ms | 305 | 1,130,448 | 658 | about 700,000 | 21 to 103 ms |

"WGSL lines compiled" is the sum over modules as the GPU receives them (shared prelude repeated in each), from `loft
show`. "Frame" is wall time for one headless frame including the pixel readback; the first view of each scene is the
slow end of the range.

`loft profile` separates GPU time from the rest. For the maelstrom scene a normal frame costs **2.62 ms of GPU time**
(0.52 ms for the thirteen compute stages, 2.10 ms for draws and passes). For the five-crab scene, with 658 draws
declared, it costs **4.26 ms** (0.79 ms and 3.47 ms).

What I conclude:

- **Steady-state GPU cost is low.** 166,000 triangles, a sea, shadows and outlines in 2.6 ms, and about 700,000
  triangles in 4.3 ms, leave room for 60 Hz on integrated graphics. The level-of-detail system works: the host reads each grid's bounds back from the GPU and
  picks a coarser grid per instance (`src/artefact.ts:288-306`).
- **Compile is fast.** About a second for a large scene, all on the CPU, with no GPU needed.
- **Load is the problem, and it scales with scene content.** A lantern is one strand on a plane and already produces
  2,074 lines of WGSL in nine modules. Five crabs produce 305 modules and over a million lines, and take 55 seconds
  before the first frame. The code knows this: "16 s of a cold load was this queue" (`src/host/host.ts:239`);
  "Pipeline creation is most of a cold load (2026-11-15: 46 of 73 pipelines, 8 of 12 s, were such surfaces)"
  (`src/host/share.ts:5-6`). The remedies in the tree are a text-level pass that merges shader modules equal up to
  their literals (`src/host/share.ts`), a module cache for the long-running server (`src/host/cache.ts`), and the Rust
  rewrite, whose design replaces per-thing modules with fixed templates plus data
  (`rust/crates/loft-core/src/draw/mod.rs:1-4`). Only the first two are in effect, and 55 s is the result with them.
- **Two structural costs remain in the draw path.** Grids are non-indexed: `6 * NU * NV` vertices
  (`src/draw/surface.ts:180`), so each grid point's position function runs about six times. And the scene is always
  drawn 4x multisampled into an 8-bit target (`src/host/manifest.ts:257`, `:263`), with up to six extra full-screen
  passes when any transparent draw reads depth (`src/host/lower.ts:6-20`).

**Build speed.** `tsc --noEmit` over the whole repository takes 0.92 s (TypeScript 7) and reports no errors. There is
no bundling step: Bun runs the `.ts` files directly. I did not build the Rust workspace. It has 151 locked packages
including `wgpu` and `naga`, so a cold build will take minutes; the workspace file documents a `lab` profile because
a one-line edit under release settings cost "64-71 CPU-s" (`rust/Cargo.toml:53-55`).

**Tests.** `bun test` ran 314 tests in 78 files in 174 s, with 269,842 assertions: 311 passed and 3 failed. All three
failures are in `tests/serve.rust.test.ts` and have one cause. The prebuilt native addon in this checkout's
`rust/target/lab/` reported "loft-node abi 2 wire 1" where the source expects "abi 5 wire 2"
(`src/core/native.ts:10`), and I did not rebuild it. So the TypeScript engine's suite was green on this machine. The
Rust-path tests neither build what they need nor skip. The stale binary was linked at 04:04 on the day of this
review, from a tree three interface revisions behind the one I read that afternoon.

## 5. What is solid

These are the parts I would be glad to inherit.

**The expression layer.** One typed graph, two backends, hash-consing, structural hashing, substitution, constant
folding in f32, symbolic gradients (`src/grad.ts`) and second derivatives. Overloads are written out so that
`mul(x, 2)` keeps the type of `x` (`src/expr.ts:548-560`). Arity mistakes are refused by name rather than silently
dropped (`src/expr.ts:610-630`). Tests compare the GPU result with the f64 CPU result to a 1e-4 relative tolerance
(`tests/expr.test.ts:93`).

**Schedules are separate from descriptions.** A distance field is written once and may be traced, bricked or meshed
(`src/draw/field.ts:60-68`). A field that could overshoot is caught at compile time by sampling its Lipschitz
constant on the CPU (`src/draw/field.ts:33-36`). Mesh normals under an arbitrary motion come from the symbolic
Jacobian's cofactor (`src/draw/mesh.ts:9-16`). This is real mathematics put to work, and the comments mark which
statements are derivations.

**The host contract.** `src/artefact.ts` and `src/host/manifest.ts` together are a complete, written specification
of what a second host must do, down to binding numbers and the rule for resolving multisampled depth
(`src/artefact.ts:60-78`). The proof that it is sufficient is that a second host exists in another language
(`loft-loader`). The executor is short because every decision was made earlier (`loadBundle`, `src/host/host.ts:643`).

**Error handling.** `SceneError` takes where, given, expected and fix (`src/errors.ts:9-15`). Constructors validate
their arguments and name the author's own object (`src/mistakes.ts:22-45`). A GPU refusal of a line the author wrote
in raw WGSL is mapped back to the author's file and line (`src/host/host.ts:198-216`). A stale native addon says
exactly which command rebuilds it (`src/core/native.ts:86`). I hit that last message myself and it was right.
There are 36 fixtures of deliberate mistakes under `tests/fixtures/mistakes/`.

**Checks that prove rather than sample.** `always(range, condition)` runs branch and bound over boxes with interval
and affine arithmetic and returns holds, fails or undecided with a typed reason (`src/ranged.ts:25-52`, `:121`). It is
honest about scope: the bounds cover the CPU evaluation of the graph, not the pixels (`src/interval.ts:1-40`).

**Measurement is built in.** GPU time per draw by timestamp queries, frame identity hashes, the triangle count
actually drawn. I got every number in section 4 from the shipped CLI without writing a line.

**No silent fallbacks, in general.** Unknown input and param names throw (`src/host/host.ts:123`, `:134`). File artefacts
refuse tables that need JavaScript by name (`src/host/host.ts:649`). There are no `TODO`, `FIXME` or `HACK` markers in
`src/` or `rust/`, no `@ts-ignore` in `src/`, and no skipped tests.

**The newer Rust code is careful.** `unsafe` is denied workspace-wide and allowed only in the two crates that
need it (`rust/Cargo.toml:24-28`; the older `loft-loader` has not opted in); wildcard matches on owned enums are denied (`:32`); `loft-ir` forbids float
arithmetic so that bundles are byte-identical across platforms (`rust/crates/loft-ir/src/lib.rs:3`).

## 6. What is fragile, duplicated, dead or over-built

### 6.1 Two compilers, two bundle formats, three hosts

| Job | Implementations |
|---|---|
| Compile a scene | TypeScript (`src/compile`), Rust (`rust/crates/loft-core`) |
| Bundle on disk | `loft-artefact` manifest, versions 1 to 5 (`src/host/manifest.ts:23`); `scene.loft` Bundle V1 (`rust/crates/loft-ir/src/bundle.rs:1-4`) |
| Host | TypeScript (`src/host/host.ts`), Rust `loft-loader` (old format), Rust `loft-gpu` (new format) |
| Interval checks | `src/ranged.ts` + `src/interval.ts`; `rust/crates/loft-core/src/check.rs` + `interval.rs` |
| Motion solver | `src/geom/solve.ts` (1,014 lines); `rust/crates/loft-core/src/solve.rs` (1,086 lines) |

The Rust headers say they are ports: "The search is `src/ranged.ts`'s" (`rust/crates/loft-core/src/check.rs:3`). The
two sides have already diverged: Rust replaces the TypeScript affine forms with a centred form (`check.rs:6-13`).

The bridge between them is a recorder that logs every expression node as four words (`src/record/recorder.ts`,
`src/record/wire.ts`). Today it carries expressions and table fills but not draws:

- "`draws` is 0 until the library records its drawables as Draw decls" (`src/core/native.ts:15`).
- "Always 0 until the wire pins Draw/Pass decls: the bundle cannot be drawn" (`rust/crates/loft-core/src/lib.rs:75`).
- "No corpus scene records them yet" (`rust/crates/loft-core/src/lib.rs:15`).
- `Program::Evaluate` returns `Internal::Unbuilt` (`rust/crates/loft-core/src/lib.rs:175`).
- `loft-loader` "moved over unchanged ... it becomes loft-gpu in lane 3.2" (`rust/Cargo.toml:26`), and has no tests.

The TypeScript engine pays for the migration now: it interns nodes for itself and records them for Rust
(`src/record/recorder.ts:8-12`), and graph functions are inlined for one engine and recorded as calls for the other
(`src/record/graph_fn.ts:1-4`). Nothing in `tests/` tests the recorder or the wire directly.

A newcomer cannot tell from the code which engine will exist in a month. That alone rules out depending on either.

### 6.2 Pixel-exactness is treated as an invariant, and it constrains the code

Several optimisations are held back, or special-cased, because they changed a hash:

- Algebraic identities (`x * 0`, `x + 0`) are applied only inside a style's body, because applied everywhere they
  "changed the GPU's rounding of vertex positions: serpent ... moved by up to 10 levels in a few hundred pixels"
  (`src/expr.ts:593-600`).
- Multiplying by a plain array of ones is folded, but multiplying by the number 1 is not, because "folding them
  would change every scene's text" (`src/expr.ts:584-587`).
- Shader sharing leaves vertex-stage literals alone because "one silhouette sample ... moved (two pixels, 58/255)"
  (`src/host/share.ts:20-23`).
- Optional parts are appended only when used "so a scene without one keeps its shaders byte for byte"
  (`src/compile/context.ts:71-72`).

The one stored image test is byte-exact against a file made on the author's GPU, and its comment records the
reference being regenerated at least four times in its first days (`tests/styles.gpu.test.ts:40-55`). Byte-exact pixels across GPU vendors is not achievable
in general, so this invariant holds only on one machine. As a discipline for one developer it is defensible. As a
property a second team must preserve it is a tax on every change.

### 6.3 Global mutable state

The expression tables are module-level and are never cleared: `interned`, `calls`, `libCalls`, `vecs`, `gets`,
`scalars`, `params`, `nextId` (`src/expr.ts:198-240`), plus a process-wide `namespace` that "does not nest"
(`src/expr.ts:342-349`) and a process-wide registry of author WGSL texts (`src/authored.ts:15`). One crab made
61,000 distinct nodes (`src/expr.ts:194`). A long-lived process that compiles many scenes grows without bound,
which is presumably why `serve` compiles each scene in a child process. A test says so directly: "built in its own
process (the recorder is global state)" (`tests/serve.rust.test.ts:40`). An editor or a web app that recompiles on
every keystroke would need the same workaround.

### 6.4 The core assumes its content

- `SceneSpec` requires `ground`, `style` and `sun` (`src/scene.ts:15-27`). The sun must be above the horizon
  (`src/scene.ts:117`).
- The sky, the ground and a shadows drawable are always prepended to `things` (`src/scene.ts:89`).
- Every lit fragment computes `sea_height(in.world.xz) - in.world.y` (`src/draw/surface.ts:319`), whatever the ground.
- `Style` has eleven required members including `water`, `shadow`, `ground`, `sky` and `ink`
  (`src/style.ts:207-243`). A style for a scene with no sea must still supply a sea colour.
- Draw phases are named for the subject: `"on-bodies" | "water" | "on-water"` (`src/compile/context.ts:33`).
- At most seven lights besides the sun (`src/scene.ts:46`). The sun's cast shadows fall only on the ground, as
  capsules built from each body's chain of joints (`src/draw/shadows.ts:1-2`, `src/map/types.ts:56`). Bodies do not
  shadow each other or themselves under the sun: its visibility is 1 (`src/light.ts:63-65`). Only lamps have shadow
  maps.
- The colour target is `rgba8unorm`, display-referred, with "no automatic conversion anywhere"
  (`src/artefact.ts:11`). The PBR style linearises and tone-maps inside its own shade function.
- Near and far planes are derived from the eye-to-target distance, fixed at 1/100 and 100 times
  (`src/host/camera.ts:35-36`).

For seas and creatures these are sensible defaults. For a product viewer, a building or a data visualisation each is
something to fight.

### 6.5 Content that has one user

The entry file itself marks sections "one user so far: general or not is unproven until a second subject uses it"
(`src/actions.ts:14`, `src/motion.ts:6`). A helper pass over `src/geom` and `src/spacetime` counted 280 exported
symbols, of which 112 are referenced by no example or test and 69 by one or two. Across all examples, 330 distinct
library names are imported and 140 of them appear in exactly one file. Comments in general-looking modules name a
single creature: "the crab's brow, the serpent's neck" (`src/map/sweep.ts:372-437`).

This is an art project's library growing by accretion, exported wholesale through one entry file. It is not wrong,
but an outside user pays for it in search time: the name you need is one of 800.

### 6.6 Duplication inside the TypeScript side

- **Hashes and noise.** `src/lib/noise.ts:1-2` says the library never uses `fract(sin(x) * big)` because its result
  depends on the precision of `sin`. `src/spacetime/wake.ts:73` uses exactly that. A second integer hash lives in
  `src/style/shared.ts:49-55` and a third in `src/style/papercraft.ts:48-55`.
- **Rotation about an axis** is written three times (`src/map/transport.ts:190`, `src/geom/placing.ts:160`,
  `src/map/chart.ts:114`).
- **The examples copy a one-line WGSL shim** for `fwidth` into five files (for instance
  `examples/recipes/hull.ts:40-41`), though `src/lib/screen.ts:18` already exports it.
- **Deep imports.** 53 of 125 example files import past the public entry. Eight names they need are not exported at
  all, including `rotate` from `src/map/chart.ts` in six files.
- `Rgb` is declared in six files and `V3` in eight or more.

### 6.7 Generated WGSL by string concatenation

Drawables build shader modules by joining template strings with interpolated names
(`src/draw/surface.ts:144-178`). Names are kept apart by convention (`loft_`, `sty_`, `surf_<name>`) and a regular
expression on identifiers (`src/draw/surface.ts:115`). The `soft` directory alone has about 200 interpolations in 29
WGSL blocks. The sharing pass then re-parses that text to find modules that differ only in literals
(`src/host/share.ts:9-18`). Generating text and then pattern-matching it to undo the specialisation is a workaround
layer; the Rust design (named functions hashed by content, fixed templates) removes the need for it.

### 6.8 Environment fragility

- **The GPU binding needs a patch.** `patches/bun-webgpu@0.1.7.patch` pins buffers that the garbage collector was
  freeing before Dawn read them. The CLI still retries a failed load on up to three fresh devices
  (`src/cli/main.ts:79-101`).
- **The test suite takes a machine-wide lock** and exits with code 75 if another checkout is testing
  (`tests/setup.ts:11-21`). With no GPU it fails rather than skips.
- **Rust tests read files that are not in the repository.** They panic with "Write it first: bun ..." if
  `out/lr21/kernels.jsonl` is missing (`rust/crates/loft-core/tests/it/kernels.rs:246-250`); `out/` is ignored by git.
- **Tools reach into sibling repositories and one home directory**: `$HOME/git/room-runners-arena-lib/keel/Cargo.lock`
  (`tools/check_pins.sh:11`), a hard-coded Node path in `tools/gallery.sh`, and a build lock shared with another
  project (`tools/lib/build_gate.sh`).
- **There is no CI configuration** and no pre-commit hook in the tree.
- **The native addon is loaded from `rust/target/lab/`** and only `.dylib` or `.so` is considered
  (`src/core/native.ts:103`), so Windows is not handled.

### 6.9 Dead or orphaned

- About thirty scripts in `tools/` are referenced by nothing (benchmarks, one-off delivery scripts, picture
  generators). Two Python tools generate concept art with an image model and are not rendering code.
- `tests/fixtures/mistakes/run.ts` holds 36 cases and is not run by `bun test`.
- `src/record/sample.ts` and `src/record/score.ts` are measurement tools living under `src/`.
- `Op::Grad` is reserved on the wire and unused (`src/record/wire.ts:15`).
- The manifest carries five format versions with rules for writing "byte for byte what it was"
  (`src/host/manifest.ts:13-23`) in a private repository that is nine days old.

### 6.10 Over-built for its age

The `serve` supervisor (699 lines: sandboxed compile children, admission limits, deadlines, typed exit codes), the
frame identity hashes, the five manifest versions and the mixed f16/f32 precision path are production machinery
around an engine whose central design is still being replaced. Each is well made. Together they are a lot of surface
to carry through a rewrite.

## 7. How hard it is to learn

**The first hour is easy.** `examples/first/lantern.ts` is readable without any documentation. `loft render` gave me
a picture in half a second. `loft api <name>` and `loft show` exist. Mistakes are answered with the fix.

**The next week is not.** The reasons are specific:

1. **Arithmetic is prefix calls.** `mul(belly, sub(1, mul(sub(1, rib), 0.05)))` (`examples/first/lantern.ts:14`).
   JavaScript has no operator overloading, so this is forced, and three.js's TSL has the same problem. But it makes
   every formula harder to read than its GLSL equivalent. Loft's own answer is the `wgsl` template tag, which parses
   a subset of WGSL into the same graph (`src/text/parse.ts`, `src/text/lower.ts`). That is a second authoring
   language with its own rules about what stays a graph and what becomes GPU-only "raw" text
   (`src/text/parse.ts:67-100`).
2. **Eight hundred names in one flat namespace.** The entry has to rename its own collisions: `Key` versus
   `LightKey`, `Cast` versus `LightCast`, `compose`, `extrude`, `round` (`src/index.ts:10-15`,
   `src/styles.ts:5-6`).
3. **Many new concepts before the second example.** Chart, Body, Strand, Poses, Placed, Skeleton, Painted channels,
   tables, map and scan, recur, rates, phases, schedules, designs, variants, slots.
4. **Types leak.** Examples need `lift<"f32">(1)`, `In<"f32">`, `Expr<"vec2">`, and eleven `as unknown as` casts.
5. **You must think in closed forms.** State exists only through `recur` (stepped on the GPU and baked) or raw
   compute. A physics step or a game loop that mutates objects has no natural place.

For a language model with the `loft context` output in its prompt the cost is lower, and I think that is the
intended reader. For a human team I would budget two weeks before someone is productive beyond strands and fields,
and I would expect them to read `src/` rather than rely on the API listing.

## 8. What is missing

Searched for and not found in code:

| Missing | Evidence |
|---|---|
| Mesh or glTF import | only export exists (`src/bake/gltf.ts`); `draw.mesh` takes arrays of number arrays built in code (`src/draw/mesh.ts:38-47`) |
| Image textures, texture sampling by authors, mipmaps | the only textures bound are internal: scene depth, clearance, post colour, pass outputs (`src/artefact.ts:78-155`) |
| Skinned meshes on input | the baker can write skins; nothing reads them |
| Picking and hit testing from the host | no object-id pass or ray query API; the Rust host states "Inputs in, pixels out, nothing back" (`rust/crates/loft-gpu/src/lib.rs:3`) |
| Sorted or order-independent transparency | draws are ordered by phase and by list order (`src/compile/index.ts:161-162`) |
| HDR output, wide colour | fixed `rgba8unorm` target |
| Sun shadows on anything but the ground | capsule patches on the sea or plane only (`src/draw/shadows.ts:1-2`) |
| Text and UI | a 5x7 bitmap font for labelling contact sheets (`src/cli/label.ts`) |
| Camera controls as an API | an `orbit` helper used by the dev viewer (`src/host/camera.ts:153`) |
| A published package | `"private": true`, version 0.0.1, exports raw `.ts`; `createHost` is not reachable through the one export (`src/host/index.ts:3` against `package.json`) |
| A licence | no licence file and no licence field anywhere |
| WebGL fallback, Windows path, CI | see 6.8 |
| A stable API | 781 commits in nine days |

The missing licence alone means I could not legally ship it.

## 9. Compared with what I would otherwise use

**three.js (with the WebGPU renderer and TSL).** TSL is the closest mainstream relative of Loft's expression graph:
node functions compiled to WGSL or GLSL, with the same prefix-call arithmetic. three.js gives me glTF, textures,
skinning, shadows, post-processing, picking, controls, a scene graph, fifteen years of answers, and an MIT licence.
It does not give me a CPU twin of every shader expression, proofs over time ranges, automatic lifting of per-frame
work into compute stages, or a scene as a self-contained data bundle. For any project with assets or interaction,
three.js wins without a contest. For formula-only art, Loft's model is cleaner.

**Babylon.js.** The same answer with more engine: physics, GUI, an inspector, a node material editor. Heavier than
three.js; far more complete than Loft.

**Bevy.** A Rust game engine on `wgpu` with an entity system, glTF, PBR and a plugin ecosystem. Slow to compile and
its API still moves between releases, but it is a general engine with thousands of users. If I were writing the
game that hosts Loft's creatures I would write it in something like Bevy and consider Loft only as the generator of
one subject's shaders, through `loft-loader`.

**Raw wgpu or WebGPU.** Total control and total cost. Loft's manifest executor is close to what I would write by
hand for a data-driven renderer, and it is the piece I would most readily lift: about 1,000 lines of TypeScript
(`src/host/host.ts`) or 3,300 of Rust. If I wanted Loft's ideas without its content I would start from raw `wgpu`,
borrow the contract, and write my own emitter.

**Shadertoy-style fragment shaders (and shader-park, which compiles JavaScript to distance-field shaders).** This is
Loft's nearest competitor for its actual use. A single fragment shader starts instantly and has no table budget, no
phases and no 800 names. Loft adds things a shader toy cannot do: rasterised parametric surfaces that composite
exactly with traced fields by depth, instancing, compute-filled tables, a CPU twin, level of detail, checks, baking
to glTF. If the project is one image, I would write the shader. If it is ten thousand related images made by a
program, Loft's structure starts to pay.

**Houdini or Blender geometry nodes with a batch renderer.** For offline procedural stills at volume this is the
conventional answer. It is slower per image and far more capable, and artists already know it. Loft wins only when
the author is code or a model and the frame budget is milliseconds.

## 10. Would I use it?

**For a new general project: no.** No assets, no interaction, load time proportional to content, an engine being
replaced under its own API, no licence.

**For this kind of project: yes, as a pinned fork.**

- A generator of stylised procedural pictures or loops at volume, authored by a program or a language model.
- Each scene small: under about thirty drawn things, so load stays under a few seconds.
- Headless on Apple hardware, or the browser viewer for inspection.
- Value placed on determinism, on checks ("the telegraph is visible before the hit lands, for all t"), and on error
  messages a model can act on.

I would pin a commit, use only the TypeScript engine, ignore `rust/` apart from `loft-loader`, write against perhaps
sixty names (the expression functions, `strand`, `surface`, `draw`, `scene`, one style), and treat everything in
`src/geom` and `src/spacetime` as examples to copy from rather than an API to rely on.

**For one hero subject inside someone else's game: possibly**, through `loft-loader`, once load time is addressed.
That appears to be the author's own plan, and the contract is clean enough for it.

## 11. The five things that would change my answer

1. **Load time independent of scene size.** Fixed shader templates with content passed as data, as the Rust draw
   design describes. Target: the five-crab scene loads in under two seconds cold. Today it takes 55.
2. **One engine.** Finish the Rust move or abandon it. One compiler, one bundle format, one host per platform, one
   implementation of checks and solving, and the recorder scaffolding removed from the TypeScript hot path. Until
   then every part of the system has a twin that may or may not be the future.
3. **A way in for assets.** At minimum, image textures that authors can sample, and mesh import (glTF with skins).
   Without these Loft can only draw what can be written as a formula.
4. **A core that stands without its content.** `scene()` with no mandatory sea, sun or style contract; the engine
   (about 21,000 lines) published separately from creatures, attacks and styles; a public surface of about one
   hundred names with the rest in opt-in modules; the host reachable from the package's exports; a version number
   that means something; a licence.
5. **Evidence that it holds up away from its author's machine.** CI from a clean clone, on at least two GPU vendors
   and two operating systems plus a browser; tests that need no sibling repository, no pre-generated `out/` files and
   no machine-wide lock; no retry loop around device creation; tolerance-based image comparison in place of
   byte-exact hashes; and a second real user.

Items 1 and 2 are the same piece of work, and the tree shows it is under way. Items 3 to 5 are decisions rather than
engineering, and I saw no code moving toward them.

## 12. Method and limits

- **Read in full or in large part by me:** `src/index.ts` and the six layer files, `src/expr.ts` (about two thirds),
  `src/scene.ts`, `src/artefact.ts`, `src/compile/index.ts`, `context.ts`, the headers and key parts of `tables.ts`,
  `rates.ts`, `bind.ts`; `src/record/recorder.ts`, `wire.ts`, `graph_fn.ts`; `src/core/native.ts`;
  `src/host/host.ts` (lines 1 to 700), `manifest.ts`, `headless.ts`, `cache.ts`, headers of `lower.ts` and `share.ts`;
  `src/draw/index.ts`, `surface.ts` (lines 1 to 330), headers of `field.ts` and `mesh.ts`; `src/map/types.ts`;
  `src/errors.ts`, `src/mistakes.ts` (part); `src/cli/main.ts`; `src/style.ts` (the `Style` type); `src/compute.ts`
  (part); the Rust workspace file, every crate manifest, and the heads of `loft-core/src/lib.rs`, `draw/mod.rs`,
  `emit/mod.rs`, `check.rs`, `loft-ir/src/bundle.rs`, `loft-gpu/src/lib.rs`, `loft-loader/src/lib.rs`,
  `loft-draw/src/main.rs`; `examples/first/*`; `viewer/main.ts` (part); `tests/setup.ts`.
- **Read by four helper passes and reported to me:** `src/geom`, `spacetime`, `soft`, `style`, `relief`, `lib`, the
  rest of `map`; the tests and tools; all examples; `src/cli`, `src/text`, the check and solve system, `src/bake`.
  Counts in sections 6.5, 6.6, 6.9 and 8 that I did not measure myself come from these passes. I re-checked at source the
  ones I quote with a line number (the `fwidth` shim, the sine hash, the Rust fixture panic, the pins script, the test
  lock, the stale-addon message).
- **Ran:** `loft help`, `loft show` on nine examples, `loft render` on five, `loft profile` on two, `tsc --noEmit`,
  one import of `src/index.ts` to count exports, and the TypeScript test suite once. All on one Apple M4. Pictures
  went to a scratch directory; the test run wrote only to the git-ignored `out/` and to the system temp directory. I
  changed no tracked file.
- **Did not do:** build or test the Rust workspace; run in a browser; run on Vulkan, DirectX or any non-Apple GPU;
  read any documentation, so where a design rationale lives only in Markdown I have not seen it. Performance on
  other hardware is inferred, not measured.
- **Uncertain:** whether the per-module WGSL line counts overstate what a driver actually recompiles (shared text
  may be cached below Dawn); the measured load times are the ground truth, the line counts are an explanation.
