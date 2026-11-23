# Is Loft a meta-engine? Problem, insights, use cases, a design, and what to delete

23 Nov 2026, second version. Written from the code and the spec, not from earlier lane
conclusions. Nothing here was compiled or timed. Every number marked *predicted* or *estimated*
has a falsifier in §9. Line counts are `wc -l` on `src/` files unless stated; the repo's own
ratchet (`tools/lines.py`, code lines only) reads about 7% lower. Syntax marked `# proposed` does
not parse today.

What changed from the first version: a deletion plan with measured sizes (§7), a scorecard for
"dense" (§8), a corrected first probe (the old P1 was wrong about how the stock shaders are
assembled), a corrected statement about the twin, and decisions where the first version left
options open.

## 1. The problem

A person or a model writes a short `.loft` program and gets a 3D world that looks right, runs
fast and moves well. When the stock answer is not good enough, they must be able to replace
*any* decision, including ones nobody anticipated, without leaving the language, losing speed,
or losing verification.

| Author | Writes | Must never need |
|---|---|---|
| Scene author | 30–200 tokens: kinds, a build, pick an engine | to know how the engine works |
| Engine author | a renderer, a motion system, a simulation, or a fork of one | to edit Rust |
| Kernel maintainer | Rust | to know what a crab or a shadow is |

**Closure.** Every stock behaviour is library source that uses only constructs any engine author
can use. The spec states this as its thesis: "Nothing in a shipped engine is privileged"
(`docs/spec/00` §1).

**Density.** Nothing can be removed without losing a capability someone uses. One implementation
of each contract (`CORE.md`, priority 5).

The first version of this note addressed closure and ignored density. This version treats them
as one problem: the duplication that blocks closure is the same duplication that makes the core
large.

## 2. What the code says today

**2.1 Two worlds that do not meet.**

| | World A: the substrate | World B: the flagship pipeline |
|---|---|---|
| Stores | `struct` + `table(Name, row: T, capacity: N)` | `kind` rows, plus Vis, Hdr, depth, shadow map, tile lists, BVH |
| Compute | `pass … reads [...] writes [...] { dispatch(wgsl, entry, n) }`, typed `kernel … allow(atomics)` | `law` (own row only), about 16 fixed schedule verbs |
| Staging | `bake_tex … on load \| on dirty(param)` | `build` (load), laws (tick), verbs (frame) |
| Assembled by | the scene's own schedule (`authored_compute.rs`, 320 lines) | Rust (`bundle.rs` 822, `pipeline/schedule.rs` 380, `emit.rs` 1,208, …) filling holes in WGSL templates |
| Proof it works | `scenes/particle_flip/sorted.loft`: a radix sort and a FLIP step as 16 authored passes | the five gates |

The portable bundle is one or the other (`crates/loft-passes/src/lib.rs:468`).

**2.2 How the stock shaders are really built** (this corrects the first version). Each built-in
pass is a WGSL template in `stdlib/core/` with named holes (`{{HIT}}`, `{{RESOLVE}}`,
`{{CALLBACK}}`, `{{WORLD}}`, …). Rust picks the template, fills the holes with generated code,
and declares the bindings and the dispatch extent. `render_template` is 20 lines. The private
door is therefore small and specific: *choosing a template, filling its holes, and binding its
resources*. Those three things are what a schedule author cannot write.

**2.3 Spec capabilities the implementation lacks.**

| Spec | Code |
|---|---|
| Laws read other tables: `law roam_step(c, dt, bodies: [BodyRow], …)` | A law takes exactly the row and `dt` |
| A drawn kind reads another table: `TentacleMesh[state_row].ctrl` | not available |
| `shade_tiles(vis, shade: ShadeFn)`, `temporal_accum`, `bloom_mip`, `Previous(x)` | `shade:` only; no history; no bloom |

**2.4 One contract, several implementations.**

| Contract | Implementations today |
|---|---|
| "A pure function over every element of a store, at a stage" | five: `law`, `check`, `bake_tex`, `screen_pass`, `surface` |
| Evaluating a Loft expression | three: the WGSL emitter, the tree-walking evaluator (`loft-eval`), the Tier 0 tape VM (`loft-tier0`) |
| CPU twin of a GPU step | two: `loft-eval` on Loft source, and `loft-std/compute_twin.rs` (768 lines) on Naga IR, plus the replay in `tools/loft-verify/src/trace.rs` (832) |
| Assembling a frame | two: the fixed pipeline and authored passes |
| Declaring a store | three: `kind`, `table(...)` inside a schedule, `bake_tex` |
| Serialising the GPU | two scripts of Loft's own (now forwarded to `agent-build` on branch `tools-agent-build`) |

