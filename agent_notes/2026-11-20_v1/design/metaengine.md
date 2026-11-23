# Meta-engine redesign: engines as authored library code over a small compiler core

23 Nov 2026. Single design owner. Written before any code. Base: `origin/main` at `51eec94b` (lean2 merged,
`cad2bf4b`). Predictions are in §6 and are not edited after code lands; results go in a dated section at the end.

**The goal is authoring control, not lines.** The lean2 spike showed that 18,000 is out of reach at the present
feature set. This design is judged by what a person or model can write and change. It keeps every feature and
every witness, and its line prediction is roughly neutral (§6).

**What this design builds on.** A second coordinating session wrote `agent_notes/2026-11-23_meta_engine/DESIGN.md`.
It is uncommitted in the primary checkout. It also ran four lanes that are committed but not merged:
- `probe-door`: schedule functions, and `pass` steps that fill WGSL template holes;
- `cut-tape`: deletes the Tier 0 tape VM;
- `cut-unused`: deletes `plan`, `solver`, `body`/`state` and the other constructs in `docs/spec/CUTS_2026-11-23.md`;
- `probe-rows`: rows read rows.

The integration lane `lane/integrate-meta` will carry them to main. **This design assumes that integration has
landed** and builds on it:
- I keep its insight that one construct (a function over a store at a stage) is implemented five times;
- I keep its read rule (§5.3 there);
- I keep its decision that per-kind entry points stay in the compiler.

I change one of its decisions. Probe A fills template holes with WGSL strings chosen by `if` (`fetch: "let c =
textureLoad(...)"`). That is the escape hatch, not the authoring surface. A stranger cannot tell what is in scope inside
the string, and the checker sees none of it. Here, library passes are typed Loft kernels. Templates stay as the
parity-preserving first implementation of a stock pass until its typed version passes the Picture gate.

Which ideas are inherited and which are mine:

| Idea | Origin |
|---|---|
| Side tables keyed by expression ID instead of a second IR | Inherited: rustc `TypeckResults`. The repository already keys types by expression ID. |
| One evaluator generic over a scalar domain | Inherited from abstract interpretation. The repository already has `Eval<S>` for f32, `Iv` and `TraceIv`. |
| Schedule functions; passes filling template holes | Inherited from Probe A |
| Compile-time parameters in brackets (`kernel k[K, light]`), kinds as values in a schedule `for`, kind adapter calls (`K.hit`), image indexing (`ColorHdr[px]`), library passes as typed kernels | Mine |
| Compiler-free `link(&Bundle) -> Linked` shared by native, browser and inspection | Mine. It extracts the structural half of `loft-wgpu::Schedule::new`. |
| `on tick` passes for authored fixed-tick solvers | Mine. It extends Probe A's `on load` / `on dirty`. |

## 1. What the user writes

Three programs, written before any IR. After each one, the questions a stranger asks, and how the design answers
them. A question that needs documentation rather than the code is marked as a defect, together with its fix.

### 1a. Fork the stock realtime engine: change its lighting pass and its draw order

Every line of `Realtime` is a schedule function in `stdlib/render/`. `loft fork stdlib::engines::realtime` copies the
engine file into the scene directory. The user then changes two places and copies one pass.

```loft
# scenes/harbor/harbor_engine.loft: forked from stdlib/engines/realtime.loft
use stdlib::render::{orbit, cull_indirect, render_bundle, draw_indirect, find_shadow_map, tile_classify, tonemap_aces}
use stdlib::render::shade::{tile_pixel, on_screen, camera_ray, sun_shadow}
use stdlib::integrators::direct::{sun_sky}

pub schedule HarborRealtime {
  orbit(at: [0, 0, 0], radius: 6, height: 1.2, period: 30, fov: 35deg)
  cull_indirect(Drawables)
  render_bundle(Vis64) {
    draw_indirect(Water)                   # changed: water first, so its depth rejects the hull below the line
    draw_indirect(Boat, Pier, Gull)
  }
  find_shadow_map(Casters, size: 2048)
  tile_classify(Vis64, tile: 8)
  harbor_shade(Vis64, sun: [0.5, 0.75, 0.35])          # changed: our own lighting pass
  tonemap_aces(ColorHdr, sky: sky_color)
}

# Copied from stdlib/render/shade.loft (shade_tiles), then changed: a second, unshadowed lamp from the pier.
schedule harbor_shade(vis, sun) {
  for k in Drawables {
    pass shade reads [vis, DepthBuf, ShadowMap, TileLists] writes [ColorHdr] {
      compute(entry: harbor_tile[k], extent: indirect(TileArgs, k), workgroup: [8, 8], sun: dir(sun))
    }
  }
}

kernel harbor_tile[K](sun: d3@World) {
  let px = tile_pixel(K, workgroup_id(0), local_id())
  if !on_screen(px) { return }
  let ray = camera_ray(px)
  let hit = K.hit(Vis64[px], ray)
  let b = K.material(hit.p, hit.n)
  let lamp = [2.0, 1.4, -3.0]@World - hit.p
  let c = sun_sky(b, hit.n, -ray.d, sun, sun_shadow(hit.p), 1.0)
    + eval_bsdf(b, hit.n, -ray.d, dir(lamp)) * ([1.6, 1.1, 0.6] / dot(lamp, lamp))
  ColorHdr[px] = [c, 1.0]
}
```

