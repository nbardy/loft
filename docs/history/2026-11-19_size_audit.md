# Loft size and bloat audit (19 Nov 2026)

Measured on `origin/main` 87c8ad1 (git archive, so local dirty files are excluded) and `feat/rust-core` 97bc1eb.
Counts are `scc` code lines: comments and blank lines are excluded unless stated. "Candidate" means found by grep or
token scan and not checked by a compiler. Read-only audit: nothing was run except `scc`, `git` and text scans.
The companion history is [2026-11-19_inventory.md](2026-11-19_inventory.md).

## Headline

- **src is 30,824 TS code lines** in 201 files, plus 11,991 comment lines (39% of code; `src/geom` has 2,854 comment lines to 2,905 code, and more comment than
  code in 16 of its 25 files). The repo holds **54.6k TS, 2.6k Rust on main, about 2.0k more Rust on `feat/rust-core`, and 19.6k
  Markdown**. DESIGN.md alone is 2,012 lines (174 KB).
- About **16.3k of the 30.8k src lines are the engine**: graph, CPU twin, checks, solve, tables, draws, format and
  host. A Rust port of the engine has to replace these. About 7.6k are authoring content (creature parts, styles,
  soft media, attacks) that stays in TS. The rest is the CLI (3.1k), the WGSL text front end (1.8k) and scene/map
  glue (1.9k).
- **The repo is 9 days old** (first commit 10 Nov). It holds 679 commits: 62.2k lines added and only 12.0k deleted in
  `src` and `rust`, and only 19 src files were ever deleted. The bloat is **accretion, not leftover debris**. Each
  feature arrived as a new parallel path next to an old one, guarded by an optional field so that old output stayed
  byte-identical. Every later rewrite was then required to keep it.
- A lean Rust engine is about **9.5–10.5k lines**. The engine today is 16.3k TS plus about 4.6k Rust: roughly a
  2x cut, if the port unifies concepts instead of porting module by module.
- The Rust proposal (`docs/proposals/2026-11-19_rust_loft.md`) is a careful parity plan, but **its gates freeze the
  bloat**. It requires byte-identical bundles, JS-bit CPU math, format versions 1–5, separate sampled and ranged checks,
  and 24 lanes mapped one to one onto today's modules. Section 4 lists what it should drop.

## 1. Size

### 1.1 By top-level area (code lines)

| Area | Code | Notes |
|---|---:|---|
| `src/` | 30,824 TS | 201 files; 11,991 comment lines |
| `tests/` | 7,270 TS | 84 files; 0.24 test lines per src line |
| `examples/` | 10,938 TS | `examples/bosses` is 10,571 of 13.4k total lines |
| `tools/` | 4,914 TS, 326 Py, 94 sh | benches, proofs, crab concept scripts |
| `viewer/` | 225 TS | |
| `agent_notes/` | 444 TS, 2,946 md | |
| `docs/` | 14,113 md | proposals 4.3k lines, art_direction 2.6k lines |
| root md | 1,844 | DESIGN.md 2,012 lines total |
| `rust/loft-loader` | 2,610 Rust | lib.rs 987, manifest.rs 844 |
| `rust/loft-core` (branch) | about 1,976 Rust | solve 774, share 666, tape 493, jsmath 111 (raw lines) |

### 1.2 `src` by module

| Module | Code | Largest files |
|---|---:|---|
| `src/*.ts` (top level) | 5,339 | expr 1,108; interval 858; check 745; grad 495; ranged 439; artefact 261 |
| `src/draw` | 3,422 | field 479; surface 369; mesh 291; impostors 258; cast 254 |
| `src/host` | 3,291 | lower 791; host 779; fixture 270; share 236; profile 186 |
| `src/cli` | 3,130 | serve 419; args 352; catalogue 320; main 223; gallery*4 560 |
| `src/geom` | 2,905 | solve 706; sea 335; grab 217; stance 217 |
| `src/style` | 2,306 | light 314; pbr_parts 279; woodblock 223; post 204 |
| `src/text` | 1,837 | lower 1,012; parse 392; index 328 |
| `src/soft` | 1,774 | medium 379; density 355; glow 216; rain 179; specks 171; specks_veil 142 |
| `src/compile` | 1,728 | tables 712; index 235; passes 163 |
| `src/spacetime` | 1,397 | attack 459; routine 370 |
| `src/map` | 1,310 | sweep 300; recur 228; brick 206 |
| `src/lib` | 883 | fold 232; noise 232 |
| `src/bake` | 744 | motion 319; mesh 243 |
| `src/frame` | 408 | pass 269; lamp_shadow 139 |
| `src/relief` | 350 | trace 130 |

