# Lean Rust Loft (19 Nov 2026)

This is the architect's decision document for moving Loft's engine to Rust. Nothing in it has been built. It replaces
the module-by-module port plan on `feat/rust-core` (`docs/proposals/2026-11-19_rust_loft.md`, "sol's plan" below) as
the plan of record. It keeps that plan's boundary, arena, program unit and cost model, and drops its byte-identical
gate and the bloat that gate preserved.

Read first: [inventory](../history/2026-11-19_inventory.md), [size audit](../history/2026-11-19_size_audit.md),
[Keel devex](../history/2026-11-19_room_runners_devex.md), [authoring API](2026-11-18_authoring_api.md),
[next](2026-11-18_next.md), and on `feat/rust-core` sol's plan and `agent_notes/2026-11-18_rust_core.md`.

**What I understand the request to be.** Authoring stays TypeScript: the call-form API, the `wgsl` text template,
`scene`, `design`, `solve`, `always`/`eventually`. The model-facing API keeps its shape. Everything after the author's
module runs is Rust, written as one lean engine with unified types and one code path per job. "One code path" means
one GPU executor (wgpu) for `loft serve`, headless tools, the browser viewer and the Keel game. Keel adopts Loft as its
art runtime, and Loft adopts Keel's build discipline. Where the owner and the evidence allow a decision from theory,
this document makes it and states the single run that scores it.

Labels used below: **DERIVATION** (follows from stated numbers), **MEASURED** (cites a run), **CONJECTURE** (a
prediction, with the run that scores it).

---

## 0. Decisions in one table

| # | Question | Decision |
|---|---|---|
| 1 | Core types | Ten Rust types replace about 60 TS ones: `Graph`, `Resource`, `Kernel`, `Draw`, `Pass`, `Medium`, `Check`/`Verdict`, `Error`, `Bundle`, `Identity`. Precision is a WGSL alias chosen at load. §1 |
| 2 | Parity gate | Picture parity (1/255, bounded silhouette allowance), same verdicts, same typed refusals. One format, `V1`; no reader for v1–v5. §2 |
| 3 | Boundary | Sol's recorder and one call per program, over a hand-written N-API binding. The twin is f32 with pure-Rust `libm`. Names come from first use, from the first emitter commit. §3 |
| 4 | GPU path | `loft-draw` (wgpu) replaces Dawn for serve, render and tools. The viewer runs the same host as wasm on WebGPU. §4 |
| 5 | Keel | Loft does its side of Keel steps 0–3, 6 and 8. `loft-gpu` is the only wgpu crate. `wgpu = "=30.0.1"`, with a pin check against Keel. §5 |
| 6 | Devex | Keel's mechanisms, including the same compile-lock files. §6 |
| 7 | Pending work | Fold: program cache, branch-local `if`, budgets, f16 as the alias, f32-padded enclosures, migration. Drop: Dawn cache, warm worker, build-time bakes, tier I, the TS naming lane. §7 |
| 8 | Lanes | 5 waves and 15 lanes, each with its own files and line ceiling. At most 4 GPU-heavy lanes at once. §8 |
| 9 | Order | Three concerns, each with one cutover, and TS deleted in the same wave. §9 |

---

## 1. The lean core

### 1.1 What is wrong today

The audit's finding, in one line: 16.3k TS plus 4.6k Rust of engine, accreted as parallel paths behind optional
fields so that old output stayed byte-identical. The lean core removes that parallelism. It does not translate it
into Rust.

### 1.2 Crates

```
rust/crates/
  loft-ir     graph arena, the op table (spelling and typing), the 10 types, wire decode, bundle read/write,
              identity hash, Error.        deps: serde, serde_json, sha2.  no wgpu.  float_arithmetic = deny
  loft-core   inline/intern, grad, fold, rate, kernels, schedule, emit, layout; tape, interval, check, solve;
              program cache.               deps: loft-ir, libm.            no wgpu (checked)
  loft-gpu    the host: load a Bundle on a caller's Device, run kernels and passes, LOD, encode_into,
              pipeline cache, readback.    deps: loft-ir, wgpu =30.0.1.    the only wgpu crate; unsafe allowed
  loft-node   N-API addon: one entry `run(program, spans) -> status`.      deps: loft-core.  unsafe allowed
  loft-draw   binary: serve worker, still, frames, clip, profile, diff.     deps: loft-gpu, png
  loft-web    wasm32 glue for the viewer (wasm-bindgen).                   deps: loft-gpu
```

`loft-gpu` never depends on `loft-core`. A game loads bundles without the compiler, and the compiler never links
wgpu. Both rules are `cargo tree` checks in the static gate (§6).

### 1.3 The graph

```rust
/// One node, 16 bytes: the same words the TS recorder writes (§3.2), so decode is a bounds check and a copy.
#[repr(C)]
pub struct Node { head: u32 /* op:8 | ty:8 | flags:16 */, args: u32 /* offset in Graph::args */, n: u32, imm: u32 }

pub enum Ty { F32, V2, V3, V4, Bool, Rec(RecId) }        // U32/I32 join in authoring-API phase 2

pub enum Op {
    Lit,                    // imm: LitId (f64 bits as authored; folded at f32 points)
    Leaf(Leaf),             // time, a param, an input, a binder
    Prim(Prim),             // the ~70 operators: arithmetic, builtins, select, pick, swizzle, compare
    Call(FnId),             // a recorded graph function; inlined during interning, emitted as a WGSL fn
    Fetch { res: ResId },   // a row of a Resource (tables, kernel outputs, solved curves)
    Loop(LoopId),           // iterate / map / scan body: count, init, binders, next, stop
    Grad { wrt: u8 },       // a request; rewritten to its derivative graph during interning
    Raw(RawId),             // a raw WGSL function: text, signature, holes. GPU-only; the twin refuses it by name
}
pub enum Leaf { Time, Param(ParamId), Input(InputId), Bind(BinderId) }

pub struct Graph { nodes: Vec<Node>, args: Vec<u32>, lits: Vec<f64>, sites: Vec<Site>, fns: Vec<Fn>, loops: Vec<Loop>,
                   raws: Vec<Raw>, roots: Vec<NodeId> }
```

**One op table, four folds.** Each `Prim` has one row in `const PRIMS: [PrimDef; N]`: its WGSL spelling, its
type rule, its f32 evaluation, its interval rule and its derivative rule. Emit, tape, interval and grad are four
loops over one table. That replaces the seven `switch` walkers and the 43 `LibFn.js` twins. Adding an operator is
one row. A missing rule is a compile error, because `PrimDef` has no optional fields.

**Interning is the identity.** Hash-consing uses full key equality after a hash match. Literal bits are compared
exactly, so `-0` and `0` are different nodes. No JS SameValueZero quirk is carried over. Live roots are explicit.
Unused recorded nodes are still validated, so an error in a discarded expression still refuses, as it does today.
They are not emitted.

### 1.4 Resources and kernels: one model of state

`Resource` replaces `DynamicBuffer`, `StaticBuffer`, `TableInfo`, `CpuUpload`, `Recurrence.data`,
`ComputeResource`, `TableSource`, `TableShape.role`, `TableRef`, `BufferDesc`/`TextureDesc` and `TextureSize`.
`Kernel` replaces table programs (pose stages), `map`/`scan`/`cpu`/`replay` fills, statics, `recur` with its
checkpoints, and author `compute` with `when` and `steps` ping-pong.

```rust
pub struct Resource { shape: Shape, init: Init }
pub enum Shape { Rows { rows: u32, lanes: u8 }, Texture { extent: [u32; 3], format: TexFormat },
                 ViewSized { format: TexFormat, scale: Scale }, Frame /* the per-frame uniform */ }
pub enum Init  { Zero, Bytes(SectionId) /* filled at compile on the tape */, By(KernelId) }

pub struct Kernel { writes: Box<[ResId]>, reads: Box<[ResId]>, body: Body, when: When }
pub enum Body { Map { f: FnId, rows: u32 }, Scan { init: FnId, step: LoopId, rows: u32 },
                Raw { module: ModId, entry: NameId, dispatch: [u32; 3] } }
pub enum When { Load, Frame, Steps { dt: f32, from: f32, count: u32, keep: Keep } }
pub enum Keep { Latest, Every(u32) }   // Every(1) = full history; Every(k) = the old checkpoints
```

These are the rules, each decided once:

- **CPU fills are a rate, not a kind.** If a `Map`/`Scan` body has Const rate (it reads no time, param or input),
  the compiler runs it on the tape at compile time. The compiler then sets `Init::Bytes` and deletes the kernel.
  This replaces `TableFillKind.cpu` and its `why` string.
- **Opaque JS fills are gone.** The recorder has no slot for a JS function, so `FilledTable.prepare`, `SculptTable`
  and the two `soft/medium.ts` CPU tables are rewritten as graph `Map` bodies (lane 2.4). Today the build refuses
  Storm's file output. That refusal disappears because the case disappears.