Questions a stranger asks, read cold:

- *What is `Drawables`, and is `Boat, Pier, Gull` all of them?* `Drawables` is the compile-time set of populated kinds
  that have `trace` or `surface`. A kind that is in neither list is not drawn, and the compiler warns, pointing at
  `render_bundle`, because a populated kind is left out. **Answered by a diagnostic.**
- *What is `k` and what can I call on `K`?* `for k in <kind set>` repeats its block once for each kind, at compile
  time. Brackets mean "fixed when compiled". That is already the rule for space parameters (`fn f[S](...)`). `K.hit`,
  `K.material`, `K.trace` and `K.id` are the kind's own queries. `material` and `trace` are the names the kind body
  already declares. `hit` is the one compiler-made query: the kind's reconstruction of the surface from the
  visibility word. **One new member, `hit`.** It earns its name because no library code can see every kind.
- *Where do tiles come from?* `tile_classify` writes `TileLists` and one indirect dispatch per kind into `TileArgs`.
  `indirect(TileArgs, k)` runs this pass with the work `tile_classify` counted. Both functions are ordinary library
  code beside each other in `stdlib/render/`.
- *Is `ColorHdr[px] = …` racy?* Each invocation writes its own pixel. Reading another pixel of a store that the same
  pass writes is a compile error (E505, the existing ordering diagnostic). It points at the read and suggests a second
  pass or a second target.
- **Defect found and fixed:** the first draft passed `light: harbor_light` as a hole and asked "what signature must
  `light` have?". There is no function-type syntax and adding one would be a new name. Fix: a compile-time function
  parameter (`kernel k[K, light]`) is checked at its instantiation, with the error at the user's argument and a note
  at the call inside the kernel. 1a does not need it, so 1a shows the simpler fork: copy the kernel and change it.

### 1b. A new style pass in library code

```loft
# stdlib/styles/hatch.loft
# Pen-and-ink hatching over the shaded frame: ink on silhouettes, one hatch direction below 35% luminance,
# cross-hatching below 12%. Put it between shading and the tonemap.

pub schedule hatch(hdr, spacing = 6, ink = [0.05, 0.04, 0.06]) {
  pass hatch reads [Vis64] writes [hdr] {
    compute(entry: hatch_px, extent: pixels(hdr), workgroup: [8, 8], spacing: spacing, ink: ink)
  }
}

kernel hatch_px(spacing: f, ink: rgb) {
  let px = global_id().xy
  if !on_screen(px) { return }
  let c = ColorHdr[px].rgb
  let luma = dot(c, [0.2126, 0.7152, 0.0722])
  let id = Vis64[px].x
  let edge = id != Vis64[px + [1, 0]].x || id != Vis64[px + [0, 1]].x
  let along = fract((f32(px.x) + f32(px.y)) / spacing) < 0.5
  let across = fract((f32(px.x) - f32(px.y)) / spacing) < 0.5
  let dark = (luma < 0.35 && along) || (luma < 0.12 && across)
  ColorHdr[px] = [select(c, ink, edge || dark), 1.0]
}
```

A scene uses it with one line in its schedule, `hatch(ColorHdr, spacing: 5)`, placed before `tonemap_aces`.

The same applies one level down, to the BSDF. The bottom-to-top layer fold is spelled three times in Rust today
(`Layout::eval`, `emit_kind_module`, `Eval::eval_layers`), and cut 4 moves it into the material library:

```loft
# stdlib/materials/bsdf.loft (moved from Rust)
# Fold a lobe stack bottom to top: each layer adds its own f·cos and attenuates the stack below it.
pub fn eval_bsdf(b: Bsdf, n: rgb, v: rgb, l: rgb) -> rgb {
  var c = rgb(0.0)
  for lobe in b {                    # one iteration per lobe of the static layout, bottom first
    let r = lobe(n, v, l)            # (f·cos, transmittance to the layers below)
    c = r.rgb + r.w * c
  }
  c
}
```