Engine and content split (sums of the files in each group):

| Group | TS code |
|---|---:|
| Expression graph and JS CPU twin (`expr.ts`) | 1,108 |
| Derivatives (`grad.ts`) | 495 |
| Interval evaluation and ranged checks | 1,297 |
| Sampled checks (`check.ts`, `check_state.ts`, `mistakes.ts`) | 928 |
| Solve (`geom/solve.ts`) | 706 |
| Tables, rates, recur, compute, bake states | 2,463 |
| Draw emitters (`draw/`, `frame/`, ray/particle parts of `soft/`, `relief/trace`) | 5,127 |
| Artefact, manifest, lowering, files, precision, share, fixture | 1,859 |
| Host executor (host, detail, camera, still, profile, cache) | 1,485 |
| Identity, bake (glTF) | 844 |
| **Engine subtotal** | **16,312** |
| WGSL text front end (`text/`) | 1,837 |
| CLI | 3,130 |
| Scene, design, light, style.ts, curve, map algebra, barrels | 1,933 |
| Content: geom (without solve), style/, spacetime, soft media, lib, relief | 7,612 |

### 1.3 The 20 largest code files (all languages)

```
1108 src/expr.ts                 495 src/grad.ts
1012 src/text/lower.ts           479 src/draw/field.ts
 987 rust/loft-loader/src/lib.rs 459 src/spacetime/attack.ts
 858 src/interval.ts             439 src/ranged.ts
 844 rust/loft-loader/src/manifest.rs  433 examples/bosses/titan/rig.ts
 791 src/host/lower.ts           419 src/cli/serve.ts
 779 src/host/host.ts            411 examples/bosses/tithe_fleet.ts
 745 src/check.ts                399 examples/bosses/maelstrom_wyrm.ts
 712 src/compile/tables.ts       394 tools/synth.ts
 706 src/geom/solve.ts           392 src/text/parse.ts
 511 examples/bosses/wreckers_lamp.ts
```

### 1.4 Growth

The repo has existed for 9 days, so the half-day snapshots below are more useful than weekly totals. Each pair is
code/comment lines at the last first-parent commit before that time:

| Snapshot | src | tests | examples | tools | md | Rust |
|---|---:|---:|---:|---:|---:|---:|
| 11-10 23h | 5,380/1,441 | 2,513 | 253 | 0 | 396 | 0 |
| 11-11 12h | 13,158/3,589 | 4,559 | 1,934 | 985 | 1,701 | 0 |
| 11-11 23h | 16,481/5,040 | 4,777 | 5,982 | 1,818 | 4,300 | 1,988 |
| 11-12 23h | 19,387/6,613 | 4,793 | 8,887 | 3,970 | 9,980 | 1,988 |
| 11-13 23h | 19,286/8,800 | 4,962 | 9,826 | 4,270 | 14,141 | 1,988 |
| 11-14 23h | 19,707/9,097 | 4,967 | 9,895 | 4,317 | 14,168 | 1,988 |
| 11-15 23h | 22,407/9,968 | 5,068 | 10,313 | 4,317 | 14,468 | 2,072 |
| 11-16 23h | 23,510/10,392 | 5,186 | 10,327 | 4,637 | 14,771 | 2,077 |
| 11-18 23h | 30,824/11,991 | 7,269 | 10,938 | 4,914 | 19,573 | 2,610 |

- **Week 1 (10–15 Nov):** src went from 0 to 22.4k. **Week 2 so far (16–18 Nov):** +8.4k, of which +7.0k landed on
  18 Nov alone.
- **13 Nov was a doc-comment day.** Code was flat while comments grew by 2.2k (commit 924e06a, +1,980 lines).
- **Churn:** 62,236 lines added and 12,036 deleted in src+rust. The most-rewritten areas were `src/geom` (+8.5k/−2.1k),
  `src/draw` (+6.1k/−1.4k), `src/host` (+5.6k/−1.4k), `src/style` (+5.2k/−1.3k) and `src/compile` (+3.7k/−1.4k).