- **`recur` and `compute steps` are one thing:** a `Steps` kernel. Ping-pong buffers, start/odd/even entries and
  history copies become the host's job for `Keep`. The author still writes `recur` (graph) or `compute` (raw).
  Both record a `Kernel`.
- **Pose stages are `Frame` kernels.** `stations` as a stats role is deleted.

### 1.5 Draws and passes: two ways to cover pixels

```rust
pub struct Draw { name: NameId, layer: Layer, cover: Cover, shade: Shade, raster: RasterState }
pub enum Layer { Subject, Surroundings }                    // Keel's "subject only" (§5)
pub enum Cover {
    Raster { prim: RasterPrim, count: Count },              // grid surfaces, meshes, instanced quads
    Ray { proxy: Proxy, find: Find },                       // everything found by a ray
    Screen,                                                 // post and full-screen passes
}
pub enum RasterPrim { Grid { u: u32, v: u32, place: FnId }, Mesh { res: ResId }, Quads { place: FnId } }
pub enum Find { Hit(FnId), Medium(MediumId) }               // Hit: a graph fn ray -> (t, normal, coords) | miss
pub enum Proxy { Box { min: [f32; 3], max: [f32; 3] }, Instances { res: ResId, local: [[f32; 3]; 2] } }
pub struct Shade { paint: FnId, style: StyleId }

pub struct Pass { target: Target, camera: CameraId, draws: Range<u32>, clear: Clear, compose: Compose }
pub enum Target { View, Texture(ResId), Shadow { light: LightId, map: ResId } }
pub enum Camera { Look { eye: NodeId, target: NodeId, up: NodeId, lens: Lens }, Matrix { cols: [NodeId; 4] } }
```

- **Five tracers become one template.** `field`, `relief`, `copies`, `impostors` and `traced` differ only in the
  hit function. The TS library already writes the sphere-trace as a graph `iterate` (`draw/march.ts`). After the
  port, each of the five is a TS library *hit function* over the graph. Rust knows one `Ray` template. It has one
  depth entry and one default shadow, and outline wiring lives in the pass composer. The three march helpers
  (`draw/march.ts`, `draw/trace.ts`, `relief/trace.ts`) become one library loop. `context.ts:126` already says
  every kind lowers to hit.
- **Four particle drawables become `Quads`** (`specks`, `rain`/`splashes`, `glow`/`flare`/`strokes`, posed
  points). A `place` function returns position, size, colour and opacity. The old instancing `draw.points` folds
  into `draw.posed`, as the authoring API already decided.
- **Outline, ink and ground are composed once** in `Compose`, not read inside nine drawables. The ground-kind
  checks in six drawables and the outline reads in nine are deleted.