**2.5 Constructs nobody uses.** Uses counted in `stdlib/` and `scenes/`; all remaining uses are
the constructs' own tests.

| Construct | Uses | Code behind it |
|---|---:|---|
| `plan` (build-time trajectory solver) | 0 | `loft-plan` 949, `passes/plan.rs` 315, `check/plan_source.rs` 410 |
| `solver`, `constrain` | 0 | `emit/solver.rs` 364, parser and scope code |
| `body`, `state`, `retarget`, `chain`, `trunk`, `segment` | 0 | `check/motion_source.rs` 598, parser and AST |
| `input`, `yield`, `wgsl det`, `waive`, `if_some`/`if_true`, queries `medium`/`sigma`/`tau`, `raster(` | 0 | parser and checker branches |
| hardware ray query | 0 | `native/ray_query.rs` 205, touched in 10 more files |
| glTF import | 0 | `loft-gltf` 394 |

**2.6 Sizes.** Rust core by the repo's ratchet: 34,010 code lines against a budget of 18,000.
Largest crates: check 5.5k, passes 4.6k, eval 3.7k, parse 2.8k, wgpu 2.8k, emit 2.6k, cli 2.3k,
tier0 2.1k, runner 2.0k, std 2.0k, native 1.7k. `stdlib` is 2,600 lines of `.loft` and about 600
of WGSL templates.

**2.7 Owner ruling on file.** "Exactness is not a goal": bit-exact twin is an opt-in determinism
contract (`docs/spec/README.md`, 20 Nov).

## 3. Insights

**I1. The meta-engine already exists; the engine is not built on it.** The work is unifying the
middle layer, not a new language.

**I2. Loft has one idea, implemented five times**: a pure function applied to every element of
a store, at some stage of time.

| Construct | Runs over | When | May read | Writes |
|---|---|---|---|---|
| `law` | rows of one kind | each tick | own row, inputs | own fields |
| `check` | rows of one kind | each tick | own row | a verdict |
| `bake_tex` | texels | on load, on dirty(param) | params, functions | own texel |
| `screen_pass(f)` | pixels | each frame | nothing (one ray) | own pixel |
| `surface(u,v) grid` | grid vertices | each frame | own row | own vertex |

Every gap found this week is a missing cell in the cross product of those columns.

**I3. Performance strategy is three decisions**: where a value lives, when its producer runs
(compile, load, dirty, tick, frame), and what is claimed about it. "Precompute" is moving a
producer to an earlier stage.

**I4. The fat row is hand-done fusion.** Eight legs packed into scalar columns exist because
rows cannot read rows. `LegChain` rows are emitted and read by nothing.

**I5. One read rule gives history, ping-pong and two-body exchange** (§5.3).

**I6. Closure and density are the same work.** Every row of the table in §2.4 is both a private
door and a duplicate. Removing the duplicate opens the door.

**I7. The unused constructs are the spec's motion and solver model, and the design replaces
them.** `body`/`state`/`chain` describe a creature as fixed tables with frozen fields. Kinds with
row references describe the same thing as data, with no new construct. A `solver` is a kernel in
a pass loop. These were not merely unused; they are superseded.

**I8. The honest kernel.** What a library cannot do for itself:
1. stores, lifetimes and bindings;
2. lowering a typed pure function to a compute or fragment entry, fusing, ordering by hazards;
3. kernels with atomics (scatter);
4. hardware raster (scatter that keeps the nearest);
5. the open set of kinds: generating `trace`/`material`/… over every kind in a World, and the
   per-kind draw entry points;
6. proofs: spaces, units, intervals, Lipschitz, AD;
7. one twin evaluator and checks;
8. the runner and host interface.

## 4. Use cases the design must carry

"Needs" names the capability from §5.1.