The largest single commits (lines added − deleted, in src/rust/examples/tests/tools):

| + / − | Commit | What |
|---|---|---|
| 3,565 / 0 | b75a2c5 11-11 | Rust loader crate on wgpu 30 |
| 2,890 / 21 | 0a94d4e 11-18 | `wgsl` text lowered into the graph (`src/text`) |
| 2,473 / 0 | 100ae90 11-10 | first compiler: curves, strand, sea, toon, pose, artefact |
| 2,414 / 1,632 | 9ddaa7a 11-10 | map algebra rethink (DESIGN 7) |
| 1,907 / 1,100 | a7a5f88 11-10 | compiler core v2 |
| 1,692 / 143 | 4d3188b 11-18 | author compute passes |
| 1,561 / 9 | 1b4937e 11-18 | ranged checks |
| 1,451 / 522 | d13fefc 11-11 | pose on the GPU (table programs) |
| 1,296 / 2 | 6560254 11-11 | spacetime attacks + `loft check` |
| 1,226 / 0 | 7a09920 11-10 | lib folds and SDFs |
| 1,197 / 762 | ac18860 11-11 | design variants |
| 1,128 / 19 | a5db84a 11-18 | `solve` |
| 1,127 / 76 | 3b6b243 11-14 | offscreen passes |
| 1,121 / 0 | b78725f 11-10 | `grad` |
| 1,027 / 27 | 5f47811 11-14 | `recur` |

The pattern: each of the four big 18 Nov features (text, compute, ranged, solve) arrived whole, with almost nothing
deleted. Compute arrived beside `recur` and the table programs. Ranged checks arrived beside the 17 sampled checks.

## 2. Bloat, with evidence

### 2.1 Several representations of one thing

**One scene in three formats, plus a fourth on the Rust side.** These are the in-memory `Artefact`
(`src/artefact.ts:461`), the on-disk `Manifest` (`src/host/manifest.ts:37-160`) and the compile-side specs
(`src/compile/context.ts:44-90`). `lower.ts` (791 lines) translates `Artefact` into `Manifest`, and
`rust/loft-loader/src/manifest.rs` (844) mirrors `Manifest`. Each pair is defined as a diff of the one before:

- `DrawSpec = Omit<Draw, "pass" | "detail"> & { phase; detail: DetailSpec }` (`context.ts:58`)
- `DetailSpec` vs `Detail` (`context.ts:44` vs `artefact.ts:309`)
- `Phase` plus `PASS_OF` vs `Draw.pass` (`context.ts:33-37`)
- `PostSpec` vs `PostPass` (`context.ts:80` vs `artefact.ts:430`)
- `Shader` vs `shaders: Record<string,string>`

`Artefact` also carries closures (`cpuPose`, `pose`, `artefact.ts:499-506`) that a file cannot hold. That forces a
`js-tables` identity variant (`identity.ts:26-28`) and a `NotDataError` path (`host/files.ts:30-32`,
`host/host.ts:647-650`).

**GPU memory has at least nine names:**

- `DynamicBuffer` (`artefact.ts:169`), `StaticBuffer` (`:175`), `TableInfo` (`:283`) and `CpuUpload` (`:286`)
- `Recurrence.data` (`:237`)
- `ComputeResource` with buffer/texture `Shape` (`compute.ts:11,36`)
- `TableSource = ProgramTable | FilledTable` (`expr.ts:75`)
- `TableShape.role: "stations" | "data" | "static"` (`expr.ts:85`). `"stations"` exists only to feed a stats
  counter (`compile/tables.ts:536`).
- `TableRef` (`context.ts:25`) and manifest `BufferDesc`/`TextureDesc` (`manifest.ts:37-50`)
- pass textures `TextureSize` (`artefact.ts:373`)

**The fill of a table is chosen by three overlapping sum types.** These are `TableProgram` `map | scan | cpu`
(`expr.ts:111`), `TableFillKind` `map | scan | cpu | replay` (`artefact.ts:192`) and the internal canon in
`compile/tables.ts:77`. A static table is encoded as `kind: "cpu", why: "filled once, at build"`
(`compile/tables.ts:177`), so a string carries the distinction.

**Three mechanisms step state on the GPU:**