- **One camera builder.** Cameras are graph roots. The frame kernel evaluates them on the GPU with one WGSL
  `look_at`/`project`. A host camera (`set_camera`, Keel's `set_view_projection`) writes the view camera's input
  leaves. The level-of-detail plan reads the camera back with the bounds it already reads. The CPU builders in
  `host/camera.ts`, `camera.rs` and `frame/pass.ts` (`cameraLanes`) are deleted.

### 1.6 One medium

```rust
pub struct Medium { density: FnId, albedo: NodeId, g: NodeId, light: MediumLight, integrate: Integrate }
pub enum Integrate { March { steps: u16, shadow_steps: u16 }, Gaussians { res: ResId } }
```

`draw.density` (a closed form over Gaussians) and `draw.volume` (a ray march) differ only in how the integral along
a ray is computed, so they are one `Medium` with two `Integrate` variants. Each medium emits one WGSL function,
`transmittance(p, dir) -> f32`, whichever variant it uses. Anything that a medium veils (particles, surfaces,
fog) calls that function. This deletes the twin functions `specksVeil` and `specksVolumeVeil`. It also closes the
authoring API's open follow-up, "grains not dimmed by a `draw.volume`", for free.

### 1.7 One check and one verdict

```rust
pub struct Check { site: Site, body: Formula, how: Strategy }
pub enum Formula { Atom { margin: NodeId }, Always(Quant), Eventually(Quant),
                   And(Box<[Formula]>), Or(Box<[Formula]>), Not(Box<Formula>) }
pub struct Quant { leaf: Leaf, lo: NodeId, hi: NodeId, body: Box<Formula> }
pub enum Strategy { Prove { budget: Work }, Sample { n: u32 } }

pub struct Verdict { truth: Truth, margin: f32, bounds: [f32; 2], witness: Box<[(Leaf, f32)]>, work: Work, scope: Scope }
pub enum Truth { Holds, Fails, Undecided(Why) }
pub enum Why   { Budget(Work), Opaque(RawId), State(ResId), WithinPrecision, Codomain(Prim) }
pub enum Scope { Proved, Sampled { n: u32 } }
```

- `always`/`eventually` (ranged), the 17 sampled checks, `solve`'s hold verification and `--explain` share these
  types. The 17 named checks become TS library *measures* (`clearance` and the others), which are expressions over
  `Formula::Atom`. The ones that cannot be expressions (`landsInside`'s footprint share, `budget`'s triangle
  count) keep `Strategy::Sample`, and the type says so. A `Sampled` scope can never print "holds" because the
  printer matches on `Scope`.
- Nested quantifiers, the and/or of two quantified checks (unbuilt today) and `Not` come from the recursive
  `Formula`. They are not separate features.
- `Decision`, `Spent`, `HoldReport`, `SolveReport`, `SolveFailure`, `checkOutcome` and `cli/verdict.ts`'s second
  `Verdict` are deleted. A solve returns curves plus `Vec<Verdict>`. A solve that fails is an `Error` (below).

### 1.8 One error

```rust
pub enum Error {
    Usage(Usage),                                  // exit 2: bad CLI or request
    Scene { site: Site, fault: SceneFault },       // exit 3: the author's fault, with the fix
    Budget { meter: Meter, limit: u64, spent: u64 },// exit 4: the scene asked for more than its budget
    Load(LoadFault),                               // exit 5: a bundle the host refuses (format, sizes, validation)
    Environment(EnvFault),                         // exit 6: the machine (device lost, library missing or stale)
    Internal(Internal),                            // exit 1: a Loft bug, never retried as the author's
}
pub enum SceneFault { Type { want: Ty, got: Ty }, Unsupported { what: Unsup }, Infeasible { a: Site, b: Site },
                      Unsolved { fix: FixHint }, Unreadable, NotFinite { at: Leaf }, Wgsl { line: u32, msg: Box<str> }, /* … */ }
```

The 31 TS `Error` subclasses (`RoutineError`, `BendError`, `PbrError`, …) become `SceneFault` variants or vanish.
The CLI maps `Error` to an exit code in one `match`. An author WGSL error that Naga or the driver refuses comes back
as `Scene { fault: Wgsl }` mapped through the bundle's site map. It is no longer the environment error with
retry, which is a known bug today (authoring API §5). The exit code numbers above are illustrative; lane 1.1 keeps
today's numbers.

### 1.9 One bundle, one format, one identity

```rust
pub struct Bundle {
    format: Format,              // enum Format { V1 }: the only variant; any other value is Load(FormatMismatch)
    compiler: Commit,            // the Loft commit that wrote it (Keel's PROTOCOL habit)
    resources: Box<[Resource]>, kernels: Box<[Kernel]>, modules: Box<[Module]>, draws: Box<[Draw]>, passes: Box<[Pass]>,
    media: Box<[Medium]>, cameras: Box<[(NameId, CameraId)]>, params: Box<[ParamDecl]>, inputs: Box<[InputDecl]>,
    sites: SiteMap, sections: Sections,
}
pub struct Module { wgsl: SectionId, origin: Origin /* Library | Author */, precision_alias: bool }

pub struct Identity { build: [u8; 32] /* sha256 of the bundle file */, frame: FrameKey, device: Device }
pub struct FrameKey { camera: NameId, time: u32 /* f32 bits */, size: [u32; 2], params: Box<[u32]>, inputs: Box<[u32]>,
                      precision: Precision /* what ran, not what was asked */ }
pub enum Precision { F32, Mixed }
```

- **One file.** `scene.loft` holds a 16-byte header (magic, `format`, JSON length), the serde JSON of `Bundle`, and
  64-byte-aligned binary sections for tables and WGSL text. A file works everywhere: Keel's `include_bytes!`, a
  `fetch` in the browser, a training request, and one hash. The `lower.ts` translation step is deleted, because
  the bundle *is* the in-memory artefact. Closures (`cpuPose`, `pose`) and the `js-tables` identity variant are
  gone, because the recorder cannot produce them.
- **One identity.** `build` is the hash of the file. Variant and style change the file, so they are inside it. The
  `tier` discriminant (only ever `"compiled"`) and the three spellings of precision are deleted. Contract: the same
  `Identity` gives the same frame bytes on that device.
- **Precision is a parameter.** The emitter writes every mixed-capable module once, using `lf` for its low-precision
  floats. At load the host prepends either `alias lf = f32;` or `enable f16; alias lf = f16;`. The choice depends
  on the request and on `SHADER_F16`. One text, one validator, one site map. Storage stays f32. The f16 palette
  packing (4,096 → 2,048 bytes, MEASURED in `f16.final.md`) is not worth a second layout. `F32Fallback` stops being
  a precision: `FrameKey.precision` records what ran.

### 1.10 The compile pipeline

```
record (TS) ─▶ decode+validate ─▶ inline+intern+grad ─▶ fold (f32) ─▶ rate ─▶ kernels/resources ─▶ schedule
            ─▶ emit (names in first-use order, shared by hash, lf alias, branch-local if, site maps)
            ─▶ layout (bind groups) ─▶ write bundle + identity
analyses on the folded arena: tape (f32 lanes) · interval (f32-padded) · check · solve · evaluate
```

Each step is a function from the arena to the arena, or from the arena to a plan. None reads JS objects, global
memo tables or strings that carry meaning. The host frame is: write the `Frame` uniform, run `Frame` kernels, run
whichever `Steps` kernels the time needs, run passes in order (shadow, offscreen, view, screen), then read back the
LOD bounds and camera asynchronously.

### 1.11 What is deleted outright

- **Replaced by `loft-core`** (about 11.6k TS):
  - the graph half of `expr.ts`, `compileJs`, the 7 op walkers and the 43 `LibFn.js` twins;
  - `grad.ts`, `interval.ts`, `ranged.ts`, the sampling loops in `check.ts`, `check_state.ts`, `mistakes.ts`;
  - the numerics in `geom/solve.ts`;
  - `compile/*`, compute, `recur` and table planning, `host/{compute_data,bake}.ts`;
  - the `draw/*` emitters, `frame/*`, the WGSL in `soft/*`, `relief/trace.ts`.
- **Replaced by `Bundle` and `Identity`:** `artefact.ts`, `host/{manifest,lower,files,precision,share,fixture}`,
  `identity.ts`, the `precision.ts` WeakSet, format versions 1–5, and every byte-preserving optional field.
- **Replaced by `loft-gpu` and `loft-draw`:** `host/{host,detail,camera,still,profile,cache,headless}.ts`, Dawn,
  `bun-webgpu`, the Dawn GPU tests and benches, and `rust/loft-loader` (2.6k; its device sharing, pipeline cache,
  LOD plan and timestamps are moved over, not rewritten).
- **Deleted with no replacement:**
  - `FilledTable`, the `cpu` table path, `NotDataError` and `js-tables`;
  - about 30 module-level `WeakMap` memo tables, which interning and the program cache replace;
  - `jsmath.rs` and `share.rs` from `feat/rust-core`. Its `solve.rs` and `tape.rs` are source material for
    lanes 1.3 and 2.3.

The engine today is about 21k lines. The lean Rust budget is **9.8k** (scc code lines, per lane in §8). The binding,
binary and wasm glue add about 1k. TS keeps:

- the recorder plus the text front end, about 1.4k;
- the CLI, about 1.8k;
- content, about 7.6k.

**DERIVATION:** the audit's §3.3 table, with the five tracers as library hit functions.

---

## 2. The parity gate that allows unification

### 2.1 Why the byte gate must go

Sol's plan requires full SHA-256 equality of the bundle after every lane. To meet that, Rust must reproduce
`lower.ts`'s version selection and field order, the old interner's quirks, JS component order, JSC's `Math` bits,
JS regex matching in sharing, and first-refusal order. Any merge of two parallel paths changes bytes. A byte gate
suits a one-for-one translation, and this port is not one.

### 2.2 The gate

A concern passes when (a)–(d) below all hold on the **corpus**:

- every `examples/**/*.ts` scene (124 paths: 63 build and 61 end in a typed outcome, MEASURED in the rust-core note);
- the 170 migrated creative seeds;
- a fixed list of frames per scene: up to three named cameras × times {0, 0.5, 1.37, 2.9} s at 512×512 (the
  training size), plus one 1280×800 frame per scene for detail and LOD paths. Scenes with `Steps` state add
  t = 6.0 s, which is after many steps.

**(a) Picture parity.** Each frame is compared with its reference frame, per channel in 8-bit:

1. every pixel off a silhouette is within **1/255**;
2. pixels over 1/255 make up at most **0.01%** of the frame (26 of 262,144 at 512²);
3. no 4-connected cluster of such pixels is larger than 4 pixels, and none of them is more than 1 pixel from a
   reference depth discontinuity.

**DERIVATION** (why these numbers, not looser ones). Unification reorders f32 operations: one template instead of
five, folding moved into Rust, a different `select` placement. A reordered f32 expression differs by a few ULP
(about 1e-7 relative). Quantized to 8 bits, the result moves by at most one code, and only when it sits within a
few ULP of a rounding boundary. So any continuous pixel is within 1/255. A larger difference needs a
*discontinuity*: a ray hit that flips, a march step count that changes, or a shadow test at its threshold. These
lie on silhouettes and shadow edges. A ULP-scale shift of the ray moves them by far less than a pixel, so the
flipped set is the edge perimeter times a probability of about 1e-6. That is far below 26 pixels. A difference
that fails rule 3 is therefore a real change of meaning. It is never noise, and the lane fixes the code; the gate
does not move.

**(b) The same verdicts.** Every check's `Truth` is equal. Margins agree within the typed tolerance
`Tol::f32_rel(1e-4)`. Witnesses agree within 0.02 s or 1% of the parameter range. There is one listed exception.
The twin moves from f64 to f32 (§3.4), so a reference margin with `|margin| ≤ Tol` may change from `Holds`/`Fails`
to `Undecided(WithinPrecision)`. The gate report lists each such case with its margin. **CONJECTURE:** the corpus
has no such case. The closest known margins are heron's 52 holds, which are proved with slack. The heron `solve`
must give 52/52 proved holds and spline controls within `Tol::f32_rel(1e-4)`.

**(c) The same typed refusals.** For the 61 non-successes, the `Error` variant, exit code and `Site` (file:line)
are equal. The message text may differ. Each deliberate change is listed in the lane that makes it. One example:
opaque CPU fills disappear, so a scene that relied on one either builds through the rewritten library or gets
`Scene { Unsupported }`.

**(d) Budgets.** Build CPU, peak RSS and GPU frame time meet §8.1. A gate without a cost check would let the port
pass while slow.

### 2.3 Where the reference frames come from

Reference frames are rendered **once**, in wave 0, at a pinned main commit `R0`. They are built by the TS compiler
and drawn by today's **`loft-view` (wgpu 30, Metal, M4)**. The comparison then isolates the compiler and host
rewrite from the Dawn→wgpu switch, because both sides run on Naga and wgpu. A small set of scenes may not draw in
`loft-view` today (CPU tables, Storm). For those the reference comes from `loft render` on Dawn and is compared
under the cross-executor rule of the Metal proof: fewer than 0.01% of pixels over 8/255 and no cluster over 4. The
gate report marks them.

References are lossless PNG (pixel-diff inputs stay lossless). Verdicts and refusals are JSON. All are stored under
`out/parity/R0/` (ignored), together with their manifest and the `R0` commit. **Precondition, MEASURED in lane
0.2:** two `loft-view` runs of the reference give byte-identical PNGs on the M4. If they do not, rule 1 becomes
"within the run-to-run envelope plus 1/255", and the report says so.

The Dawn→wgpu switch itself is not gated here. It was proven before: 98 edge pixels and an 8–14% frame cost
(MEASURED, `8b6249a`). §4.3 deals with the frame cost.

### 2.4 One format, and why no reader for v1–v5 is owed

The project rule is that persisted data is the only compatibility obligation. Here is the persisted data that
touches Loft output:

| Data | Persisted as | What happens |
|---|---|---|
| Creative training seeds (170) and prompts | **TS source code** | `tools/migrate.ts` ports them (feat/migration: 170/170 compiled, 0 hand fixes, MEASURED). The port does not change the authoring API, so the codemod needs at most the renames in §7. Lane 4.3 reruns it and bumps creative's pin |
| Old training runs and their rewards | frozen Loft copies: `frozen-loft` at `d80eaed`/`a3c7612`, vendor pins `b5c8abe`/`e7c3c5a` | reproduce by checking out the SHA. Their artefacts are never read by the new host |
| Keel | nothing: no Keel crate reads an artefact ("nothing moved from LOFT") | the first Keel artefact is a v1 bundle |
| `out/` artefacts | generated | rebuilt |
| `loft serve` answers and verdict JSON | a deployed API with one consumer (creative), which is pinned | creative's reader changes in the same pin bump (lane 4.3), as the identity lane did |

So `Format` has one variant. The host refuses anything else by name: `Load(FormatMismatch { found, expected:
V1, compiler })`. From then on, any incompatible change bumps to `V2` *and* deletes `V1` in the same commit, unless a
persisted consumer exists. Today none does.

---

## 3. The boundary

### 3.1 Kept from sol's plan

These parts of sol's plan are kept:

- TS records and never evaluates graph values.
- A closed program makes one native call. The programs are **Scene**, **Solve**, **Evaluate** and **Check**.
- No per-node FFI, no callbacks into JS, and no Rust pointers in author objects.
- Every bound, tag and quota is validated before any pass.
- A panic becomes `Internal`, and a stale library becomes `Environment`. There is never a quiet fallback to TS.

### 3.2 The wire (pinned here so that lanes 1.1 and 1.2 can run in parallel)

Pinned as implemented on 19 Nov (lane 1.2's writer `src/record/wire.ts`, decoded by `loft-ir/src/wire.rs`; the two
first filled the open parts differently, `agent_notes/2026-11-19_lean_rust_wire.md`). Wire 2 (`feat/lr-wire2`,
`agent_notes/2026-11-19_lean_rust_wire2.md`) adds every remaining decl kind and the tables' fills. A job is one byte
string, all words little-endian:

```
header  u32 magic 'LFTW', u32 wire (= 2, internal, never persisted), u32 program (Scene, Solve, Evaluate, Check),
        u32 n_spans = 11
spans   in this order, each: u32 byte length, u32 0, the bytes, zeros to a multiple of 8; nothing after the last
        nodes   u32×4   [op:8 | ty:8 | flags:16, args offset, n args, imm]       (ty: f32, vec2, vec3, vec4, bool)
        args    u32     operand node ids, in order, each below its user's id
        lits    f64     authored numbers, exact bits, finite
        rows    f32     supplied samples (curve controls, author tables)
        blobs   u8      supplied binary assets
        syms    u8      the UTF-8 text of every symbol, concatenated
        symAt   u32     n_syms + 1 offsets: symbol k = syms[symAt[k] .. symAt[k+1]]
        sites   u32×3   (file sym, line, col)
        siteOf  u32     per node: 0 none, k = sites[k-1]
        decls   u32     [program root count R, then per decl: tag, n roots, n words, words...]
        roots   u32     the R program roots, then each decl's roots in decl order
```

Op codes, one per `loft-ir::Op` variant (flags and imm 0 unless named; a node without args has args offset 0):

| code | op | imm, flags, args |
|---:|---|---|
| 0 | Lit | imm first lit, n = 1 per component (a bool is one lit, 0 or 1) |
| 1 | Time | |
| 2 / 3 | Param / Input | imm the decl's ordinal |
| 4 | Bind / Var | imm the name's sym; flags 1 = `Bind`, a loop's or graph function's binder (named `L3s0`, `L3i`, `F2a1` by its owner), 0 = `Var`, a free variable (u, v, p) |
| 5 | Call | imm a graph Fn's ordinal; args the call's arguments |
| 6 | Fetch | imm a Kernel's ordinal; flags the lane (16 bits: tables reach 632 lanes); args [index] |
| 7 | Loop | imm a Loop's ordinal; flags the state component |
| 8 | Grad | args [f, x]; not written in wave 1 (derivatives arrive expanded) |
| 9 | Raw | imm a raw Fn's ordinal; args its arguments |
| 10 | Swizzle | imm components (2 bits each) \| length << 8 |
| 11 | Vec | args 1..=4 parts |
| 16 + i | `Prim::ALL[i]` | the order of `src/expr.ts` `Fn` and loft-core's `Prim`; 12..=15 refused |

An ordinal counts the decls of one tag, in decl order. Decl tags and layouts (words, then roots):

| tag | decl | words | roots |
|---:|---|---|---|
| 0 | Fn, graph | [0, name, ret ty, n, binder node × n] | [body] |
| 0 | Fn, raw | [1, name, ret ty, text sym, n deps, dep Fn ordinal × n deps]; each dep a raw Fn declared before it | [] |
| 1 | Loop | [n, has stop, state binder × n, index binder] | [count, init × n, next × n, stop?] |
| 2 | Kernel, map table | [0, head, n runs, (count, index binder) × n]; head = name, lanes, entries, role (stations, data, static); runs fill rows end to end | [lanes of run 0, of run 1, …] (vec4) |
| 2 | Kernel, rows | [1, label, offset in rows, length] | [] |
| 2 | Kernel, scan table | [2, head, first, steps, n, state binder × n, index binder]; step j writes row first + j from the state before it | [init × n, next × n, lanes] |
| 2 | Kernel, data table | [3, head, offset in rows] (entries × lanes × 4 f32) | [] |
| 2 | Kernel, recurrence table | [4, head, recurrence sym]: its recurrence is not on the wire yet; the driver leaves it zero and counts it `unfilled` | [] |
| 3 | Draw | [name, layer (subject, surroundings), cull, depth write, blend (opaque, alpha, add), paint Fn, cover…]; cover: 0 grid [u, v, place Fn, count], 1 mesh [kernel, count], 2 quads [place Fn, count], 3 ray box [find, f, 6 lits min max], 4 ray instances [find, f, kernel, 6 lits local], 5 screen []; find 0 hit (f a Fn), 1 medium (f a Medium ordinal) | [] |
| 4 | Pass | [camera, first draw, end draw, clear, ground, outline]: clear 0 load \| 1 colour [4 lits]; ground 0 none \| 1 traced [hit Fn, paint Fn]; outline 0 none \| 1 ink [Fn]. The target is the view: an offscreen or shadow target names a texture resource, which no decl declares yet | [] |
| 5 | Medium | [0 march, density Fn, 6 lits within, steps, shadow steps] \| [1 gaussians, kernel] | [] |
| 6 | Check | [label, 0 prove boxes \| 1 sample n, formula…]; formula pre-order: 0 atom, 1 always / 2 eventually [n, leaf node × n] then the body, 3 and / 4 or [n] then n parts, 5 not then one part | roots in that order: atom [margin, test], quantifier [lo × n, hi × n] |
| 7 | Solve | [label, 6 lits (t0, t1, delta, jerk, acceleration, near weight), n, (name, controls, jet binder × 4) × n, m, unknown × m, h, hold × h]; hold [0 equal, within lit \| 1 bound, times, said sym, site sym], times 0 at [lit] \| 1 during [2 lits] \| 2 span. Jets are the unknown's value and three time derivatives, read by the margins | [from × n, near-to × m, margin × h] |
| 8 | Param | [name, def lit, min lit, max lit] | [] |
| 9 | Input | [name, ty, first def lit] | [] |
| 10 | Camera | [name, 0 look] \| [name, 1 matrix] | look [eye, target, up, fovy]; matrix [4 vec4 columns] |

A `Call` or `Loop` node comes after every root and binder of its decl, so the job is acyclic through decls. Fn,
kernel, medium and camera references of scene-level decls are checked after every decl is read, with each role's
signature (`paint (vec3, vec3, f32) -> vec4`, `hit (vec3, vec3, vec2, vec2) -> vec4`, grid `place (vec2, f32) -> vec3`,
quads `place (f32) -> vec4`, `density (vec3) -> f32`, `outline (vec3, vec3) -> vec4`). A fill's, a role's and a solve's
binders are `Bind` nodes owned by that decl. The decoder classifies each decl into its kind's array
(`Graph::{fns, raws, loops, kernels, params, inputs, draws, passes, media, cameras, checks, solves}`, a table's fill as
`Fill::{Map, Scan, Data, Recurrence}`) and each op into one `Op` variant, so no consumer reads a tag or a flag. The
canonical print (`loft-ir/src/print.rs`, grammar in `src/record/print.ts`) is equal to the TS graph's over the corpus,
fills included, and to the TS job print on `src/record/sample.ts`'s job, which carries every scene-level layout.

There are eleven declaration tags. The render, post and offscreen descriptors of sol's plan are one `Pass`. Tables
and compute are one `Kernel`.

**What the recorder does in TS.** `Expr<T>` stays a handle: `{ id: u32 }` plus the fluent getters, and its TS type
gives the static type. Each constructor appends one node after an O(1) check of each operand's type tag, which
the recorder knows because it wrote that tag. The check throws at the author's call site, as today. Deep
validation moves to Rust. Graph functions, which are `wgsl` `fn`s and library functions declared with
`graphFn(sig, body)`, are recorded **once** with symbolic binders and called with `Op::Call`. Rust inlines them
during interning. Text and call form therefore still meet at the same structural hash, which the authoring API
requires, but at about 25 ns per node in Rust instead of about 100 ns per node of JS construction per call site.
This is the main lever of §8.1. `gridDetail` records one surface over symbolic grid coordinates, and the 25 rest
points become 25 tape lanes (sol's 1b; 27% of Titan's main thread, MEASURED).

### 3.3 Names independent of creation order: the first invariant, not a TS lane

Recorder node ids follow creation order. Creation order depends on module import order, on bytecode versus
source, and on discarded expressions. The emitter numbers generated identifiers by **first use in its ordered walk
from the roots**, with stable scopes per function and per loop. A shared function gets its name from its
structural hash, so its name does not depend on which module includes it. Ids never reach output. This is lane
2.2's first check: three permutations give the same bundle hash (import order shuffled, discarded expressions
added, the library loaded as a bytecode bundle versus source).