Questions a stranger asks:
- *Which `ColorHdr` does `hatch_px` read, given that `hatch` takes `hdr`?* **Defect.** The kernel names `ColorHdr`
  while the schedule function takes `hdr`. Fix: a store that a kernel reads or writes is either a parameter or a
  top-level store of the same name. The pass binds `hdr` to the kernel's parameter: `kernel hatch_px(hdr:
  ColorHdr, …)` and `compute(…, hdr: hdr)`. That is how `compute(entry: scatter_particles, grid: Grid)` already binds
  tables. The kernel parameter's type names the store's declaration, which gives its format and extent, and the
  argument picks the instance. The shipped file uses that form.
- *Is `for lobe in b` a runtime loop?* No. A `Bsdf` is a static lobe layout (checked today), so the loop is unrolled
  at compile time, like `for k in Drawables`. A stranger could not know that without being told, so the loop carries
  the comment above, and its diagnostic says "unrolled over the 3 lobes of this material".
- *Where are `Vis64`, `ColorHdr` and `pixels` defined?* In `stdlib/render/frame.loft`, as ordinary `target`
  declarations (`pub target ColorHdr: rgba16float, screen`). These are no longer names known only to Rust.

### 1c. A fixed-tick motion solver authored in stdlib

The approved cuts remove `solver … config` and `constrain` (`docs/spec/CUTS_2026-11-23.md`): "a kernel in a pass loop".
This is that loop, written for the 120 Hz motion clock.

```loft
# stdlib/motion/xpbd.loft
# XPBD distance constraints on the 120 Hz motion clock. Copy this file to change the sweep order, add a constraint
# kind, or change the integrator. Deterministic: the sweep is one thread in one fixed order, so every device and the
# CPU twin agree bit for bit (Det).
use stdlib::motion::solve::{solve_dist_edge}

pub kind XpbdPoint @World (x: p3@World, v: v3@World = [0, 0, 0]@World, prev: p3@World = [0, 0, 0]@World, w: f = 1.0) {}
pub kind XpbdLink (a: XpbdPoint, b: XpbdPoint, rest: m, alpha: f = 0.0, lambda: f = 0.0) {}

pub schedule xpbd(substeps = 4, iterations = 2, gravity = [0, -9.81, 0]@World) {
  repeat(substeps) on tick {
    pass predict writes [XpbdPoint, XpbdLink] {
      compute(entry: xpbd_predict, extent: rows(XpbdPoint), gravity: gravity, h: tick / substeps)
    }
    repeat(iterations) {
      pass sweep writes [XpbdPoint, XpbdLink] {
        compute(entry: xpbd_sweep, extent: 1, workgroup: 1, h: tick / substeps)
      }
    }
    pass settle writes [XpbdPoint] {
      compute(entry: xpbd_settle, extent: rows(XpbdPoint), h: tick / substeps)
    }
  }
}

kernel xpbd_predict(points: [XpbdPoint], links: [XpbdLink], gravity: v3@World, h: s) {
  let i = global_id(0)
  if i < rows(links) { links[i].lambda = 0.0 }
  if i >= rows(points) { return }
  let p = points[i]
  points[i].prev = p.x
  points[i].v = p.v + gravity * (h * select(0.0, 1.0, p.w > 0.0))
  points[i].x = p.x + points[i].v * h
}

kernel xpbd_sweep(points: [XpbdPoint], links: [XpbdLink], h: s) {
  for j in 0..rows(links) {
    let k = links[j]
    let a = points[k.a]
    let b = points[k.b]
    let dl = solve_dist_edge(a.x, b.x, a.w, b.w, k.rest, k.alpha / (h * h), k.lambda)
    let n = dir(b.x - a.x)
    points[k.a].x = a.x - n * (dl * a.w)
    points[k.b].x = b.x + n * (dl * b.w)
    links[j].lambda = k.lambda + dl
  }
}