| # | Use case | Stores | Steps | Needs |
|---|---|---|---|---|
| 1 | Fork edge antialiasing | Vis, Hdr | law over Hdr reading Vis neighbours | R, D, F |
| 2 | TAA, temporal upscale | Hdr, History | law reading its own store's previous frame | R, D |
| 3 | À-trous denoise | Hdr, two scratch | same law run 5 times with a step argument | R, D |
| 4 | ReSTIR | Reservoir image | three laws | R, D |
| 5 | SSAO, bloom, DoF | half and quarter targets | laws down and up | R, D |
| 6 | Gaussian splats | Splat rows, keys | law → library sort → raster or tile kernel | F, kernel |
| 7 | Own visibility (compute tracing, Hi-Z) | any record | raster with another record, or a law that traces | F |
| 8 | Shadow map only when casters move | ShadowMap | the raster, `on dirty(Casters)` | W |
| 9 | Sky baked once instead of integrated per pixel | SkyCube | law `on load`; shade samples it | W |
| 10 | Legs culled individually | Leg rows | Leg is its own drawn kind | R |
| 11 | N-legged gait, any body | Walker, Leg | parent loops its legs; leg reads its parent | R |
| 12 | Skin separate from skeleton | Walker, StageCrab | a `surface` that reads Walker and Leg rows | R |
| 13 | Two bodies exchanging momentum | Walker, Link | Link computes an impulse; each body gathers | R |
| 14 | Crowds and neighbours | Agent, cell ranges | library sort, then a law over ranges | R, kernel |
| 15 | Fluids, cloth, particles, constraint solvers | grids, particles | kernels with atomics, pass loops | F |
| 16 | RL: batched headless worlds, policy inference | Agent, Weight | law with a matmul loop over Weight rows; host reads obs and reward columns | R |
| 17 | Authored clips mixed with procedural motion | Clip rows | law samples Clip | R |
| 18 | Verification of any of the above | Verdict | check | R |

Not carried by laws alone: froxel integration along depth (a scan down a column) and anything
that truly scatters. Both go through `kernel`, which exists.

## 5. The design

### 5.1 Four capabilities, no new keyword

| | Capability | Today | Change |
|---|---|---|---|
| **R** | A law may read any store | own row only | Laws and kind queries accept `[Other]` parameters, as the spec writes them. |
| **D** | A law may run over any store | kind rows only | A law's target may be a `target` (an image), not only a kind. |
| **W** | Any step may run at any stage | `on load`/`on dirty` on `bake_tex` only | The same `on …` clause is legal on any scheduled step. Defaults stay. |
| **F** | One frame | render bundle XOR authored compute | An authored `pass` runs inside a render frame, binds the frame's stores by name, and fills a template's holes. Stock verbs become `stdlib` schedule functions built from it. |

**Decisions the first version left open.**
- *Spelling.* One construct: `law`. `bake_tex` becomes a `law … on load` over a `target`.
  `screen_pass` becomes a law over the frame's colour target. `check` keeps its name because its
  result type (Held, Missed, Unknown) is a different contract. Five map constructs become two.
- *The escape.* A pass template: a WGSL file with named holes, used by a `pass` statement that
  supplies the holes (Loft function symbols, constants) and the bindings. This is the mechanism
  the compiler already uses privately. Making it the public one is what lets the stock shaders
  move without changing a character.
- *Per-kind draw entry points stay in the kernel for now* (item 5 of I8). Moving the frame-level
  passes (cull, classify, the shade dispatch, shadow, post, present) is enough for closure of
  everything an engine author has asked to change. The kind adapters move only when someone
  needs a new way to draw a kind.
- *Stores.* Two declarations: `kind` (rows emitted by a build, with queries) and `target`/`table`
  for everything else, at top level. `table(...)` inside a schedule becomes the top-level form.

### 5.2 What authors write

**Legs as rows (10, 11).**

```
kind Leg @Deck (
  actor: Walker, previous: Leg, next: Leg,
  socket: p3@Body, home: p3@Deck, upper: m > 0m, lower: m > 0m,
  anchor: p3@Deck, toe: p3@Deck, age: s = 0.12s, aloft: f = 0.0
) {
  hit(ray) { … two closed-form capsules, hip to knee to toe … }     # analytic: no marching
  material(p, n) { … }
}

law balance(w: Walker, dt, legs: [Leg]) writes w.{x, z, vx, vz, lean} {       # proposed: third parameter
  var cop = [0m, 0m]
  for i in w.first_leg..w.first_leg + w.leg_count { cop = cop + share(legs[i]) * legs[i].anchor.xz }
  …
}

law swing(l: Leg, dt, walkers: [Walker]) writes l.{anchor, toe, age, aloft} {
  let w = walkers[l.actor]
  …
}
```

A body plan is the number of `Leg` rows a `build fn` emits.

**Skin separate from skeleton (12).** The spec's own form:

```
kind StageCrab @World (actor: Walker, …) {
  surface(u, v) grid 20x14 = shell(Walker[actor], Leg, u, v)
}
```

**TAA (2).**