I do **not** spend a lane renaming in the TS emitter first, as sol's plan did. Under picture parity the reference
does not need stable names, and the TS emitter is deleted in wave 3. The TS lane would buy about 55 ms of
bytecode-bundle headroom only until wave 3, and the same lever arrives with the Rust emitter.

### 3.4 One arithmetic contract: f32

The twin, the tape, the folds and the table fills compute in **f32**, with transcendentals from the pure-Rust
`libm` crate. Three consequences follow.

- **The twin matches what is drawn.** The GPU renders f32. Checks are about the picture and the margins of motion.
  An f64 twin with JS's operation order was a contract with JavaScriptCore, which nobody consumes.
- **The same bits on macOS, Linux and wasm.** Platform libm differs between OSes. `libm` is the same code
  everywhere, so a cached verdict or a CPU-filled table is portable. `jsmath.rs` and its per-OS bit tests are
  deleted.
- **Intervals become sound for the f32 twin.** Each interval op computes its f64 enclosure and then widens it
  outward by the op's f32 rounding bound: 0.5 ulp for basic ops, and `libm`'s documented ulp bound for
  transcendentals. Proofs are then about the f32 twin, which is what "f32-padded enclosures" in `next.md` §6.5 asked
  for. The GPU still differs from the twin, through FMA contraction and driver transcendentals, and the verdict text
  says "proved for the f32 twin". **DERIVATION:** a sum of per-op relative bounds encloses the rounded evaluation.
  This is textbook outward-rounded interval arithmetic.