- table programs (pose stages, each frame)
- `recur` (`map/recur.ts`, `compile/recur.ts`, `Recurrence`, bake plus checkpoint replay, `host/bake.ts`)
- author `compute` with `when: build | load | frame | steps` (`compute.ts:14-18`), whose `steps` has its own
  start/odd/even ping-pong and history copies (`manifest.ts:96-110`)

`recur` with checkpoints and `compute` with `steps` are the same idea: a step function at a rate, keeping every k-th
state. One is written in expressions and one in raw WGSL.

**Two cameras, three matrix builders.** The host `Camera` (`artefact.ts:159`, numeric perspective/orthographic) goes
through `cameraMatrices` (`host/camera.ts:126`), and Rust repeats it in `loft-loader/src/camera.rs`. `PassCamera`
(`frame/pass.ts:36`, expressions: look/mirror/matrix) goes through `cameraLanes` (`frame/pass.ts:250`). Further
local types sit on top: `CameraAsk` (`cli/serve.ts:65`), the detail `Camera` (`host/detail.ts:37`),
`FALLBACK_CAMERAS` (`cli/cameras.ts:4`) and `SHEET_CAMERA` (`style/sheet.ts:18`).

**Two `Verdict` types**, `check.ts:44` (sampled | ranged | raw-compute) and `cli/verdict.ts:8` (exit classes), next
to:

- `Decision` and `Spent` (`ranged.ts:25,39`)
- `HoldReport`, `SolveReport` and `SolveFailure` (`geom/solve.ts:180-206`)
- `checkOutcome` (`check.ts:49`)
- **31 Error subclasses** (`RoutineError`, `BendError`, `PendulumError`, `StruckError`, `RestlessError`,
  `ExpressionError`, `PbrError`, ...). These are mostly `SceneError` with a different name.

**Precision is a duplicated overlay, not a parameter.** Every shader may carry a second copy: `Shader.mixedCode`,
`PostSpec.mixedWgsl`, `Artefact.mixed.{sceneWgsl, shaders, stages, statics}` (`artefact.ts:476`) and the manifest
`precision` overlay, chosen at load by a 40-line validator (`host/precision.ts`). Membership is a global `WeakSet`
side channel (`src/precision.ts:4`). The precision itself is spelled three ways: `Precision`, `FramePrecision` with
`"f32-fallback"`, and `Identity.build.precision`. `Identity.build.tier` has the single value `"compiled"`
(`identity.ts:31,119`), a dead discriminant.

**One concept, several types with the same name:**

| Name | Definitions (code text) | What differs |
|---|---|---|
| `F` | 22 | the same `Expr<"f32">` each time |
| `V3` | 14 | `readonly [n,n,n]` vs mutable `[n,n,n]` (`map/transport.ts:24`, `bake/motion.ts:34`) |
| `P3` | 10 | |
| `Rgb` | 8 | |
| `Frame` | 7 | for example, the expression frame `map/types.ts:13` vs the numeric frame `lib/similar.ts:94` |
| `Placed` | 3 | `map/types.ts:16`, `lib/similar.ts:164`, `compile/tables.ts:113` |

`Copies` is declared twice, identically (`draw/surface.ts:76`, `draw/mesh.ts:85`). `Recurrence` is declared twice
(`artefact.ts:237`, `map/recur.ts:41`). `src/index.ts:11-15` documents the name collisions it has to settle by hand
(`compose`, `extrude`, `round`, `Frame`, `Placed`, `Part`, `Plane`, `Posed`, `Key`). `lib/similar.ts` is a second,
numeric transform algebra (Frame/Part/Placed/Copy/Tree) beside the expression map algebra.

### 2.2 Parallel code paths doing one job

- **Five ray-hit drawables with one skeleton:** `field` (`draw/field.ts`, 479), `relief` (`draw/relief.ts` plus
  `relief/trace.ts`), `copies` (`draw/copies.ts`), `impostors` (`draw/impostors.ts`) and `traced`
  (`draw/traced.ts`). Each imports the same `classify, lookFns, tracedCall, withShading` (`copies.ts:28`,
  `impostors.ts:16`, `traced.ts:7`). Each then writes its own fragment template (`copiesWgsl` `:152`, `walkWgsl`
  `impostors.ts:236`, `fragmentWgsl` `traced.ts:146`), its own outline wiring, its own default shadow and its own depth
  entry. `context.ts:126` itself says "every other kind can be lowered to [hit]". March helpers exist three times:
  `draw/march.ts`, `draw/trace.ts` and `relief/trace.ts:155`.