```
target History: Radiance view

law taa(p: History, dt, hdr: [Hdr], vis: [Vis], old: [History]) writes p {    # proposed
  let was = sample(old, reproject(vis[p.px]))        # `old` is last frame: this law writes History
  p = if was == [0, 0, 0] { hdr[p.px] } else { mix(clip_to_neighbours(was, hdr, p.px), hdr[p.px], 0.1) }
}
```

**Precompute as a choice (8, 9).**

```
schedule Main {
  run_laws(sky_cube) on load                                   # proposed: `on` on any step
  find_shadow_map(Casters, size: 1024) on dirty(Casters)
  …
}
```

**Two-body exchange (13).**

```
kind Link (a: Walker, b: Walker, j: v3 = [0, 0, 0])
law exchange(k: Link, dt, ws: [Walker]) writes k.{j} { k.j = impulse(ws[k.a], ws[k.b], dt) }
law recoil(w: Walker, dt, links: [Link]) writes w.{momentum} {
  for i in w.first_link..w.first_link + w.link_count {
    let k = links[i]
    w.momentum = w.momentum + (if k.a == w.id { -1.0 } else { 1.0 }) * k.j
  }
}
```

**A stock pass as library source (F).** What `tile_classify` becomes in `stdlib`:

```
pub schedule tile_classify(vis: Target) {                                     # proposed
  pass classify reads [vis] writes [TileLists] {
    dispatch("core/classify.wgsl", "main", extent: tiles(vis, 8), halo: edge_halo)
  }
}
```

An author who wants 16-pixel tiles or a different binning copies those five lines.

### 5.3 Semantics: one rule

> A read of store S sees S after the last earlier step in this tick or frame that writes S.
> If there is none, or the reader is itself the writer of S, it sees S as it stood at the end
> of the previous tick or frame.

A law always sees its own element's fields as it has written them so far, as laws do today. The
rule is about every other element.

**Determinism.** A law writes only its own element and every read is of a fixed version, so the
result does not depend on execution order.

**Consequences.**
- *History* needs no keyword. An image read before it is written is last frame's image. It is
  zero on the first frame, after a resize, and after a host reset.
- *Ping-pong* is inferred: a law that reads other elements of the store it writes gets a second
  buffer.
- *Mutual reads are ordered by the schedule.*
- *Two-body exchange is exactly antisymmetric*: both bodies gather the same stored bits.

**Stages.** compile (kind parameter) < load < dirty < tick < frame. A step may read only values
produced at the same or an earlier stage.

### 5.4 Verification

- A law is a pure function, so its twin is the same function on the CPU against the law's GPU
  inputs read back. For images, sample pixels.
- **Correction to the first version:** authored WGSL passes are not necessarily Unknown.
  `loft-std/compute_twin.rs` already evaluates the deterministic compute subset from Naga IR.
  A stock pass moved into the library as a template keeps a twin through that path.
- Two twins exist (Loft source, Naga IR). They catch different faults: the source twin catches
  emitter bugs, the Naga twin catches driver and backend bugs. Keeping one is a decision for
  after Probe A, when it is known which passes each one covers. It is listed in §7 as a study,
  not as a cut.

### 5.5 Cost model

- A cross-store read is one more bound buffer and an indexed load.
- Laws on K kinds that read each other need at most K dispatches per tick. Walker + Leg is 2;
  with Link, 3. Catching up n ticks costs n × K dispatches for such laws.
- Island fusion (one thread per ownership root) can restore a single dispatch later if measured
  necessary. It is what the fat row does by hand.
- Image laws: one dispatch each, costed by bytes moved.

## 6. The attempts

| | Attempt | Verdict |
|---|---|---|
| A | New universal construct (`each px in …`, `draw`, `index`) | Dropped. A third world and five new names. |
| B | Finish the spec as written | Dropped as the answer. Its substrate is right; its render path is built-ins plus a raw escape; its motion tables freeze one creature into the ABI and are unused. |
| C | Rebuild the flagship on kernels | Half right. Kernels are the escape, not the default. |
| D | One law, one store model, any stage, one frame (§5) | Chosen. |
| E | Algorithm separate from schedule (Halide) | Cleanest theory; too large a compiler; scatter and state do not fit. Its principle is kept. |
| F | Forkability by regenerating Rust passes | Dropped. Consumers cannot write it and the kernel grows without bound. |
| G | General joins | Dropped. A stored row reference and a child range cover the cases at zero runtime cost. |
| H | Typed rewrite of every stock shader into Loft | Dropped as the first step. Text would change, and the N-feet lane showed that a changed loop shape alone moves pixels by 1/255. Templates with public holes move the shaders unchanged; typed rewrites can follow one pass at a time, each gated on its picture. |