kernel xpbd_settle(points: [XpbdPoint], h: s) {
  let i = global_id(0)
  if i >= rows(points) { return }
  points[i].v = (points[i].x - points[i].prev) / h
}
```

A scene emits rows and calls the schedule:

```loft
use stdlib::motion::xpbd::{xpbd, XpbdPoint, XpbdLink}
build {
  for i in 0..32 {
    let p = emit XpbdPoint(x: [0.05 * i, 2.0, 0.0]@World, w: select(0.0, 1.0, i > 0))   # the first point is pinned
    if i > 0 { emit XpbdLink(a: p - 1, b: p, rest: 0.05m) }
  }
}
schedule Main { xpbd(substeps: 8)  Realtime }
```

Questions a stranger asks:
- *What is `tick`, and what happens when a frame is late?* `tick` is the motion clock's step, 1/120 s, the same clock
  that laws run on. A late frame runs every due tick before drawing, up to the existing catch-up limit. Both are
  defined once in `stdlib/core.loft`, beside `time`.
- *Does `xpbd_predict` resetting `lambda` in the same pass that writes points race?* No. Thread i writes only
  `links[i]` and `points[i]`, and the compiler proves it with the existing one-writer analysis for kernels. A write to
  `points[k.a]` from a thread that is not k.a is legal only in a `workgroup: 1` pass. **The rule must be in the
  diagnostic text**, because a stranger would ask.
- *`Realtime` inside `Main`?* **Defect.** Is it a call or an import? Fix: an engine schedule is called like any
  schedule function, `Realtime()`. A bare name is an error with that fix.
- *Is the twin automatic?* Yes. Authored kernels already have the Naga-IR CPU twin (`loft-std/compute_twin.rs`). A Det
  `on tick` pass is replayed against it at every checked tick.

### What the three programs need, and nothing more

| Construct | New? | Capability it buys | Where it lands |
|---|---|---|---|
| `schedule f(params)` + calls | exists (Probe A) | stock passes as copyable library code | — |
| `pass … reads/writes { compute(entry: k, extent, workgroup, args) }` | exists | an authored compute pass | — |
| `for k in <kind set> { … }` in a schedule | **new**, 1 grammar rule | per-kind passes (shade, shadow, vis) written in the library | S2 |
| `kernel k[K, f](…)`: compile-time kind and function parameters | **new** (`gen_params` already exists on fns) | one library kernel specialised per kind or per callback | S2 parse, S1 check |
| `K.hit(vis, ray)` | **new** member, compiler-made | reconstruct any kind's surface from the visibility buffer | S2 |
| `target T: format, extent` + `T[px]` read/write | `target` is in the type grammar; indexing is **new** | images as kernel stores | S2 |
| extents `pixels(T)`, `rows(K)`, `indirect(T, k)` | lowers to the existing `DispatchExtent` | screen-sized and indirect work from source | S2 |
| `repeat(n) { … }`, `… on tick` | `repeat` lowers to the existing `PassLoop`; `on tick` is new beside Probe A's `on load`/`on dirty` | authored multi-pass solvers on the motion clock | S2 producer, S3 host |
| `for lobe in b` | **new**, unrolled | the layer fold as library code (cut 4) | S2 |

Seven additions. Each one replaces Rust that only the compiler could change today.

## 2. The typed program

### 2.1 What checking produces

No second IR. The lossless CST and AST stay as they are: spans, edits, `loft fork` and `loft context` keep working
byte for byte. Checking produces one `Typed` record, indexed by `Expr::id`:

- `ty`: the existing type table (`Vec<Ty>`).
- `resolved`: what checking decided that the syntax does not say:
  - `Syntax`: the form is the meaning (literals, operators, `if`, constructors, indexing);
  - `Name(NameRef)`: `Local | Time | Param(slot) | Const{runtime} | World(i) | Fn | Kind | Bake | Scan | Reason |
    EventTag`;
  - `Member(MemberRef)`: `Field | MapCell | MapSide{to_parent} | AtomicLoad`;
  - `Call(Callee)`: `Builtin(id) | Fn | World | Intrinsic(AtomicAdd | EmitEvent | WorkgroupBarrier | SubgroupAdd |
    GlobalId | InscatterJoint)`.

Every consumer reads one exhaustive view, `typed.op(&expr) -> Op<'a>`. It pairs the syntax with the resolution, so each
consumer has **one dispatcher with one arm per meaning**. A pairing the checker should never produce (for example a
`Name` recorded as `Syntax`) panics in `op()` with the expression's span. That makes a checker bug loud instead of a
silent fallback.

Effect-ordered blocks are the AST's statement lists, which are already in order. Effects (law write sets, kernel
atomics, events) remain in `CheckedEffects`. Declaration operands appear as resolutions: the texture in
`sample(Ground, uv)` is `Name(Bake)`, the callee of a user call is `Call(Fn)`, and `light: sun_sky` in a schedule is
`Name(Fn)`.

AD (`ad.rs`) still writes derivative functions as source before checking. The checker types them like authored
functions, so they get entries too. Any later pass that synthesises expressions must check them, which gives them
entries. I keep `ad.rs` symbolic: derivatives must be emitted to WGSL for normals, so a dual-number domain would be a
second derivative path, not a replacement.

### 2.2 How each consumer interprets it

| Consumer | Today | After |
|---|---|---|
| check (`Ck::call`, `Ck::expr_inner`) | resolves overloads, then forgets the result | **producer**. It records `Resolved` at the one site where it decides. Shadowing precedence (a user `fn sample` beats the builtin) is decided here and only here. |
| eval f32 / interval / trace-interval (`Eval<S>`) | re-resolves names (`time`, consts, params, worlds), maps `callee.name` to `Builtin` and re-checks shadowing | one `match typed.op(e)`, generic over the existing `Scalar` domain. The numeric rules per builtin stay per domain; the lookup goes. |
| emit WGSL (`Emitter::expression`) | a guard chain on `callee.name` strings: `"workgroup_barrier"`, `"sky_zenith"`, `!self.p.fns.contains_key(...)`, `WorldCall::resolve` re-run, `is_lobe` re-derived | one `match typed.op(e)`. Emission policy that depends on emitter state (the prepared-light `eval_bsdf` and the baked column) stays as guards inside the matching arm. |
| Tier 0 tape (`Lower`) | a fourth call dispatcher | deleted by `cut-tape`, on the owner's decision. If that decision is reversed, `Lower` becomes a fourth `match typed.op(e)`. |
| AD | symbolic over the AST | unchanged. It runs before check. |
| references (`motion_math`) | its own name list | reads `Callee::Builtin(id)` → catalog `MotionClass` |

**WGSL is not a domain of the evaluator.** The spike (§1) found that this needs symbolic booleans, rows and control
flow, which amounts to building a new IR. Emission and evaluation share the resolution, not the walker.