- **Two media renderers:** `draw.density` (`soft/density.ts`, Gaussian closed form) and `draw.volume`
  (`draw/volume.ts`, ray march). `soft/specks_veil.ts` then has twin functions `specksVeil` (`:19`) and
  `specksVolumeVeil` (`:42`) to veil particles in each.
- **Four billboard and particle drawables:** `specks` (`soft/specks.ts`), `rain` and `splashes` (`soft/rain.ts:86,149`),
  and `glow`/`flare`/`strokes` (`soft/glow.ts`). These are all instanced quads with hand WGSL. Copies and points also
  exist as `draw.posed`/`points`.
- **Sampled vs ranged checks:** 17 hand-written sampled checks in `check.ts` (`clearance` `:296` through `shoves`
  `:913`) each run their own sampling loop over a `Probe`. Beside them sit `always`/`eventually` (`ranged.ts`) over
  `interval.ts`, and `solve`'s own hold verification (`geom/solve.ts`). `Ranged.run` can return a `sampled` verdict
  (`ranged.ts:52-58`), so the two paths already cross.
- **CPU evaluation four ways:** `compileJs` (the generated-JS twin in `expr.ts`), the interval `Evaluator`
  (`interval.ts:587`), the derivative walk (`grad.ts:392,432`) and the rate walk (`compile/rates.ts:44,80`). That makes
  seven separate `switch (n.op)` walkers in `expr.ts` alone (15 in src) (`:844, 892, 1002, 1117, 1188, 1438, 1519`). Every
  `LibFn` is written twice, as `wgsl` text and a `js` function (`expr.ts:55-61`): 43 `js:` twins.
- **Caches:** about 30 module-level `WeakMap` memo tables keyed by author objects, including
  `specks_veil.ts:10`, `glow.ts:77`, `cast.ts:98`, `shadows.ts:122`, `lamp_shadow.ts:67`, `pass.ts:74,82`,
  `pbr_parts.ts:210,347,348`, `ranged.ts:85,204`, `compute.ts:54,56`, `expr.ts:142,471,485,1171`,
  `solve.ts:84,606` and `style.ts:299`. Each hides scene state outside the graph, and each is a reason the proposal
  says Rust must "audit library uses of object identity".
- **Shader sharing twice:** `host/share.ts` (236 TS) and `loft-core/src/share.rs` (666 Rust), a "line-for-line twin"
  that reproduces JS regex leftmost-match behaviour (`share.rs:1-4`).
- **Barrels twice:** `src/actions.ts` and `src/spacetime/index.ts` re-export the same attack list. `src/styles.ts:9-20`
  and `src/style/index.ts:57-68` re-export the same style list. Together the public surface is about 723 names in
  explicit lists, plus `lib/` star exports.

### 2.3 Kind and null checks inside handlers

- 293 `.kind === "..."` comparisons, 350 `switch`es, 1,009 `=== undefined`/`!== undefined`/`??` and 129 `typeof`
  checks across src. 271 optional `readonly x?:` fields, and 53 "absent means ..." comments.
- **Ground kind** is checked inside six drawables (`volume.ts:190`, `specks.ts`, `shadows.ts`, `sky.ts`, `cast.ts`,
  `scene_wgsl.ts`) as well as its own dispatcher (`draw/ground.ts`).
- **The outline method** is read in nine drawables: `surface`, `mesh` (three times), `field`, `copies`, `impostors`,
  `traced`, `relief`, `light`, `woodblock`. It belongs once in the pass that composes the fragment.
- **Version and precision branching:** `host/precision.ts:12,17` checks `m.version !== 4 && m.version !== 5`, and
  `identity.ts:119` repeats `precision === undefined && version !== 4`.

### 2.4 String-keyed control flow

- `host/host.ts:339` picks an error message by `p.label.startsWith('compute "')`.
- `host/host.ts:647` keys CPU tables by `` `${label}@${offset}` ``.
- Ranged leaves use string keys `input:time`, `param:<name>` and `var:<name>` (`ranged.ts:61`), parsed back in
  `cli/explain.ts:170`.
- Namespacing by string prefix: `expr.ts:364,368` (`__` and `.`), `compute.ts:217` (`prev_`), `draw/shader.ts:25` and
  `frame/pass.ts:332` (`loft_`), `text/index.ts:360` (`_L`).