## 7. What gets deleted

Sizes are `wc -l`. "Loses" is what a user can no longer do. Nothing here changes a picture: each
cut removes a path that no gate scene or library file takes.

### 7.1 Superseded by the design (cut once the replacement passes)

| Cut | Lines | Replaced by | Loses |
|---|---:|---|---|
| `body`, `state`, `retarget`, `chain`, `trunk`, `segment` | about 800 (`motion_source.rs` 598, parser, AST, scope) | kinds with row references (Probe B) | the frozen BODY/STATE/POSE layouts; nothing uses them |
| `solver`, `constrain` | about 500 (`emit/solver.rs` 364, parser, scope, `run_solver`) | a kernel in a pass loop | a dedicated XPBD syntax; nothing uses it |
| Fixed frame assembly: verb matching, hook signature checks, hole filling for frame-level passes | about 1,200 of `bundle.rs` 822, `pipeline/schedule.rs` 380, `shade_schedule.rs` 80, `shade_tiles.rs` 110, `lower_passes.rs` | `pass` + templates in `stdlib` (Probe A) | nothing; `stdlib` gains about 150 lines |
| `bake_tex`, `screen_pass` lowering as separate paths | about 400 (`bake_tex.rs` 90 + 110, `paths.rs` 258, part of `emit.rs`) | `law` over a `target` at a stage | the `bake_tex` spelling |
| The packed-leg gait (`gait.loft`, `body.loft` columns, `sites` parameter use) | library, not Rust | Walker + Leg rows | nothing |
| Options that accept one value (`tile: 8`) | small | an argument of the `stdlib` function | nothing |
| Loft's own GPU and quiet-window locking | 43 (done on `tools-agent-build`) | `agent-build` | nothing |

### 7.2 Unused (cut on the owner's word; each is a spec feature)

| Cut | Lines | Loses |
|---|---:|---|
| `plan` | about 1,700 (`loft-plan` 949, `passes/plan.rs` 315, `check/plan_source.rs` 410, parser) | build-time trajectory optimisation. The motion theory and the stride work do not use it. |
| Hardware ray query | about 350 (`native/ray_query.rs` 205 and call sites) | hardware tracing of static triangle meshes on devices that have it |
| `input`, `yield`, `wgsl det`, `waive`, `if_some`/`if_true`, `medium`/`sigma`/`tau`, `raster(` | about 200 | each is a spec construct with no user |
| glTF import | 394 | mesh import. I would keep it if authored assets are a v1 goal; say which. |

### 7.3 A third evaluator (the largest single cut; an owner decision)

| Cut | Lines | Loses |
|---|---:|---|
| Tier 0 tape VM and its edit path | about 3,300 (`loft-tier0` 2,073, `native/edit.rs` 360, `cli/edit_cmd.rs` 260, `wgpu/edits.rs` 242, `runner/encode/edit_gate.rs` 210, `std/promotion.rs` 150) | Live edits of one kind in under 2 ms. Without it an edit recompiles that kind's module, about 70–100 ms per pipeline. |

It is the third implementation of expression semantics, it is not among `CORE.md`'s five
priorities, and its timing test is the one that fails under contention. If instant edit preview
matters for a host you have in mind, keep it; otherwise it is 10% of the core for one feature.

### 7.4 Studies, not cuts yet

- **One twin instead of two** (`compute_twin.rs` 768, `trace.rs` 832, `bvh/twin.rs` 301,
  `stepping.rs` 360). Decide after Probe A.
- **`tools/loft-verify/src/controls.rs`** (1,138, outside the core count). It holds hand-written
  control shaders and still uses the triangle rule the Speed gate replaced.
- **`loft-cli/src/diff.rs`** (288): the Phase 5 legacy-parity rule. Cut when legacy parity is closed.
- **`scan`** (3 uses): a sequential fold baked to a LUT. It may be a law on load with a loop.

### 7.5 What the design adds

Capability R about 400 lines, D about 300, W about 100, F about 400. About 1,200 in total
(*estimated*).

## 8. The scorecard for "dense"