`solve`'s optimizer keeps **f64** internally (Cholesky, exchange rounds). It is a numerical method, not a twin. Its
returned curves are verified in f32 by the same `Check` machinery.

### 3.5 Bun FFI, N-API or wasm: N-API, hand-written

**N-API** is chosen, for four reasons:

- **Stability.** Its ABI is stable in both Bun and Node. Bun documents its FFI as experimental, and sol's plan
  already names N-API as the fallback.
- **Zero-copy input.** It borrows the typed arrays directly. Wasm would copy the spans into linear memory.
- **Load cost.** Bun FFI pays dlopen plus TinyCC trampolines, about 7 ms per process (MEASURED). N-API pays dlopen
  only, about 1–2 ms (CONJECTURE).
- **Errors.** A failure arrives as a JS exception with a structured payload.

The f32 plus `libm` contract removes the one argument sol's plan had against wasm, which was platform libm bits.
Wasm still loses: compiling never happens in a browser (§4.2), so it would pay a copy and a slower start for
nothing.

The binding is **hand-written**: about ten N-API functions declared `extern "C"` in `loft-node`, about 250 lines,
the only `unsafe` outside `loft-gpu`. It exports one entry, `run(kind, spans…, out_dir) -> { status, report }`. Big
results (bundles, verdict lists) go to a file the caller names. napi-rs is not used, because its proc-macro stack
costs compile time and dependency weight for one function. The addon is built per platform in
`rust/target/<profile>`. A small version handshake at load refuses a stale library as `Environment`.

---

## 4. One GPU path

### 4.1 Headless: `loft serve`, `render`, `profile` and tools on wgpu

`loft-draw` is a warm Rust process. It reads JSON-lines requests on stdin: bundle path, camera, times, size,
params, inputs, precision. It loads through `loft-gpu` on its own device and writes tightly packed RGBA plus an
`Identity` per frame. The Bun supervisor (`src/cli/serve.ts`) keeps everything it owns today:

- JSON-lines routing;
- the confined, GPU-less compile child, which now does Bun, record and one N-API call;
- slot limits, deadlines, crash disposition and `--check`.

Only the draw child changes, from Bun+Dawn to `loft-draw serve`. `loft render|frames|clip|profile` call the same
binary. The Dawn benches under `tools/` (`gpu_time`, `pose_time`, `tracer_bench`, `serve_bench`) become `loft-draw`
flags or are deleted. `bun-webgpu` leaves `package.json`.

Training on Linux uses Vulkan, with the pipeline cache `loft-loader` already has (feat/pipecache: 0.08–1.28 ms
activation from a retained cache, MEASURED). On Metal, the OS cache applies. The Dawn disk cache lane is dropped
(§7).

### 4.2 The browser viewer: the same host in wasm

`loft dev` (Bun) records and compiles natively through N-API, as the CLI does, and serves `scene.loft`. The viewer
page loads `loft-web`, which is `loft-gpu` compiled to `wasm32-unknown-unknown` with wgpu's `webgpu` backend.
Sliders, time and camera call its exports. Keel's `host_web` already ships this configuration (wgpu features
`webgpu, wgsl`, wasm32, no blocking), so it is proven in this organisation. That is why I choose it over keeping a
thin TS browser host.

A TS browser host would be a second executor, with its own format reader and its own pass order. Keeping one
recreates the "two GPU executors" drift (inventory §3 item 4). The cost is real but bounded:

- `loft-gpu` must load asynchronously, with no `pollster` on wasm. Keel's step 2 needs that anyway.
- The viewer page shrinks from 225 lines to about 100.
- **CONJECTURE:** the wasm build is about 1–2 MB, since wgpu's WebGPU backend is a thin wrapper.

Compiling in the browser is not offered. The viewer always has a local `loft dev`.

### 4.3 The 8–14% wgpu frame gap: a theory to score once

`loft-loader` creates every module with checked `create_shader_module` (`lib.rs:244`). Naga then inserts bounds
clamps on every buffer and array index. Generated Loft code indexes tables in every pose stage and march step.

**CONJECTURE:** the gap is mostly these runtime checks. The prediction is that compiling **compiler-generated**
modules with `create_shader_module_trusted` and `ShaderRuntimeChecks::unchecked()` closes the gap to within ±3%
of Dawn on the Metal-proof scenes. That is sound only because the emitter proves every generated index in range:
`Fetch` clamps by construction, and loop counts are static. Author raw modules (`Origin::Author`) stay checked.
The unsafe call lives in `loft-gpu`.

Lane 4.1 scores this with one A/B. If it misses, the next candidate is Metal's compile `mathMode` (Naga's default
against Dawn's), scored the same way. The frame-time gate (§8.1) does not wait on this. It compares against the
current wgpu loader, not against Dawn.

---

## 5. Keel integration

### 5.1 The ten steps (devex report §3.3)

| Step | Who | In the port? |
|---|---|---|
| 0. Where the bridge lives (`keel_loft`, Loft by git rev) | Keel decides | **Loft side yes** (§5.2): Keel links only `loft-gpu` |
| 1. A frame that does not own the frame | Loft | **Yes**, built in from the start. `encode_into(encoder, color, depth, sample_count, layers)` loads and does not clear attachments it does not own. It takes the caller's format, depth and sample count, and pipelines are keyed by them. Layers are chosen at draw time, so no `--subject-only` build is needed |
| 2. A WASM-capable loader | Loft | **Yes**: `Bundle::from_bytes`, async load, per-target wgpu features matching Keel (`metal, vulkan, gles, wgsl`; `webgpu` on wasm; `dx12` behind a feature). `cargo check -p loft-gpu --target wasm32-unknown-unknown` is in the gate |
| 3. Shared pin check | both | **Yes, Loft's half**: `tools/check_pins.sh` compares `wgpu`/`naga` in both locks. Keel adds the mirror script |
| 4. One boss through Loft in `keel_preview` | Keel | No. Unblocked at the end of wave 3 |
| 5. GPU budget per boss | Keel | No. Loft supplies timestamps per pass (moved from the loader) |
| 6. Hot reload | Keel, using a Loft API | **Loft side yes**: `loft_gpu::validate(&bundle)` runs Naga and size limits before any GPU object exists, so a game keeps the old `Loft` on refusal |
| 7. Browser | Keel | No. It uses the step-2 host |
| 8. Many instances | both | **Loft side yes**: `Loft::prepare(encoder, &[Instance])` runs `Frame` kernels per instance, and `Loft::draw_subject(&mut RenderPass, &[Instance])` records subject draws into the *caller's* pass, so there is no render pass per instance |
| 9. Retire Keel's CPU art per boss | Keel | No |