**What this deletes.** The string-keyed call arms in `Emitter::expression` (about 115 lines go to about 60). The name
re-resolution chain in emit and eval. `Eval::builtin`'s name-to-id mapping and shadow checks. `references::
motion_math`. `is_lobe`/`WorldCall::resolve` re-derivation in the emitter. With the tape already cut, these are three
of the spike's four dispatchers. The spike's 700–1,000 figure assumed the tape was a fourth consumer. Without it the
honest prediction is in §6.

## 3. The engine as plan producer

### 3.1 Stage 0 builds the plan from authored declarations

A scene's schedule is already evaluated at stage 0, and Probe A inlines schedule functions there. After this
redesign stage 0 evaluates the schedule into an ordered list of **plan steps**, one sum type:

```text
Step = Store(table | target)                      # declared extent: rows, screen, screen/n, fixed
     | Pass { name, reads, writes, clock: Frame | Load | Dirty(params) | Tick, run }
     | Repeat(n, [Step])
run  = Kernel { entry instantiated with its [K, f] arguments, extent, workgroup, bindings by name }
     | Template { wgsl, entry, extent, holes }      # Probe A's form, kept as the parity escape
     | Raster { draws: [kind draw entries in authored order], colors, depth }
     | Laws { names }                               # run_laws: the fused fixed-tick dispatch, unchanged
     | Mips(target)
```

Lowering (`bundle::lower_program`) turns these steps into the **existing** `TableDecl`, `ModuleBlob` and `PassDecl`
records. It resolves symbolic extents against the compile's target size and device features. It keeps format and role
precision, binding reflection, initialisation and E505 ordering in the linker. **The LOFTB001 wire does not change.**
One descriptor tag is added: the clock `Tick`, tag 2 in `Execution` (§4, frozen in S0). An old reader rejects a
bundle that uses it, which is the right behaviour.

Static specialisation:
- `for k in Drawables` expands once per kind.
- `kernel f[K, light]` is instantiated once per distinct argument tuple. The instance is named, for example
  `harbor_tile[Water]`, so `loft show` and provenance read naturally.
- `for lobe in b` unrolls over the static lobe layout.

The per-kind shader count therefore stays one module per kind, the property that `WHY_LOFT_LANG` §1 measures.

### 3.2 What moves from Rust to `stdlib`

| Stock behaviour | Rust today (lean2 inventory) | Library file after |
|---|---|---|
| camera (`orbit`, `use_camera`) | `schedule.rs` verb arms, `program.rs` `Orbit::eye` | `stdlib/render/camera.loft`: a kernel writing `CameraSlot` |
| visibility (`render_bundle`, `draw_indirect`, cull) | `bundle.rs` 504–693, `lower_passes.rs` 37–48, `stage0.rs` 143–180 | `stdlib/render/visibility.loft`. Probe A moved cull. |
| shadow map (`find_shadow_map`) | `bundle.rs`, `native/shadow.rs` reuse policy | `stdlib/render/shadow.loft` (`for k in Casters`). The reuse policy becomes `on dirty(Casters)`. |
| tiled shading (`shade_tiles`), the `light:` callback | `shade_schedule.rs`, `emit/shade_tiles.rs` 61–110, `emit.rs` 826–945 | `stdlib/render/shade.loft`: `shade_tiles` + `shade_tile[K, light]` |
| full-screen integration (`screen_pass`) | `emit/paths.rs` 203–245 | `stdlib/render/screen.loft` |
| material layer fold, GGX energy lookup | `material_shape.rs` 48–79, `emit.rs` 765–807, `eval.rs` 1125–1155, `check/bsdf.rs` 9–55 | `stdlib/materials/bsdf.loft` (`eval_bsdf`, `ggx_energy`) |
| display (`tonemap`, `tonemap_aces`, `outline_and_paper`) | moved by Probe A, except `outline_and_paper` | `stdlib/render/display.loft` |
| fixed-tick solvers | `solver`/`constrain`, removed by `cut-unused` | `stdlib/motion/xpbd.loft` (1c) |

**What stays in Rust, and why** (the honest kernel, as in DESIGN.md I8):
- store layouts and lifetimes;
- lowering of typed functions to entries, and hazard ordering;
- atomics;
- the per-kind adapters `K.hit`/`K.material`/vis/shadow raster entries, because the set of kinds is open;
- the world/BVH module;
- proofs: units, spaces, intervals, Lipschitz, AD;
- the twins;
- the runner.

Stock engines change from Rust verbs to library calls with **the same names, arguments and defaults**. Every scene
and engine file compiles unchanged (prediction P6). A scene that copied a stock schedule keeps overriding it, because
a user's schedule function shadows the library's by the ordinary import rule.

## 4. The host plan reader

`loft_std::linked::link(&Bundle) -> Result<Linked, LinkError>` (frozen in S0) is compiler-free, wgpu-free and
naga-free. It validates the incoming records once:
- pass IDs are distinct, including inside loops;
- modules are unique and WGSL;
- every binding names a declared table, sits in the group its access requires, and occupies a unique slot;
- extents reference declared tables, and offsets are 4-aligned;
- draws use storage vertex pulling, their arguments exist, and index tables are U16/U32 words;
- a raster pass has a target and 1 or 4 samples;
- the execution descriptor decodes, and its views and initialisers name real passes and slots.

It then materialises the plan once:
- the ordered steps (`Run`/`Repeat`);
- each pass's slots, with their buffer views;
- its work (`Dispatch`, `Ticks`, `Draws`, `Mips`);
- its clock (`Frame`, `Load`, `Dirty`, `Tick`);
- whether it is culled from the front.

Errors are typed variants that name the pass and the slot. Today these come out as one catch-all, `AbiMismatch`.

| Host | Consumes `Linked` and adds |
|---|---|
| native (`loft-wgpu::Schedule::new` → `loft-native::Loaded::load`) | naga reflection of the used bindings against the slots, wgpu layouts and pipelines, device limits, timestamps, the outbox, and the tick loop for `Tick` passes |
| browser (`loft-wgpu/src/web.rs`, `web/threejs_demo/runner.js`) | a projection check against `HostCaps::WEB_BASELINE` (8 storage buffers per stage, no ray query, no MetalFX), then the same native path on the injected device. The host shader callback is kept. |
| inspection (`loft show --plan`, tests, `loft-verify`) | printing and asserting plans with no device |

Row-count dispatch keeps its dynamic semantics, and epoch checks stay where their inputs become available.
`Schedule::new` loses its structural half (about 90 lines). `loft-std::bundle::{distinct_passes, passes, children}`
fold into `link`.

## 5. The slices

### 5.0 Order and preconditions

```text
S0 interface (this branch, merged now: additive files only)
   │
   ├─ wait: lane/integrate-meta lands on main (probe-door, cut-tape, cut-unused, probe-rows, merge-2026-11-23)
   │