| Measure | Today | After §7.1 | After §7.2 and §7.3 as well |
|---|---:|---:|---:|
| Rust core, ratchet lines | 34,010 | about 32,500 | about 27,300 (26,900 without glTF) |
| Top-level keywords | 26 | 21 | 19 |
| Built-in schedule verbs | 16 | 5 (`run_laws`, `check`, a raster, `pass`, camera) | 5 |
| Constructs for "a function over a store" | 5 | 2 | 2 |
| Evaluators of a Loft expression | 3 | 3 | 2 |
| Ways to assemble a frame | 2 | 1 | 1 |
| Pipeline words in compiler crates | about 490 | kind adapters only | kind adapters only |
| Constructs with zero library or scene uses | 16 | 8 | 0 |

The 18,000 budget is not reached by any of this. The remaining mass is the language itself
(checker 5.5k, evaluator and proofs 3.7k, parser 2.8k) and the host surface (backend 2.8k, CLI
2.3k, runner 2.0k). Either the budget was set for a smaller language than the spec describes, or
a second round has to cut host features. I would reset the ratchet to the measured value after
these cuts so it starts holding again, and treat 18,000 as a question for that second round.

## 9. Predictions and how each can fail

| # | Prediction | Basis | Falsified if |
|---|---|---|---|
| P1 | `tonemap`, `tile_classify` and `cull` move to `stdlib` as `pass` + template with **byte-identical module text**. Pictures 19/19 hash-equal; pipelines and frame time unchanged. | The templates and the hole-filling stay the same; only who names them changes. | Any module's text hash differs, or a hole needs something a schedule cannot express. |
| P2 | Crab Live with legs as rows of an analytic two-capsule kind: about 4.4 ms, under 5 ms p95 (today 10.8–11.6). Thin margin. | Without legs, measured under load: Vis 0.52, Shadow 1.20; scaled by the with-legs ratio 0.75: 0.39 + 0.91. Shade about 1.95. Sixteen analytic capsules about 0.5. Cull, classify, present up to 0.6. | p95 ≥ 5 ms on a quiet machine. |
| P3 | Gait on Walker + Leg rows: under 0.15 ms per frame for one crab, 2 dispatches per tick, replay agrees. | One more small dispatch per tick. | Over 0.15 ms, or replay hashes differ. |
| P4 | TAA as one law: 0.40 ms at 1280×800. | 40 bytes per pixel × 1.02 M pixels ÷ 90 GB/s. | Over 0.6 ms. The bandwidth figure is unmeasured. |
| P5 | Rust core after §7.1 lands: 32,500 ± 500 ratchet lines (34,010 − 2,900 × 0.93 + 1,200). After §7.2 and §7.3 as well: 27,300 ± 700 (a further 5,550 × 0.93), keeping glTF. | §7 sizes × 0.93, plus §7.5. | Outside the band. |
| P6 | Scene authors write nothing new; token counts for existing scenes do not rise. | Stock engines keep their names and arguments. | Any gate scene needs an edit. |
| P7 | Every cut in §7.2 and §7.3 leaves all 19 pictures hash-equal and Twin and Truth unchanged. | No gate scene or library file takes those paths. | Any hash moves. |

## 10. What I still do not know

1. **Holes that hold expressions chosen by Rust.** `present` picks one of several resolve
   fragments by AOV and paper. A schedule must be able to make that choice (a compile-time
   `if`, or one template per variant). P1 tests it on `tonemap` first because it has this shape.
2. **Checkpoints.** `state … checkpoint N` is unused, but the runner's seek and replay may share
   code with it. Confirm before cutting `state`.
3. **Child ranges.** Two `u` columns work. Probe B reports whether a typed range is needed.
4. **Which twin to keep.**
5. **Whether models write engine-level code well.** Untested.
6. **Formats for `target`.** `bake_tex` names a format (`r16float`, `r8unorm`). With formats
   following the element's precision role, an explicit 8-bit choice needs a home.

## 11. Order of work

1. **Probe B, rows read rows** (running, branch `probe-rows`): capability R, legs as rows,
   `crab_rows`. Tests P2 and P3.
2. **Probe A, the public door**: `pass` inside a render frame with template holes; move
   `tonemap`, `tile_classify`, `cull`. Tests P1. Capability W rides with it.
3. **Cut 1** (§7.1): delete what A and B superseded. Tests the first half of P5.
4. **Cut 2** (§7.2, §7.3), on the owner's word per item. Tests P7 and the second half of P5.
5. **Probe C, a law over an image**: TAA in `stdlib`. Tests P4. Then `bake_tex` and
   `screen_pass` fold into `law`.
6. Reset the line ratchet to the measured value.

Probes add before they delete. The rule that keeps the core from only growing: no probe is
finished until the cut it enables has landed.