### 5.2 Crate layout under Keel's layering lints

Keel's lints apply to Keel's workspace members, not to dependencies. Even so, Loft adopts the same policy, so that
`keel_loft` sees nothing that would need an exception:

- `[workspace.lints.rust] unsafe_code = "deny"`, with `allow` only in `loft-gpu` and `loft-node`. Every `unsafe`
  block carries a `SAFETY:` comment naming its incident or contract.
- `PipelineCache::open` becomes **safe**. It takes an exclusive `flock` on the cache directory, which turns the
  "only this API writes" contract into an enforced one. Then `keel_loft` calls no `unsafe` function.
- `clippy::float_arithmetic = "deny"` in `loft-ir`, which computes identity, names, hashes and bundle bytes.
  Literals are carried as bits. Float arithmetic is allowed in `loft-core`, `loft-gpu`, `loft-draw` and `loft-web`.
- `clippy::wildcard_enum_match_arm = "deny"` workspace-wide. This is Keel's speed-swarm S8 rule: no `_ =>` on enums
  Loft owns. It is how "one handler per variant, no defensive if-chains" is enforced by the compiler, not by review.
- `tools/check_layers.sh` resolves the real `cargo tree`:
  - `loft-core` and `loft-ir` reach no `wgpu`;
  - `loft-gpu` reaches no `loft-core`;
  - only `loft-gpu`, `loft-draw` and `loft-web` reach `wgpu`.
- Edition 2024 and toolchain 1.99.1, as in Keel.

**wgpu pin:** `wgpu = { version = "=30.0.1", default-features = false }` in `[workspace.dependencies]`. This is exact
and the same as Keel's lock. A bump happens in both repos in one change, and `check_pins.sh` fails the side that lags.

**The contract Loft states in `loft-gpu`'s docs** (Keel's rule): inputs in, pixels out, nothing back. Hit shapes stay
in Keel's sim. A Loft attack is built from the sim's timing, never the reverse.

---

## 6. Devex for Loft

Everything here is copied from Keel, where each mechanism has a dated incident behind it. Loft copies the
incident notes with the scripts.

```
rust/
  Cargo.toml              workspace; members = crates/*; profiles copied from Keel (dev deps opt 3, [profile.lab])
  Cargo.lock              one lock
  rust-toolchain.toml     1.99.1, minimal, rustfmt + clippy, wasm32-unknown-unknown
  .config/nextest.toml    4 threads; groups gpu (max-threads 1); SLOW at 30 s, TIMEOUT at 5 min; flaky-result = "fail"
  crates/{loft-ir,loft-core,loft-gpu,loft-node,loft-draw,loft-web}/   each with tests/it/main.rs (one test binary)
tools/
  dev.sh                  check|lint|test|build|run CRATE [--fast]: pinned toolchain, --locked, rust/target, gate flags
  check.sh                static | quick | full
  lane.sh                 new NAME BRANCH | done NAME | check NAME
  lib/cache-env.sh        target admission (rust/target only; others need LOFT_TARGET_OWNER), sccache on, incremental off
  lib/build_gate.sh       Keel's flock protocol, verbatim, on the SAME files
  lib/gate_flags.sh       clippy flags in one file, so lint and gate share units
.cambium.toml             require_cow = true; skip rust/target, out/
key_learnings/README.md   read first; one line per incident
```

- **Compile slots shared with Keel.** `build_gate.sh` is Keel's roughly 100-line perl `flock` script, copied with a
  header naming its origin. It uses the same `~/Library/Caches/keel-build-gate/slot.0` (HEAVY: gates, builds, at
  full `-j`) and `slot.light` (a single-crate check, lint or test: 4 jobs, `nice +10`). On Linux it uses the same
  path rule as Keel's script. With one machine-wide lock across both repos, a Loft build waits behind a Keel gate
  instead of fighting it: at load 60–200, a 54-CPU-second build took 8 minutes of wall time (Keel, MEASURED). Bun
  tests keep their own user lock (`tests/setup.ts`), unchanged.
- **Long jobs run outside the lock.** Corpus renders, parity runs and serve benches start after the build, under
  `nice -n 15`, with `--wait-quiet`. Every timing JSON writes `{load1, cpus}` at start and end, and a run taken
  under load is marked.
- **Gates.**
  - `static` compiles nothing and runs its jobs concurrently with buffered output and per-job seconds: `tsc`,
    sharded `cargo fmt --check`, `check_pins`, `check_layers`, `check_loader_closure`, and the test-layout check.
  - `quick` maps changed paths: `src/**` to the Bun subset, `rust/crates/X` to X plus its reverse dependencies,
    `docs/**` to nothing. `Cargo.lock`, `package.json` or `bunfig.toml` fall back to `full` and say why. An unmapped
    path is printed, never dropped.
  - `full` is the integrator's, once per wave: Bun suite, nextest, the parity corpus, budgets.
- **Lanes.** `lane.sh new` runs `git cambium add -b BRANCH -name NAME -ephemeral ~/git/_wt/NAME <HEAD>`, with an
  explicit base commit. `done` refuses a lane outside `~/git/_wt/`, a dirty lane, or the primary checkout. It then
  runs `cambium remove`.
- **No seed and no hakari yet.** Loft's dependencies are small (wgpu, serde, sha2, libm, png). Port Keel's seed only
  if a fresh lane spends more than 60 CPU-seconds building third-party dependencies. Adopt hakari only when `-p`
  and `--workspace` resolve different features.

---

## 7. Pending work: fold or drop

| Item | Decision | Reason |
|---|---|---|
| Dawn disk pipeline cache | **Drop** | Dawn is deleted. Vulkan has wgpu's `PipelineCache` (built); Metal has the OS cache |
| Warm confined compile worker | **Drop** (revisit only on the trigger below) | **DERIVATION:** after the port a cold build is 72–257 ms CPU (§8.1), of which Bun start plus library load is 27 ms. A sample spends about 0.8 s of GPU on 16 frames (`next.md` 6b). The worker would save at most 27 ms, about 3% of a sample, and confinement would need a fresh module registry per job. Trigger: training shows compile CPU above 25% of sample time |
| `solve` cache across processes | **Fold**, generalised to a **program cache** in lane 2.3 | Key = sha256(recorded spans ‖ options ‖ core commit), which is canonical because recording is deterministic (§3.3). Value = the program's result: curves and verdicts, or a bundle. It covers the solve cache *and* `next.md`'s "exact structural cache". Heron's 87 ms solve falls to 0 on a hit |
| `steps` baked at build | **Drop** | A bake moves the same GPU work from load to build. It saves only when a bundle loads more than once, and it puts a GPU into the GPU-less compile child. Const-rate kernels are already filled at compile on the tape (§1.4) |
| Branch-local `if` | **Fold** into emit (lane 2.2) | A fresh emitter places nodes used by one side of a `select` inside a real `if`. Values are unchanged (the graph is pure), so picture parity holds, and a skipped march costs nothing |
| Training budgets as typed exits | **Fold** into `Error::Budget` (lanes 1.1 and 4.2) | Compile budgets are counted in **work units** (nodes, tape instructions, interval boxes), not wall time, so a budget refusal is deterministic. Frame budgets are measured GPU time in `loft-draw` |
| f16 `--precision mixed` | **Fold** as the `lf` alias (§1.9) | It costs about 40 lines and has no second shader. It stays opt-in. The 2 KB storage packing is dropped |
| Tier I (GPU interpreter); faster TS graph construction | **Drop** | **DERIVATION:** tier I's ceiling was the warm 0.3–0.6 s of driver work, and a folded march interprets more slowly than it compiles. After the port, compile CPU is 0.07–0.26 s and pipeline caches cover driver work. The recorder replaces TS graph construction |
| TS naming lane | **Drop** | §3.3 |
| f32-padded enclosures (`next.md` 6.5) | **Fold** (§3.4, lane 1.3) | It follows from the f32 contract |
| Creative migration (`tools/migrate.ts`, feat/migration) | **Fold**: the integrator lands it in wave 0, and lane 4.3 reruns it and bumps creative's pin | Seeds are source code. The codemod scored 170/170 |
| `context-version`, `training-host-contract` branches | **Fold** | Identity carries version and commit. Cameras are resolved at compile into the bundle |
| L8 `loft context` (dirty worktree) | Land as-is in wave 0 if its own check passes, otherwise drop | It is authoring-facing and independent of the port |

**A freeze rule for wave 0 onward:** no new feature lands in the TS engine (`src/compile`, `src/draw`, `src/host`,
`expr.ts`, `interval.ts`, `ranged.ts`, `check.ts`, `geom/solve.ts`). On 18 Nov, 9.5k TS lines landed on the day the
owner asked for Rust (inventory §3 item 7). A feature request during the port is written against the Rust types in
this document and lands in its lane.

---

## 8. Lane plan

### 8.1 Derived targets (the pass/fail numbers)