S0b prep (one mechanical commit on that main, by the lead, before any slice starts; §5.4)
   │
   ├── S1 typed program ─┐
   ├── S2 plan producer ─┼── in parallel, disjoint files
   └── S3 host reader ───┘
   │
integrate: S3 → S1 → S2   (host first: it consumes only frozen types; S2 last: it is largest and uses S1's Typed)
```

**Why S0 lands before the integration and S0b after.** S0 only adds files (`loft-check/src/typed.rs` and
`loft-std/src/linked.rs`), one `pub mod` line each, and one variant in `loft-ir/src/execution.rs`, a file that no
pending lane touches. It therefore merges cleanly either way. S0b moves code between existing files that the pending
lanes edit heavily (`emit.rs`, `eval.rs`, `check.rs`), so it must run on the integrated tree. Lean1's failure was
exactly that kind of overlap.

### 5.1 Frozen interfaces (written in S0, Rust)

```rust
// crates/loft-check/src/typed.rs
pub struct Typed { pub types: Vec<Ty>, pub resolved: Vec<Resolved> }
impl Typed {
    pub fn new() -> Self;
    pub fn ty(&self, e: &Expr) -> &Ty;
    pub fn set_ty(&mut self, e: &Expr, ty: Ty);
    pub fn resolve(&mut self, e: &Expr, r: Resolved);
    pub fn resolved(&self, e: &Expr) -> &Resolved;
    pub fn op<'a>(&self, e: &'a Expr) -> Op<'a>;
}
pub enum Resolved { Syntax, Name(NameRef), Member(MemberRef), Call(Callee) }
pub enum NameRef { Local, Time, Param(u32), Const { runtime: bool }, World(u32), Fn, Kind, Bake, Scan, Reason, EventTag }
pub enum MemberRef { Field, MapCell, MapSide { to_parent: bool }, AtomicLoad }
pub enum Callee { Builtin(Builtin), Fn, World, Intrinsic(Intrinsic) }
pub enum Intrinsic { AtomicAdd, EmitEvent, WorkgroupBarrier, SubgroupAdd, GlobalId, InscatterJoint }
pub enum Op<'a> { /* one variant per (syntax, resolution) pair; see the file */ }

// crates/loft-std/src/linked.rs
pub fn link(b: &Bundle) -> Result<Linked<'_>, LinkError>;
pub struct Linked<'b> { pub bundle: &'b Bundle, pub execution: Execution, pub steps: Vec<Step>,
                        pub passes: BTreeMap<PassId, LinkedPass<'b>> }
pub enum Step { Run(PassId), Repeat(u32, Vec<Step>) }
pub struct LinkedPass<'b> { pub decl: &'b PassDecl, pub slots: Vec<Slot>, pub work: Work<'b>,
                            pub clock: Clock, pub cull_front: bool }
pub struct Slot { pub group: u8, pub binding: u32, pub table: TableId, pub access: BindAccess,
                  pub view: Option<(u64, u64)> }