- `TableFillKind.cpu.why` doubles as a dependency marker ("read by ...", `compile/tables.ts:14`).
- Draws reference shaders by string key into `Artefact.shaders`.

### 2.5 Dead code, unused exports, shims

- **1,625 exported declarations in src. 346 have no reference outside their own file** (253 types, 92 values). Of
  these, 11 have no reference anywhere:
  - `currentNamespace` (`expr.ts:352`)
  - `callerSite` (`text/lower.ts:1230`)
  - `coilPlan` (`geom/spiral.ts:157`)
  - `echo` and `exaggerated` (`geom/lively.ts:315,417`)
  - `rayOf` (`geom/fan.ts:103`)
  - `pincerAngle` (`geom/grab.ts:466`)
  - `inflate` (`relief/height.ts:155`)
  - `wgslRgb` (`style/shared.ts:15`)
  - `LookName` and `LightName`
  
  30 more are used only by tests: `structuralHash`, `readTablesAlone`, `boundsFrom`, `fbm2`, `gaussRay`,
  `quatFromColumns`, `sampleGrid`, ... (candidates; token scan over src/tests/examples/tools/viewer).
- **The `FilledTable` shim:** `TableSource = ProgramTable | FilledTable` keeps the pre-program `prepare` form
  (`expr.ts:71-75,90`). Its only user is `relief/sculpt.ts:27`. The cited example (`examples/bosses/tithe/kit.ts`) no
  longer uses it. The whole `cpu` fill path (`interval.ts:259,288`, `files.ts:30`, `host.ts:647`,
  `identity.ts:26-28,82`, `compile/tables.ts:177-294`) exists for two tables in `soft/medium.ts:293,305` and
  `SculptTable`. No example uses it.
- **Format versions 1–5, a byte-identity shim:** `manifest.ts:13-22` and `lower.ts:243-248` choose the *lowest*
  version that can hold a file "so a file with neither is byte for byte what it was". `loft-loader` repeats this
  (`manifest.rs:12-16`). The code does this for a format that is 8 days old.