This rescores sol's cost model with three changes:

- recording graph functions once with `Call` (§3.2);
- sharing by structural hash instead of a text scanner;
- N-API in place of Bun FFI.

The per-unit rates are sol's. U = 16.4k/182.9k/39.8k (MEASURED created nodes). R = recorded nodes, where sol
assumed 4U/6U/4U.

| Phase (ms CPU, cold, GPU-less) | Owl | Titan | Heron |
|---|---:|---:|---:|
| Bun 10 + N-API ~2 + bytecode library 15 | 27 | 27 | 27 |
| Scene module link (sol's allowance) | 8 | 20 | 10 |
| Callbacks + recorder: sol's base + 100 ns/R, with R = 3U / 0.55M / 3U | 16.9 | 87 | 31.9 |
| Handoff, 1 ms + 28 B/R at 5 GB/s | 1.3 | 4.1 | 1.7 |
| Decode + intern + inline: 35 ns/R + 25 ns/U | 2.1 | 24 | 5.2 |
| Rewrite, fold, bind, rate: 8 visits/U at 24 ns | 3.1 | 35 | 7.6 |
| Emit: 2 statements/U at 55 ns | 1.8 | 20 | 4.4 |
| Sharing by hash plus layout: 2 ms + 1 visit/U at 10 ns | 2.2 | 3.8 | 2.4 |
| Rest and table fills on the tape | 4.7 | 22 | 7.1 |
| Bundle, hash, write, CLI | 4.5 | 14 | 4.8 |
| Solve (cold; 0 on a program-cache hit) | 0 | 0 | 87 |
| **Projected** | **72** | **257** | **189** |
| **Gate (owner)** | **≤ 80** | **≤ 300** | **≤ 200** |

**DERIVATION** with one **CONJECTURE**. The Titan recorded count R ≈ 0.55M assumes that recording library graph
functions once at least halves sol's 1.1M, which included per-call-site expansion in JS. Lane 1.2 scores it first,
by counting R for all three scenes.

**Kill condition:** if R_Titan > 0.75M after lane 2.4 (library graph functions), Titan cannot meet 300 ms under
this model. The next lever is the warm worker (−27 ms), and the decision goes back to the owner with the counts.

Owl's margin is 10% and heron's is 5%. Heron's margin depends on the solve, whose Rust kernel was 58 ms (MEASURED),
plus derivatives and setup now in Rust.

**Peak RSS gate** (sol's derived upper bounds, which assumed a larger R): Owl ≤ 101 MiB, Titan ≤ 187 MiB,
heron ≤ 136 MiB. Today these are 152, 492 and 236 MiB on the partial-Rust path (MEASURED).

**Frame time gate:** p95 GPU frame time of the new bundle on `loft-gpu` is at most that of the R0 artefact on
today's `loft-loader`, plus 3%, at the gate's 1280×800 cameras. Load is recorded and loaded runs are marked.

Each lane also has a **line ceiling** (scc code lines). Going more than 20% over requires a note in the lane's
journal saying what grew and why it could not be cut.

### 8.2 Waves

GPU class: **none**; **light** (one device, a few renders); **heavy** (corpus renders or benches). At most 4 heavy
lanes run at once. Workers run focused checks only. The integrator merges each lane through its check and runs one
`check.sh full` per wave. The integrator alone owns `rust/Cargo.toml`, `loft-core/src/lib.rs` (the program
driver), `src/cli/main.ts` routing and the `LOFT_ENGINE` switch.

`LOFT_ENGINE` takes `ts | rust | shadow`. It is classified once at the CLI boundary into a sum type, and any other
value is a `Usage` error. `shadow` runs both and diffs. It exists only for the corpus tools.

#### Wave 0: ground (main stays on TS)

| Lane | Owns | Lines | GPU | Pass / fail |
|---|---|---:|---|---|
| **0.1 devex** | `rust/Cargo.toml`, `rust-toolchain.toml`, `.config/nextest.toml`, `tools/{dev,check,lane}.sh`, `tools/lib/*`, `.cambium.toml`, `key_learnings/`; moves `rust/loft-loader` into `rust/crates/` unchanged | ~700 sh/toml | none | Keel's A1–A12 checks: one lock and wgpu resolved once; `rustup show` gives 1.99.1; a second lint compiles 0 units; `CARGO_TARGET_DIR=/tmp/x tools/dev.sh check loft-loader` exits 2 with the reason; while a Keel gate holds HEAVY, `dev.sh build` prints the holder and waits; `static` wall time is below the serial sum; `lane.sh done` on the primary checkout refuses |
| **0.2 parity** | `tools/parity/{corpus.json,run.ts,diff.rs→loft-draw diff later}`, `out/parity/R0/` | ~800 | **heavy** | The reference set exists for all 124 scenes plus 170 seeds, at the stated frames. Two runs give byte-identical PNGs (or the envelope is recorded). Verdict and refusal JSON hold the full typed payload. The comparator implements §2.2 and fails a planted 2-pixel-wide shifted edge |

Integrator tasks: land `tools/migrate.ts` and `docs/MIGRATION.md` from `feat/migration`; land or drop L8; declare
the freeze rule in `AGENTS.md`.

#### Wave 1: the IR, the recorder and the math (no GPU)

| Lane | Owns | Lines | GPU | Pass / fail |
|---|---|---:|---|---|
| **1.1 ir + binding** | `crates/loft-ir/*`, `crates/loft-node/*`, `src/core/native.ts` | ≤ 1,300 + 400 | none | Wire decode refuses 10⁵ fuzzed malformed jobs with typed errors and no panic. Bundle write→read→write is byte-identical. Changing any `Identity` field changes the hash. `float_arithmetic` and `wildcard_enum_match_arm` are clean, and `check_layers` is green. A 1M-node job crosses N-API in < 5 ms of overhead. A Rust panic becomes `Internal` without killing Bun. A missing or stale addon becomes `Environment`. The same addon passes under the pinned Bun and Node 22 |
| **1.2 recorder** | `src/record/*` (new); `src/expr.ts` constructors call the recorder; `src/text/lower.ts` writes `Call`s | ≤ 1,400 TS | none | Under `shadow`, for all 124 scenes, Rust decode, inline and canonical print equals the canonical print of the TS graph from the same roots (integrator runs this after 1.1 merges). Recorded spans are byte-equal under shuffled imports and bytecode versus source. **Scores R** for Owl, Titan and heron against §8.1 |
| **1.3 math** | `crates/loft-core/src/{prim,tape,grad,interval}.rs` (rows of `PRIMS`; `tape.rs` from feat/rust-core as source) | ≤ 1,700 | none | (a) Tape against TS `compileJs` on every `tests/expr*` fixture and every `LibFn`: within `Tol::f32_ulps(4)`, with any wider op listed by name and bound. (b) Interval soundness: 10⁶ random boxes × 16 points, zero tape values outside. (c) Grad against central differences, rel 1e-3. (d) The same tape bits on macOS and on the Linux training host |

#### Wave 2: the compiler middle and the analyses (cutover: CPU analyses)

| Lane | Owns | Lines | GPU | Pass / fail |
|---|---|---:|---|---|
| **2.1 kernels** | `loft-core/src/{rate,kernel,schedule,layout}.rs` | ≤ 1,000 | none | For the corpus, the `Kernel`/`Resource` plan accounts for every TS table, `recur` and compute: the same rows and lanes, `Steps` keep the same history, and the number of per-frame dispatches is ≤ TS's. Const-rate fill bytes equal the TS f32-encoded fills within 1 ulp |
| **2.2 emit** | `loft-core/src/emit/*` | ≤ 900 | light | Every module of the 63 scenes passes Naga validation (no GPU needed). Three permutations (§3.3) give one bundle hash. Shared modules ≤ TS's shared chunk count. The `lf` alias validates both ways where f16 is available. A raw-function typo gives `Scene{Wgsl}` at the author's line |
| **2.3 analyses** | `loft-core/src/{check,solve,cache}.rs` (`solve.rs` from feat/rust-core as source; holds as symbolic graphs, derivatives in Rust) | ≤ 1,400 | none | Same verdicts on the corpus and on `tests/{ranged,solve,hold}`. The tunnelling claw is found (sampled pass, proved fail). `--explain` witness within 0.02 s. Heron: 52/52 proved, controls within `Tol::f32_rel(1e-4)`, solve ≤ 90 ms CPU, a cache hit runs 0 rounds |
| **2.4 library** | TS content: `src/checks/*` (17 checks become measures or `Sample`), `graphFn` on hot library functions (`lib/{noise,fold,sdf*}`, light terms), symbolic `gridDetail`, `soft/medium.ts` and `relief/sculpt.ts` fills as `Map` bodies, delete `FilledTable` | ≤ 1,200 TS changed | none | R_Titan ≤ 0.75M (kill condition, §8.1). Every former check is classified. Picture parity holds on the TS engine for touched scenes (this lane changes recording, not meaning) |

**Cutover (integrator):** `loft check`, `solve()`, `evaluate()` and serve's `--check` run on Rust, through the
recorder and the binding. The TS compiler still builds bundles, and gets solved curves from Rust as numbers.
**Deleted in this wave:** `interval.ts`, `ranged.ts`, the sampling bodies in `check.ts`, `check_state.ts`,
`mistakes.ts`, the numerics in `geom/solve.ts`, `cli/verdict.ts`'s second `Verdict`, and `feat/rust-core`'s
`src/core/*` and `jsmath`/`share`. `grad.ts` and `compileJs` stay until wave 3, because the TS compiler still uses
them.

#### Wave 3: draws, host, worker, viewer (cutover: bundle and GPU execution, atomic)

| Lane | Owns | Lines | GPU | Pass / fail |
|---|---|---:|---|---|
| **3.1 draws** | `loft-core/src/draw/*`, `crates/loft-core/wgsl/*.wgsl` (one `Ray` template, one `Raster` per prim, `Screen`, `Medium` both integrations, pass composer); TS library hit and place functions in `src/draw/*.ts` (rewritten as content) | ≤ 2,000 Rust+WGSL | **heavy** | Picture parity (§2.2a) on every scene that uses a ray, medium or quad drawable. Ground and outline logic appear exactly once (grep gate). Grains dim inside a `draw.volume` by a measured share |
| **3.2 host** | `crates/loft-gpu/*` (moved over from `loft-loader`: device sharing, pipeline cache, LOD plan, timestamps) | ≤ 1,500 | **heavy** | Picture parity on the whole corpus with 3.1. The frame-time gate. `encode_into` draws a subject over a game-owned colour and depth with `LoadOp::Load` and clears nothing it does not own. `cargo check --target wasm32-unknown-unknown`. A bundle with a wrong `format` is refused by name |
| **3.3 executors** | `crates/loft-draw/*`; the `src/cli/serve.ts` draw child; `loft render/frames/clip/profile` routing; `crates/loft-web/*`, `viewer/main.ts`, `src/cli/dev.ts` | ≤ 900 Rust + ≤ 350 TS changed | **heavy** (native and browser GPU) | `tests/serve.test.ts` semantics hold: deadlines, crash disposition, budget exits, `--check`. The same request twice gives the same `Identity` and RGBA. Training's 16-frame canary is ≤ 50 ms GPU per frame (`next.md` 6b). In headless Chrome with WebGPU, the viewer draws three corpus scenes within the §2.2a rule of the native frame, sliders move params live, and a refused rebuild keeps the old bundle |

**Cutover (integrator, one commit):** `LOFT_ENGINE` defaults to `rust`. The full parity gate passes (§2.2 a–d) on
every concern. Then, **in the same wave:**

- delete `src/compile/*`, the `draw/*` emitters, `frame/*`, the WGSL in `soft/*`, `relief/trace.ts`, the graph half
  of `src/expr.ts`, `compileJs`, `grad.ts`, `artefact.ts`, `identity.ts`, `precision.ts`, all of `src/host/*`,
  `rust/crates/loft-loader`, `bun-webgpu`, the Dawn GPU tests and the Dawn benches;
- remove `LOFT_ENGINE`.

The cutover is atomic across compile, serve and viewer. They share one format, and the TS host cannot read
`Format::V1`. If one of the three lanes misses, the cutover waits for it, and main stays on TS.

#### Wave 4: Keel, Linux, performance, cleanup

| Lane | Owns | Lines | GPU | Pass / fail |
|---|---|---:|---|---|
| **4.1 keel** | `loft-gpu`: the instance and layer API (`prepare`, `draw_subject`), safe `PipelineCache`, trusted modules for generated code (§4.3); `tools/check_pins.sh`; `docs/keel.md` | ≤ 800 | **heavy** | `loft-draw` draws five lamp-boss instances in one pass for less GPU time than five loads (timestamps). `check_pins` fails on a planted mismatch against Keel's lock. No `unsafe` in the public API. Scores §4.3's conjecture with one A/B, load recorded |
| **4.2 measure** | `loft-draw profile`, the `Budget` wiring, the `tools/parity` Linux runner, the Vulkan cache in `loft-draw` | ≤ 500 | **heavy** (local, then remote) | Scores §8.1 on the gate machine: three alternating runs each of Owl, Titan and heron; CPU, RSS and phase counts against predictions; a count miss is investigated before any rate is tuned. An over-budget scene exits `Budget` deterministically. On Linux/Vulkan the corpus builds and renders, pixels meet the cross-backend rule against R0 (§2.3), and tape bits equal macOS. The container scales to zero after the run |
| **4.3 cleanup** | `src/index.ts` one entry, deduplicated barrels, the TS error classes mapped to `Error`, CLI trims, `docs/` (DESIGN.md engine sections, `docs/bundle.md` replacing `artefact_format.md`), `loft context` regenerated; creative pin bump via `tools/migrate.ts` | ≤ 1,500 TS changed (net deletion) | light | 170/170 seeds compile and 4-frame smoke hashes are stable on the new pin. `loft context` stays within its budget. Exports with no reference outside their own file are 0 for new code |

Totals: 15 lanes. About 9.8k Rust engine lines plus 1.3k of binding, binary and glue, against about 21k lines
deleted. GPU-heavy lanes per wave: 1, 0, 0, 3, 2. That never exceeds four.

---

## 9. Risks, and the order that keeps main working

**The order.** Main always builds and renders on whichever engine is the default. Each concern switches only when
the Rust path passes the gate for that concern, and the TS code for that concern is deleted in the same wave, so
no second engine survives a wave boundary:

1. **Wave 0.** Nothing switches. The reference is frozen and the TS engine stops growing.
2. **Wave 1.** Rust runs only in `shadow`.
3. **End of wave 2.** The CPU analyses switch. TS checks, ranged and solve numerics are deleted.
4. **End of wave 3.** Bundles and GPU execution switch together. The TS compiler, TS host, Dawn and `loft-loader`
   are deleted.
5. **Wave 4.** Additions and deletions only, each with its own gate.

| Risk | Where it shows | Response |
|---|---|---|
| The reference is not run-to-run deterministic on the M4 | lane 0.2 | the gate uses the measured envelope, and the report says so. This is decided before any port lane runs |
| The unified `Ray` template changes the march step rule, so silhouettes fail rule 3 | lane 3.1 | the step rule is part of the TS library's hit function, which is content, so the library keeps each old rule. Rust never adds a per-kind branch |
| Titan's recorded R misses (CONJECTURE) | lane 1.2 counts, lane 2.4 kill condition | the warm worker, then the owner decides. The model's rates are not retuned to fit |
| An author callback needs a number during recording (a hidden explicit read) | lane 1.2 shadow | the recorder refuses a numeric read of a handle with a named `Scene` error at that site. An author who needs a number uses `evaluate()`, which is its own program |
| f32 twin verdict flips near zero | lane 2.3 | listed in the report as `Undecided(WithinPrecision)`; none expected |
| Bun's N-API differs from Node's | lane 1.1 | the same addon under both is the check. Node is the fallback runtime, not a second binding |
| Naga refuses generated modules (64/294 before rate folding, MEASURED) | lane 2.2 | Naga validation runs at compile with no GPU, so refusals surface in wave 2, not at the cutover |
| The wasm viewer fails in the browser | lane 3.3 | the cutover waits. A thin TS host is not reopened, because it would be the second executor this plan removes |
| Creative training needs Loft mid-port | any wave | creative stays pinned to its pre-port SHA until lane 4.3. The next training run is the owner's call (`NEXT_RUN.md`) |
| Lanes grow back the parallel paths | every review | the line ceilings, `wildcard_enum_match_arm = deny`, no optional fields "for byte preservation", and the grep gates of lane 3.1 |

---

## 10. What is inherited and what is new

**Inherited.**

- From sol's plan: the recorder, the spans, one call per program, closed programs, the arena, the cost model, and
  no quiet fallback.
- From the audit: the types and the line budgets.
- From Keel: the devex mechanisms, the layering checks, the no-wildcard rule and the pin discipline.
- Standard techniques: hash-consing, outward-rounded intervals, robustness margins, and SSA with if-conversion.

**New here.**

- Picture parity, with a derived silhouette allowance.
- The f32 plus `libm` contract, from which f32-padded proofs follow.
- `Call` recording, which moves inlining from JS into Rust.
- Precision as a WGSL `alias`.
- One `Medium` exposing `transmittance`.
- Five tracers as library hit functions over one `Ray` template.
- The program cache keyed by the recorded spans.
- The hand-written N-API binding.
- The wasm viewer as the same host.
- The theory that the wgpu frame gap is runtime bounds checks.