pub enum Work<'b> { Dispatch { module: ModuleId, entry: &'b str, extent: &'b DispatchExtent },
                    Ticks { module: ModuleId, entry: &'b str, instances: TableId, max_ticks: u32, det: bool },
                    Draws { draws: &'b [IndirectDraw], index: Vec<Option<IndexWidth>>, colors: &'b [TargetDesc],
                            depth: Option<&'b DepthDesc>, samples: u8 },
                    Mips(TableId) }
pub enum Clock { Frame, Load, Dirty(Vec<usize>), Tick }
pub enum LinkError { /* one variant per refusal, each naming its pass, table or slot */ }

// crates/loft-ir/src/execution.rs: one added variant, wire tag 2, no version change
pub enum Clock { Load, Dirty(Vec<usize>), Tick }
```

**Freezing rules.**
- S1 owns `typed.rs` after S0. It may add variants to `NameRef`, `MemberRef`, `Intrinsic` and `Op`.
- S1 may not rename or remove `Typed::{ty, op}`, `NameRef::Fn`, `NameRef::Kind`, `Callee::Fn` or `Op::{Call, FnValue,
  KindRef}`. Those are what S2 reads.
- S3 owns `linked.rs`. It may add `LinkError` variants and fields that nothing else reads. It may not change `link`,
  `Step`, `Work` or `Clock`, which S2's tests use.
- Nobody changes `loft-ir/src/schema.rs`.

### 5.2 Ownership

| Slice | Owns (and only these) | Consumes | Produces |
|---|---|---|---|
| **S1 typed program** | `loft-builtins/**`; `loft-check/src/{check.rs, typed.rs, references.rs, infer.rs, kernel_units.rs, world.rs, bsdf.rs}`; `loft-eval/src/{eval.rs, lip.rs, refine.rs, ad.rs, lib.rs}`; `loft-emit/src/{emit.rs, emit/column.rs, provenance.rs}` | `Typed` (S0) | `Typed` filled by check; eval and emit dispatch on `Op`; the S2 hook functions in `check/frame.rs` *called*, not edited |
| **S2 plan producer** | `loft-parse/**`; `docs/spec/loft.lark`; `loft-check/src/{defaults.rs, source.rs, source/scope.rs, frame.rs}`; `loft-passes/**`; `loft-emit/src/{emit/kind.rs, emit/shade_tiles.rs, emit/paths.rs, emit/lighting.rs, emit/surface.rs, material_shape.rs, bake_tex.rs}`; `loft-eval/src/layers.rs`; `stdlib/**`; `engines/**` | `Typed::{ty, op}`, `NameRef::{Fn, Kind}`; the `Emitter` public API frozen in S0b; `Linked` in tests | plan steps → the existing `Bundle`; `Clock::Tick` initialisers |
| **S3 host reader** | `loft-std/**` (except `linked.rs` signatures); `loft-wgpu/**`; `loft-runner/**`; `loft-native/**`; `loft-cli/**`; `web/**`; `tools/loft-verify/**` | `Bundle` (frozen), `Linked` (S0) | native and browser both built on `Linked`; the tick loop for `Clock::Tick` passes; `loft show --plan` |

Anything not listed (`loft-abi`, `loft-format`, `loft-ir`, `loft-gltf`, `loft-plan`, the remaining `loft-check`
files) is frozen for the duration. A slice that needs a frozen file stops and reports. It does not edit the file.

### 5.3 Tests, gates and predicted lines per slice

Counted with `tools/lines.py` (scc, the ratchet rule), as a delta from the S0b base.

| Slice | Tests it adds or changes | Gates before its merge | Predicted Rust added / removed / net |
|---|---|---|---|
| S0 | `loft-std/tests/linked.rs`: link accepts every in-tree `.loftb` produced by the corpus build, and rejects one mutant per `LinkError` variant | crate checks; workspace check + tests; Picture 19/19 and Twin 36/36 unchanged (no behaviour change) | +380 / −0 / **+380** (the test lives in `tests/` and is not counted) |
| S0b | none (moves only) | `tools/lines.py` total unchanged ±10; workspace tests identical names and results | ±10 |
| S1 | update the diagnostic snapshots only if a span moves (prediction: none move) | grammars 330/330; diagnostic snapshots identical; Picture 19/19 with **byte-identical module text** for all modules (emission must not change); Twin 36/36; Truth 34/34 | +170 / −420 / **−250 ± 100** |
| S2 | one fork test per program in §1: the user's copy changes the shader or state; the stock engine's module text is unchanged | grammars; Picture 19/19 (deterministic tolerance; module text identical for template-backed stock passes); Twin 36/36; Truth 34/34; Speed/Load paired, neutral or better; pipeline and module counts per gate scene ≤ the base | +520 / −880 / **−360 ± 150**; stdlib .loft **+600 ± 150** |
| S3 | the runner, wgpu and native tests that exist; one browser projection test against `WEB_BASELINE` | wasm32 check; Picture; Twin; Load neutral | +160 / −280 / **−120 ± 60** |
| **Total** | | | **≈ −350 ± 250** counted Rust, plus +600 Loft |

### 5.4 S0b: the mechanical prep commit

This commit changes no behaviour. It moves code so that ownership is disjoint. It runs on main after
`lane/integrate-meta` lands.
1. `loft-emit/src/emit.rs` → `loft-emit/src/emit/kind.rs`: `emit_kind_module` and its private helpers (lean2
   ranges 727–945 and 973–995, re-found after integration). `emit.rs` keeps `Emitter` and its
   `expression`/`stmt`/`expr` methods. Freeze `Emitter::{new, expr, stmt, function}` as `pub(crate)` signatures used by
   `kind.rs`.
2. `loft-eval/src/eval.rs` → `loft-eval/src/layers.rs`: `Eval::eval_layers` and its helper, as an `impl` block in the
   new file.
3. `loft-check`: add `src/frame.rs` with the S2 hook signatures, called from `check.rs`. Each hook returns the
   current diagnostic. The hooks are:
   - `index_target(ck, base_ty, index) -> Res<Ty>`;
   - `kind_member_call(ck, kind, query, args) -> Res<Ty>`;
   - `instantiate(ck, kernel, args) -> Res<()>`.
4. Replace every `types: &mut Vec<Ty>` parameter in the workspace with `typed: &mut Typed`, reading `typed.types`
   (mechanical, about 30 sites).

## 6. Predictions, risks and kill conditions

Written before any code.

| # | Prediction | Falsified if |
|---|---|---|
| P1 | S1 leaves **every** emitted module byte-identical across the 45 gate/corpus scenes and 11 stdlib engines | any module text hash changes |
| P2 | S1 deletes ≥300 counted Rust lines and adds ≤200 | net worse than −100 |
| P3 | S2: stock engines keep byte-identical module text while backed by templates. A stock pass converted to a typed kernel moves ≤1/255 on <0.1% of pixels, with identical vis IDs and D32 | any template-backed pass changes text; a converted pass exceeds tolerance |
| P4 | S2: pipelines and modules per gate scene do not increase (Hermit ≤ its base count) | any count rises |
| P5 | Each program in §1 compiles as written after S2. Each produces a visible change against stock (1a: a different water/hull depth order and a lit pier side; 1b: hatching; 1c: a hanging rope whose Det replay matches 481/481 ticks against the twin) | any program needs a construct not in the §1 table |
| P6 | No existing scene or stdlib engine needs an edit. Token counts are unchanged | any gate scene edited |
| P7 | Speed and Load paired A/B are neutral (CI includes 0) or better for Capsules-, Blobs- and Hermit-1/20 | a paired regression beyond the admitted margin |
| P8 | Picture 19/19, Twin 36/36 and Truth 34/34 hold after each slice merges | any drop |
| P9 | Counted Rust after S0–S3 is the S0b base −350 ± 250 | outside the band |
| P10 | `link` accepts every bundle the present host accepts, and rejects the same mutants (no new refusals) | any corpus bundle refused |

**Kill conditions.**
- **K1:** S0b cannot be made behaviour-free in one commit. Then do not run in parallel: serialise as S3, then S1,
  then S2.
- **K2:** S1 misses P1 or P2. Revert S1 and keep only the catalog `MotionClass` reuse.
- **K3:** S2 misses P4 or P7 on a scene. That pass stays template-backed (Probe A form) until fixed. The library
  still offers the kernel form for user forks.
- **K4:** a slice needs a file it does not own. It stops and reports. The lead decides; the slice never edits the
  file. This is the rule lean1 broke.
- **K5:** two fixes in the same subsystem without a passing gate. Stop, and write the failure class before a third
  attempt.

**Risks.**
- R1: kind adapters exposed as callable queries (`K.hit`) may cost a register-pressure change in the shade entry.
  Mitigation: the template-backed stock pass remains until the kernel form is measured.
- R2: `on tick` passes catching up n ticks cost n × passes dispatches. The xpbd example is 2 + iterations passes per
  substep. Measure Crab Live before making any stock motion use it.
- R3: the integration lane slips or changes the base. S0 is unaffected. S0b and the slices wait.

## 7. What not to do

- Do not cut features: the browser tier, interval proofs, AD, both CPU twins, the glTF import, Three.js interop,
  CLI commands. Tier 0 was cut on the owner's decision (`cut-tape`), not by this design.
- Do not change persisted formats or contracts:
  - the LOFTB001 wire and `schema.rs`;
  - the execution descriptor v1/v2 readers (the only addition is clock tag 2);
  - `.loftlog` replay and identity;
  - the `loft-abi` wire records and golden vectors;
  - the public runner API (`load_bundle`, `upsert_rows`, `set_row_count`, `resize_table`, `hot_swap_*`,
    `push_input`, `encode_into`);
  - CLI exit codes and outputs;
  - `loft context` output;
  - picture pins and gate manifests.
- Do not build a new IR or SSA. Do not make WGSL a domain of the evaluator. Do not adopt dual numbers for AD.
- Do not add function-type syntax, set algebra on kinds, or a template language. Compile-time brackets and a
  schedule `for` cover the three programs.
- Do not move the per-kind adapters, the BVH or the proofs into the library. They are the part a library cannot
  write.
- Do not run parallel slices on files outside their ownership table. Do not start them before S0b.
- Do not touch the `perf-*` branches or the `auncel/*` scenes.