- **Byte-preserving optional fields as a design rule:** `light.ts:177` ("every scene written before lights keeps its
  shader text byte for byte"), `context.ts:72` (`depthEntry`), `artefact.ts:360` (`Draw.depth`), `manifest.ts:165`
  (`instanceBase`), `toon.ts:45` ("toon's frames predate the cap and stay as they were"), `woodblock.ts:145-159`
  (four "the print as it was" defaults) and `relief.ts:107`. This is the main source of optional fields and absent-case
  branches.
- **Stale comments:** for example, `text/lower.ts:642` says integer division is "raw until phase 2". `soft/medium.ts:293`
  says "not ported in phase 1".

## 3. A lean core

### 3.1 Data types (Rust)

Seven types replace the roughly 60 listed above.

```rust
// 1. One graph. Nodes are 4 x u32 (as the proposal says); one op enum, one table of per-op meanings.
enum Op { Const(LitId), Bind(BinderId), Slot(SlotId), Fetch{res: ResId, lane: u8}, Prim(Prim),
          Call(FnId), Loop(LoopId), Get(Swizzle) }
// Prim: the 53 operators. ONE table gives each its WGSL spelling, f32 eval, interval rule and derivative,
// so emit/tape/interval/grad are four folds over one table, not four hand-written switches.
// FnId: library and author functions are graph bodies; a raw WGSL function is Fn{text, sig, holes} with no CPU meaning.

// 2. One resource: every table, static, curve set, recur state, compute output, pass picture and the frame uniform.
struct Resource { id: ResId, shape: Shape, history: History, made_by: KernelId }
enum Shape   { Buffer{entries: u32, lanes: u8}, Texture{extent: [u32; 3], format: Format} }
enum History { Latest, Every, Checkpoints(u32) }

// 3. One kernel: what writes a resource and when. Replaces pose stages, map/scan/cpu fills, statics, recur and compute.
struct Kernel { writes: Vec<ResId>, body: Body, when: When }
enum Body { Map{runs: Vec<Run>}, Scan{init, step: LoopId}, Raw{fn_: FnId, dispatch: [u32; 3]} }
enum When { Build, Load, Frame, Steps{rate: f64, from: f64, count: u32} }

// 4. One draw model: two ways to cover pixels, and one shading interface.
enum Cover { Raster{grid: Grid | Mesh | Instances(Quad)}, Ray{proxy: Proxy, find: Find} }
enum Find  { Hit(FnId) /* field, relief, copies, impostors, traced are library hit fns */,
             Medium{density: FnId, emit: FnId} /* density and volume */ }
struct Draw { cover: Cover, shade: FnId, state: RasterState /* blend, cull, depth, phase */, detail: Detail }
struct Pass { target: Target, camera: Camera, draws: Vec<DrawId>, keep: Keep } // view, offscreen, depth, shadow, post
enum Camera { Look{eye, target, up, lens}, Matrix{columns, eye} } // fields are graph roots; the view camera is literals

// 5. One check and one verdict, for every check, hold and budget.
struct Check { domain: Vec<Leaf>, body: Quant /* always | eventually, nested */, margin: NodeId, how: Strategy }
enum Strategy { Prove{precision, budget}, Sample{n} }
struct Verdict { holds: Truth /* True | False | Undecided(Reason) */, margin: f64, bounds: Interval,
                 witness: Vec<(LeafId, f64)>, work: Work, scope: Scope /* Proved | Sampled(n) */ }

// 6. One artefact: the manifest is the in-memory artefact (serde). No lower step, one version, no closures.
//    Precision is an emit parameter: the bundle holds f32 modules and, optionally, f16 modules,
//    as one map Precision -> modules.
// 7. One identity: sha256(bundle bytes) + canonical view record. One Error enum:
//    Usage | Load | Scene{site, what} | Check | Environment | Budget | Internal.
```

### 3.2 Passes

1. **decode and intern** the recorder spans into the graph, with constant folding at the f32 points
2. **derive** (gradients as a graph rewrite)
3. **rate**: classify each node as Const/Build/Load/Frame/Fragment, then lift and assign kernels and slots
4. **schedule** kernels in topological order over resources
5. **emit** WGSL, one emitter with a precision parameter
6. **layout and share**: bind groups and shader sharing
7. **write** the bundle

The analyses run on the same graph: the tape (f32, lane-batched), interval proof, and solve. The host loads the bundle,
then runs kernels by `When` and passes in order, with LOD. The CLI and the authoring library stay in TS.

### 3.3 Lines: today vs lean

| Lean Rust module | Folds in today | Today | Lean |
|---|---|---:|---:|
| graph, intern, fold, record decode | `expr.ts` graph half, `wire.rs`, `lib.rs` | about 700 TS + 160 Rust | 700 |
| prim table + tape | `compileJs` half of `expr.ts`, `tape.rs`, `jsmath.rs`, 43 `LibFn.js` twins | about 400 TS + 600 Rust | 600 |
| grad | `grad.ts` | 495 | 300 |
| interval + quantifiers | `interval.ts`, `ranged.ts` | 1,297 | 800 |
| checks (one Check, one Verdict) | `check.ts` core, `check_state.ts`, `mistakes.ts`, `cli/verdict.ts` | 928 + 75 | 300 (the 17 named checks become TS library expressions) |
| solve | `geom/solve.ts`, `solve.rs` | 706 + 774 | 600 |
| resources, kernels, rates, schedule | `compile/*`, `compute.ts`, `map/recur.ts`, `map/table.ts`, `host/compute_data.ts`, `host/bake.ts` | 2,463 | 1,000 |
| draws, passes, WGSL templates | `draw/*`, `frame/*`, `soft/{density,specks,specks_veil,rain,glow,gauss}`, `relief/trace` | 5,127 | 2,000 |
| bundle format + sharing | `artefact.ts`, `host/{manifest,lower,files,precision,share,fixture}`, `manifest.rs`, `read.rs`, `share.rs` | 1,859 + about 1,600 | 600 |
| host (wgpu), LOD, camera, still, profile, cache | `host/{host,detail,camera,still,profile,cache}`, `loft-loader` `lib.rs`/`plan.rs`/`camera.rs`/`cache.rs` | 1,485 + about 1,450 | 1,500 |
| identity + bake glTF | `identity.ts`, `bake/*` | 844 | 450 |
| **Engine total** | | **about 16.3k TS + 4.6k Rust = 20.9k** | **about 8.9k Rust (+15% margin: about 10.2k)** |

On the TS side, the recorder plus the text front end is about 1.4k (today `text/` is 1,837). A CLI without the
gallery page generator and the duplicated barrels is about 1.8k (today 3,130). Content stays at about 7.6k, and
shrinks further once the ray/particle drawables are library hit functions.

Where today's modules go:

| Today | Becomes |
|---|---|
| `draw/{field,relief,copies,impostors,traced,march,trace}` and `relief/trace` | `Cover::Ray` + `Find::Hit`; each shape is a TS hit function |
| `soft/density` and `draw/volume` | `Find::Medium` |
| `soft/{specks,rain,glow}` | `Raster{Instances(Quad)}` with a shade function |
| table programs, `recur`, `compute`, statics, `cpu` fills | `Kernel` (cpu fills deleted, with a typed refusal) |
| `frame/pass`, `lamp_shadow`, post, ink | `Pass` |
| artefact, manifest, lower, `manifest.rs` | the one serde bundle |

## 4. Where the Rust proposal keeps bloat it should drop

1. **The byte-identical gate everywhere (§5, all 24 lanes).** It requires full SHA-256 equality of `manifest.json`,
   `bin/` and `wgsl/` against a frozen TS oracle after every lane. That obliges Rust to reproduce:
   - `lower.ts`'s lowest-version selection and field order
   - the old interner's SameValueZero and representative-sign quirks (§3)
   - "JS's vector component order" and JS Math bits through `jsmath`
   - "leftmost scanner behavior" in sharing
   - first-refusal order
   
   Byte equality fits a one-for-one port. It forbids every unification in §3. Gate on semantics instead: the same
   verdicts, values within typed f32 tolerances, and pixels within the Metal-proof thresholds the proposal already
   defines in §6. Take one deliberate baseline reset, as the proposal already does for naming.
2. **Three arithmetic contracts (§3 "CPU twin uses f64 semantics, including JS's...").** If the CPU twin's only jobs
   are checks, solve and baking, it should match the GPU (f32), not JavaScriptCore. Drop `jsmath.rs` and the
   per-OS bit tests of "scaled `hypot`".
3. **Format versions 1–5 (lane 6c "Preserve versions 1–5").** There is no persisted user data from an 8-day-old
   format. Ship one version, and delete `formatVersion` and the version checks in `precision.ts` and `manifest.rs`.
4. **Sampled and ranged kept apart (§3 "Sampled solver verification and ranged proof remain distinct operations";
   "Port built-in sampled checks as typed measures").** Make sampling a `Strategy` of the one `Check`, with the
   17 named checks as TS expressions over it.
5. **Lanes mirror modules, not concepts.** Lanes 1b (`grid_detail`), 3c/3d (draw/style/text capture adapters), 5a
   (table declarations and CPU/static fills), 6a (recurrence and author compute) and 7a/7b (field, light, PBR, cast
   recipes) port each parallel path separately. There is no lane that merges tables + recur + compute into `Kernel`, or
   the five tracers into `Cover::Ray`. Add those as the first lanes after the recorder, and port drawables onto them.
6. **The recorder's descriptor list repeats today's split (§1).** "Dense tagged descriptors for loops, tables,
   functions, renders, compute, post and check requests" gives seven descriptor kinds. Render, post and offscreen are
   one `Pass`. Tables and compute are one `Kernel`.
7. **Compatibility hooks kept as permanent API (§2–3):**
   - the `compileJs` wrapper over a Rust tape
   - `LibFn.js` and `FilledTable.prepare` given an "explicit migration boundary"
   - an "external opaque CPU extension" error class
   - the 25-copy `gridDetail` specialization replayed "through an explicit recording of the old roots"
   - `replace`/`fold` "saved step" behaviour
   
   Delete the opaque-CPU paths outright: their users are two `medium.ts` tables and `SculptTable`. Express rest
   detail as one symbolic surface from the start.
8. **Precision as an overlay.** The proposal carries "mixed and f32 fallback" through the loader as today's
   duplicated-module overlay. Make it an emit parameter and a `Precision -> modules` map.
9. **Two engines kept indefinitely.** The JS implementation stays as a "pinned historical reference" after parity,
   and Dawn stays until wgpu gates pass. Set a dated removal for both, or the 9a "retire JS evaluator paths" step
   will slip the way the `cpu` tables did.

The proposal's FFI choice, closed-program boundary, arena layout, cost model and "never a quiet compiler fallback"
rule are sound. Keep them.
