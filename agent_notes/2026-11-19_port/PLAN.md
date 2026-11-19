# The 1:1 port of the TS lowering to Rust (19 Nov 2026)

## The decision this plan carries out

- **The contract is the existing artefact format** (`docs/artefact_format.md`): what `loft build` writes today and
  `rust/crates/loft-loader` (`loft-view`) runs. The format is fast. The five-crab lineup draws in 3.0 ms p50 on
  loft-view, against 97 ms on loft-draw (agent_notes/2026-11-19_loader_measure.md on `scratch/loader-measure`). The
  frame time comes from the artefact's shape: level-batched pose stages, shared draw shaders, and live level of detail.
- **TypeScript keeps running author code**: closures for paint, hit, place and surface maps, style functions, lights,
  and the `wgsl` tag's text-to-graph lowering.
- **Rust takes over the pure lowering**, translated 1:1 in a new crate, `rust/crates/loft-lower`:
  - graph to WGSL, slots, rates and table packing;
  - the level batching of pose stages (src/compile/tables.ts);
  - passes, LOD slots, scene assembly, `lower`, `share` and the artefact writer.
  - Its output must be **byte-identical** to the TS compiler's on the whole corpus: 125 examples and 170 seeds
    (tools/parity/corpus.json).
- **WGSL library text is shared, not translated.** There is one `.wgsl` file, which TS reads and Rust `include_str!`s.
  WGSL that TS builds from string templates gets its generator ported 1:1.

## 1. The TS compiler, from the recorder to artefact bytes

These are lines at main 5d94531. The **Side** column says what each module becomes:

- **R**: pure lowering, translated to Rust.
- **T**: stays in TS (author code or recording).
- **G**: GPU execution (bakes). The Rust side takes its outputs as data.
- **H**: host runtime. The loader already exists in Rust.

"Closures" means the module calls author closures, with the arguments named.

### Graph and compiler core

| Module | Lines | In -> out | Closures | Side | Depends on |
|---|---:|---|---|---|---|
| src/expr.ts | 1665 | constructors -> interned DAG; `emitWgsl`, `lit`, `fold`, `compileJs`, `topo`, `leaves` | `iterate` step once over fresh vars | constructors T (author API) and R (the compiler builds nodes too); emit/fold/compileJs R | recorder, artefact |
| src/grad.ts | 706 | Expr -> derivative Expr | none | R (draw modules differentiate fields) | expr, lib/noise |
| src/compile/bind.ts | 175 | leaves -> input/param/row/table slots; `emitFn` text | none | R | expr, rates |
| src/compile/rates.ts | 163 | programs -> lifted frame values | none | R | expr |
| src/compile/tables.ts | 903 | TableSource[] -> packing (first fit, 5 buffers), levels, **pose stages batched per level**, mixed f16 tables, CPU `poseFrame` | `lanes(i)`, `step(state,i)` over binders; static CPU fills | R | expr, rates, recur, bind |
| src/compile/recur.ts | 199 | recurrence -> start/step/replay/bake WGSL | step over binders | R | expr, recorder |
| src/compile/compute.ts | 100 | author compute -> bindings, `loft_value_k` accessors | none | R | compute, bind |
| src/compile/passes.ts | 205 | lib reads -> pass defs; `planPasses` by WGSL reach | `r.lanes` over a binder | R | frame/pass, bind |
| src/compile/scene_wgsl.ts | 99 | style + ground + sun + lighting -> style WGSL via `emitFn` | style `shade/ink/mark/sky/ground/shadow`, `outline` | emitting R; closure calls T | style, light, bind |
| src/compile/index.ts | 293 | Scene -> Artefact: draws, assembly, `detailSlots`, rest frame, `refuseSpread`, cast text | `t.draws(ctx)` | R | everything above |
| src/compile/context.ts | 170 | `CompileContext`, `Drawable`, `PHASES`, `PASS_OF` | defines them | types R; interface T | |
| src/frame/pass.ts | 391 | `pass(spec)` -> PassDef, camera tables | none | author API T; camera table and accessors R | expr |
| src/frame/lamp_shadow.ts | 202 | lighting + things -> lamp depth passes, shadow test | `pass.project`, `pass.at` (compiler-built) | R | cast, pass, light |

### Drawables (src/draw, src/soft, src/spacetime)

| Module | Lines | In -> out | Closures (arguments) | Side |
|---|---:|---|---|---|
| src/draw/surface.ts | 608 | Surface/Grid/Look/Copies -> shaders, draws, bound table, detail spec | `look.paint(u,v,at)` with compiler-built u, v (`grid.u.at(st.x)`); `point`/`normal` on the `placed(i)` pose; `poses.at(i)` with numbers too; glazed `choose(fragment)` | R (D1), closures T |
| src/draw/painted.ts | 142 | paint result -> look functions | branches on JS shape (`Expr` or `Painted`, channels, `scatter`, `shade`) | R (D1) |
| src/draw/grid_detail.ts | 105 | grid -> levels and rest measurement | `at.u/at.v` over binders | R (S: `measureRest`; D1: levels) |
| src/draw/relief.ts | 247 | height shell -> shaders | `height(site(u,v))`, `paint` | R (D1) |
| src/draw/ground.ts, sky.ts, ink.ts | 123, 46, 61 | ground, sky, screen ink | `ground.height(xz,t)` | R (D1) |
| src/draw/field.ts | 708 | field -> trace / brick / mesh schedule, union masks | `field(p)` at JS number points (Lipschitz) and over the ray; `paint({p,normal})` | R (D2) |
| src/draw/march.ts, trace.ts, field_shadow.ts | 108, 281, 88 | sphere trace graph, bound proxies, blob-shadow fit | `field(p)` | R (D2) |
| src/draw/traced.ts | 252 | `hit(ray, copy)` -> shaders, shadow fit | `hit` with a compiler ray; CPU 24x24 grid | R (D2) |
| src/map/brick.ts, nets.ts | 291, 126 | field -> brick table, surface-nets mesh | CPU sampling via `compileJs` | R (D2) |
| src/draw/mesh.ts, copies.ts, impostors.ts, shader.ts | 401, 229, 312, 50 | mesh rows, copy lists, lattices, raw modules | `motion.at(row, I)`, `shape(cell)` with compiler cells, `paint` | R (D3) |
| src/draw/cast.ts, light.ts, shadows.ts | 330, 249, 234 | `castRay` tables and mean colours (CPU at rest), style shadow block, ground shadows | `form.paint`, `form.field`, `joint(k)` with numbers | R (D4) |
| src/draw/volume.ts, src/soft/* (lowering parts), src/spacetime/{smear,telegraph,wake}.ts | 233, ~1300 of 2529, 498 | media, glow, rain, specks, lightning, wakes | `density(p,t)`, `albedo`, `emission`; stroke `point(u,i)` | R (D5) |
| src/draw/record.ts, src/record/decls.ts | 301, 94 | redesigned-path draw records (cover + roles) | role closures over binders | parked (section 5) |

### Lowering, sharing and files (src/host)

| Module | Lines | In -> out | Side |
|---|---:|---|---|
| src/host/lower.ts | 950 | Artefact -> manifest, chunks, blobs. Order: `checkArtefact`, `share`, chunk interning (`chunkName`), buffers, layouts, pose dispatches, compute, modules and pipelines, passes (depth resolve, clearance), plan, manifest, `formatVersion` | R (LW) |
| src/host/share.ts | 318 | draw shaders equal up to numbers -> one module and rows | R (SW) |
| src/host/files.ts `writeBundle` | 30 of 142 | Bundle -> `manifest.json` (JSON.stringify, compact), `wgsl/<chunk>.wgsl`, `bin/<blob>.bin` | R (SW) |
| src/host/manifest.ts | 263 | the format's types and key order | R (LW) |
| src/host/detail.ts | 217 | `gridBounds`, `slotsUsed` (lowering, about 25 lines); `planFrame` (per frame) | lowering R; plan H (already in loft-loader plan.rs) |
| src/host/bake.ts, host.ts `bakeRawBuild` | 116, 35 | recurrence bake, compute build | G: their bytes are inputs (`input/baked`) |
| src/host/{host,camera,still,profile,cache,precision,compute_data,headless}.ts | about 1700 | the Dawn host | H |
| src/bake/* | 949 | `loft bake` glTF export | not on the build path |

### Recording and author-side lowering (stays TS)

| Module | Lines | Role |
|---|---:|---|
| src/record/{recorder,wire,print}.ts | 588, 103, 492 | Wire 4: node log, job writer, canonical print. Wire 5 lands with integrate/design. |
| src/record/port.ts | 25 | New. The port's input. Owned by lane W. |
| src/text/* | 2312 | The `wgsl` tag: parse, lower text to graph, raw-function rewrite. It runs when an author function is called. Its output (graphs, LibFn text) crosses the wire as data. |
| src/style.ts, src/light.ts, src/style/* (except post templates) | 388, 238, about 3500 | Style and light closures. TS evaluates them at the contexts the compiler builds. |
| src/map/{table,surface,recur}.ts, src/relief/sculpt.ts, src/style/pbr_parts.ts | | Author-time CPU fills (`tabulate` static data, lathe winding, SH projection). The results are wire data. |

## 2. The cut, and what wire 4 lacks

### Wire 4 does not carry what the pure lowering consumes

`src/record/port.ts` writes today's port input: the wire-4 job that `LOFT_ENGINE=rust loft build` writes.
`probe.wire` shows Rust decodes it exactly: TS `printJob` equals loft-ir `print`. It carries:

- the graph (nodes, lits, rows, syms);
- graph and raw Fns, Loops, table Kernels (map, scan, data, recurrence, compute);
- Params, Inputs, Checks, Solves;
- the redesigned path's Draw, Pass, Medium and Camera decls;
- the de-duplicated set of roots the TS compiler emitted.

### The gaps

Each gap lists what the lowering reads, where it reads it, and the smallest wire addition.

1. **Names follow process ids.**
   - Every let is `e${expr.id}` and every loop `L${loop.id}` (expr.ts:1089, 1098).
   - Ids count every node the process made: imports, discarded work, earlier variants.
   - Measured: `diff.ts --mode shift` (1000 extra nodes made before the scene) changes the chunks of every entry with a graph.
   - No wire field can fix this. **Lane P0** makes TS names canonical (numbered per emission), which re-baselines TS bytes once. Without it nothing is byte-identical.
2. **Drawables (`t.draws(ctx)`).** The draw modules read:
   - the thing's name (its `nameId` hash goes into WGSL);
   - enums (look, sides, outline, phase, schedule);
   - numbers (grid counts, steps, `hMax`, cells, g);
   - bounds;
   - copies and poses;
   - mesh rows and indices (Float32Array, Uint32Array);
   - raw module text;
   - closures.

   **Add:**
   - Step 1: a tape entry per thing (below).
   - Step 2: a typed **Thing** decl. It holds the kind, a name sym, words and lits per kind, a graph Fn per closure call site, Fn tables for closures the compiler calls with numbers, and blobs for mesh data.
3. **Paint classification.** `classify` branches on the paint result's JS shape (painted.ts:28-55). **Add:** a Painted record with the base Fn, (name, Fn) per channel, and scatter/shade Fns at their contexts.
4. **Style.** Its closures take contexts that themselves hold closures (`lights`, `channel`, `sky`, `scatter`, `geometry`; style.ts:207-384). They are called under `withIdentities` (scene_wgsl.ts:21, 49) and probed by `styleReads`. **Add:** TS evaluates every style function at every context the compiler builds. It records each result as an emit request (fn name, args, ret type, roots), plus a reads word and the notes.
5. **Lighting.**
   - the sun's kind and its direction, normalised with `Math.hypot` (scene.ts:116);
   - the amount;
   - each light's Arrival fields;
   - sampling;
   - the lamp-shadow passes (lamp_shadow.ts:44-58).

   **Add:** a Light decl (one Fn per Arrival field), plus shadow words.
6. **Ground.**
   - kind;
   - the sea's `height`, `gradient`, deformations, glazed `choose` and cases (geom/sea.ts:44-70, scene_wgsl.ts:28-47).

   **Add:** a Ground decl. The sea is drawn as a Thing.
7. **Skeletons.** `Drawable.skeletons` is read by shadows, cast, light and lamp_shadow. **Add:** a Skeleton decl: chain counts, plus joint position and radius roots.
8. **Forms.** Cast mean colours come from CPU evaluation of each form's paint at rest (cast.ts:117-150). The Thing decl covers them, and lane G2 evaluates.
9. **Raw library functions.**
   - The wire carries name, ret, text, deps and reads.
   - It lacks the `js` twin and the gpu-only flag (expr.ts:58-66), which rates, `fold` and `compileJs` read. It also lacks `rawComputeOf`.
   - **Add:** a flags word and the rawCompute resource sym.
   - The twins (lib/noise.ts and the others) are ported to Rust by name in lane G2.
10. **Tables only template text declares.** Tables that `ctx.table(t)` declares for draw-template text (copies, shadows, light, cast, surface bounds) are unreachable from the roots. Their declaration order decides packing. **Add:** the ordered table-declaration list, per tape entry.
11. **Roots lose their grouping.** The watch set is flat and already lowered. **Fix:** the port job drops it and carries emit requests and tape entries instead.
12. **Compute passes.** These are written only when a raw Fn reads them (recorder.ts:441). **Add:** a program-level list of compute kernels.
13. **Screen passes.** These throw `Unrecorded` (recorder.ts:428). **Add:** a Pass target "screen" with its shader.
14. **Post passes.** `style.post`, `ctx.post` and `INK_POST` are not carried. **Add:** a Post decl: label, wgsl and mixed wgsl syms.
15. **Scene scalars.** The outline method, the loop period, `style.clear`, and numeric cameras are missing; there is no orthographic camera on the wire. **Add:** a Scene decl.
16. **Authored-text registry.** The registry (authored.ts:15-30) feeds `manifest.authored`. **Add:** Authored entries.
17. **CPU readings.** Detail rest measurements, field Lipschitz, bricks, surface nets and cast colours all need a Rust `compileJs` that matches JS bit for bit. This is a risk (section 6), owned by G2, not a wire field.
18. **Closures called with constants.** The constructors simplify on constant operands (`isZero`, `isOne`, `fold`), so a Fn over binders, substituted later, can differ from TS's direct call. **Add:** Rust substitutes by reconstruction through the same constructors (G1). Where the compiler calls a closure with plain JS numbers (`poses.at(f(i))`, `joint(k)`), the decl records each call's result.

### The cut, in two steps, both byte-identical on the whole corpus

**Step 1: the emission tape** (lanes W, B, S, LW, SW).

- TS records each drawable's `Emitted`:
  - shader text and the LibFns it names;
  - draw specs;
  - posts;
  - tables declared, in order;
  - the roots it registered, in order, so Rust replays slot allocation.
- TS also records the scene-level decls (gaps 4 to 7 and 12 to 16), which include the style's emit requests.
- Rust does everything `compileScene` does after `t.draws(ctx)`, then `lower`, `share` and the writer:
  - slots;
  - tables, levels and pose stages;
  - style WGSL;
  - passes;
  - detail slots;
  - assembly.
- At the end of step 1, `loft-lower` writes every corpus artefact byte for byte. The drawables' template text still comes from TS.

**Step 2: the drawables** (lanes D1 to D5).

- Each draw lane turns its kinds' typed Thing decls into the same `Emitted` the TS module produced.
- It gates on its kinds' `probe.tape.<k>.<kind>` sections.
- Once a kind passes, the tape entry for that kind is replaced by its decl.
- When the last kind lands, TS records only author code: graphs, data and decls. The TS compiler, `src/host/lower.ts` and the old TS loader can then be deleted, as CORE.md's "Next" asks.

Why two steps:

- The style's contexts are higher-order (gap 4), and the draw modules branch in JS on closure results (gap 3, field variance, mesh leaves).
- Recording those faithfully is the hard part of the wire.
- Step 1 puts everything downstream of the drawables (about 70% of the lowering's lines) under byte parity without waiting for it.

## 3. The diff harness (built: tools/port)

```
bun tools/port/diff.ts [--mode port|lower|shift|ts|cli] [--scope LANE[,LANE]] [--only TEXT] [--jobs N] [--shard I/N]
                       [--baseline out/port/<run>] [--run NAME] [--reuse-ts]
```

**Per entry:**

1. The TS side runs `tools/port/ts_side.ts`, in its own process so Expr ids start fresh, as `loft build` does. It writes:
   - `outcome.json`, with the stage a refusal came from (`author`, `compile`, `lower`);
   - `compile.json`, the Artefact before bake, as JSON (`plain`: typed arrays as base64). This is the boundary
     between the compile lanes and the lower lanes. Rust writes the same file;
   - `artefact/`, byte-identical to `loft build`;
   - `input/job.bin` and `input/baked/*.bin`, the GPU bake outputs as data;
   - `probe/{wire,graph,eval}.txt`.
2. The Rust side runs `rust/target/lab/loft-lower`, built with `tools/dev.sh build loft-lower --fast`.
3. The two sides are compared section by section, in pipeline order:
   - `probe.*`: `wire` (W), `graph` (G1), `eval` (G2), `tape.<k>.<kind>` (D lanes);
   - `compile.*`: slots and tables (B), everything else (S);
   - `files.*`: every manifest key, every chunk and blob by index, `manifest.json` whole, and the file set (LW, SW);
   - `outcome`: a refusal at stage `compile` or `lower` must equal the TS compiler's in exit code and head.

   A refusal at stage `author` (the scene's module does not load, construct or style; most are library files with no
   default export) passes in `port`, `lower` and `cli`. No lowering ran, so there is nothing to port.

**Modes:**

- `port`: the default, the whole port.
- `lower`: `loft-lower lower` from the TS compiler's own compile.json. LW and SW gate on it from day one.
- `shift`: the TS side again with Expr ids shifted by 1000. This is P0's gate.
- `ts --baseline RUN`: the TS side against an earlier run's TS side. It proves a TS refactor (LIB, W) keeps every byte.
- `cli`: the TS side's artefact against `loft build`'s. It proves the harness's reference is the shipped compiler.

**Reports:**

- `--scope` compares only that lane's sections. An entry with none of them on either side passes ("no section in scope").
- One `FAIL <id>: <section> [<lane>]: <where>` line per failing entry. `where` gives the first differing line of both sides, or a byte offset.
- A summary of failures by first lane and section.
- `out/port/<run>/report.json`.

**Strictness:**

- Each of these fails its entry by name:
  - a TS crash (an untyped throw is the TS compiler's bug);
  - a TS input the wire cannot write;
  - a missing binary;
  - a killed or verdict-less Rust process;
  - a Rust refusal the TS compiler did not make;
  - a section absent on either side.
- Exit 0 only when every entry passes in an unfiltered run. A run with `--only` or `--shard` cannot pass.

**Parallelism:**

- `--jobs N` runs entries concurrently; the default is half the cores.
- `--shard I/N` splits the corpus across processes or machines.
- `--reuse-ts` keeps the TS side of an earlier run of the same `--run`, so a Rust lane iterates in seconds.

**Proof on today's state** (stub crate, every lowering module unported): see "Proof" at the end.

## 4. Lanes

Fourteen lanes. Each owns disjoint files, works on branch `port/<lane>` in `~/git/_wt/loft-port-<lane>`
(`tools/lane.sh new`), and gates on the harness scoped to itself. The briefs are `lanes/<lane>.md`, ready for
`codex exec` (`run_lane.sh`). Estimates are Rust lines translated (TS lines in brackets).

| Lane | Translates | Owns | Est. Rust (TS) | Needs | Gate |
|---|---|---|---:|---|---|
| **P0** canon | names per emission, not per process | src/expr.ts `emitWgsl`; any id-named site the shift run finds | 0 (60 TS) | none | `--mode shift`: 0 failures |
| **LIB** wgsl-lib | constant WGSL to `src/wgsl/*.wgsl`, read by both sides | src/wgsl/, the TS constant sites, loft-lower/src/wgsl.rs | 120 (150 TS edits) | none | `--mode ts --baseline` 0 failures; `tools/dev.sh test loft-lower` |
| **W** wire | the tape and scene decls (gaps 2 to 16, step 1 form); the Thing envelope | src/record/port.ts, src/record/tape.ts, loft-lower/src/input.rs (and input/) | 1000 (700 TS) | none (rebases on wire 5 when it lands) | `--scope W` (probe.wire round trip), `--mode ts` unchanged |
| **G1** graph | src/expr.ts constructors, interning, `fold`, substitute, `topo`/`leaves`, `emitWgsl`, three `lit`s, JS number spelling | loft-lower/src/graph/ | 1400 (900) | P0 (the probe is canonical only after it) | `--scope G1` |
| **G2** eval | `compileJs` as a Rust evaluator, src/grad.ts, the library's JS twins, JSC-exact `Math` | loft-lower/src/eval.rs and eval/ | 1300 (1300) | G1 | `--scope G2` |
| **B** tables | src/compile/{bind,rates,tables,recur,compute}.ts | loft-lower/src/compile/{bind,rates,tables,recur,compute}.rs | 1800 (1540) | G1, W | `--scope B` |
| **S** scene | src/compile/{index,scene_wgsl,passes}.ts, the lowering in src/frame/*, `measureRest`, `detailSlots` | loft-lower/src/compile/{mod,scene_wgsl,passes,frame}.rs | 1100 (900) | G1, G2, B, W | `--scope S` |
| **LW** lower | src/host/lower.ts, src/host/manifest.ts, detail.ts `gridBounds`/`slotsUsed`, the Artefact types | loft-lower/src/lower/, artefact.rs | 1500 (1250) | SW's interface (in its brief) | `--mode lower --scope LW` |
| **SW** share and write | src/host/share.ts, `writeBundle`, `chunkName`, JS-exact JSON | loft-lower/src/{share,files}.rs | 900 (380) | none | `--mode lower --scope SW` |
| **D1** surface | src/draw/{surface,painted,grid_detail,relief,ground,sky,ink}.ts | loft-lower/src/draw/surface.rs (and surface/) | 1400 (1330) | G1, W | `--scope D1` |
| **D2** field | src/draw/{field,march,trace,field_shadow,traced}.ts, src/map/{brick,nets}.ts | loft-lower/src/draw/field.rs (and field/) | 1900 (1850) | G1, G2, W | `--scope D2` |
| **D3** solid | src/draw/{mesh,copies,impostors,shader}.ts | loft-lower/src/draw/solid.rs (and solid/) | 1000 (990) | G1, G2, W | `--scope D3` |
| **D4** light | src/draw/{cast,light,shadows}.ts | loft-lower/src/draw/light.rs (and light/) | 850 (810) | G1, G2, W | `--scope D4` |
| **D5** soft | src/draw/volume.ts, src/soft/* (their `draws`), src/spacetime/{smear,telegraph,wake}.ts | loft-lower/src/draw/soft.rs (and soft/) | 1600 (about 1800) | G1, W | `--scope D5` |

Total: about 16,000 Rust lines, translated from about 14,000 TS lines.

### Waves

```text
wave 0 (start together): P0  LIB  W  G1  G2  LW  SW            7 lanes
wave 1 (as G1 and W land): B  D1  D3  D5  (G2 too: D2  D4)    6 lanes
wave 2 (as B lands):       S                                    1 lane
step 1 done when B, S, LW, SW, W, G1, G2 pass unscoped (`bun tools/port/diff.ts`, 0 failures with tape-fed draws)
step 2 done when D1 to D5 pass: the TS compiler can go
```

- The critical path is W and G1, then B, then S.
- LW and SW never wait: they lower the TS compiler's own compile.json.
- The D lanes wait only for G1 and W's envelope, not for B or S.
- P0 is the one TS-engine change. It lands first, about a day's work, because every WGSL comparison before it is noise.

### Integration

- One integrator merges lanes in dependency order: P0, LIB, then G1, W, SW, LW, then G2, B, then S, then D*.
- After each merge it runs `bun tools/port/diff.ts` once (one full run per wave, not per merge; the machine is shared).
- A lane rebases onto main when a dependency lands. It never merges.

## 5. The redesigned path: park its lowering, keep its pieces

**Decision: park.**

- loft-core's lowering is frozen as it stands on main: `emit/`, `kernel.rs`, `fill.rs`, `layout.rs`, `schedule.rs`.
- So are Bundle V1 (loft-ir `bundle.rs`), `loft-draw` and loft-gpu's per-kernel `dispatch_all`.
- No new work is funded on them.
- They are not deleted while integrate/design (feat/design-render and feat/design-motion) lands on main and its checks run on them.
- They are deleted, with the TS compiler, when step 2 passes and loft-view draws the corpus at parity.

**Why:**

- The measurement says the executor is not the problem; the artefact's shape is. loft-view runs the TS artefact at or below Dawn's cost.
- loft-draw runs Bundle V1 at 10 to 30 times that cost, because it has one dispatch per kernel (4,884 against 24) and one pipeline per draw.
- Giving loft-core the TS shape means re-deriving level batching, sharing and LOD in a different IR. The port translates them directly, and the harness proves each byte.
- Two lowerings kept alive is a CORE.md not-goal.

**Merge into the port:**

- **loft-ir's wire decoder and canonical print.** The port's input is a wire job, and `probe.wire` already matches.
- **loft-core's `solve`, `check`, `interval`, `tape`.** They are analyses, not lowering, and keep CORE's truth priority (`Unproved`, `Undecided`). Untouched.
- **loft-gpu's naga validation, pipeline cache and Keel embedding (instances, pins).** These move into the loader runtime. That is the parallel loader lane the lead named, not this port.
- **integrate/design's measured ideas.** These are the render checks, POSE clip, GPU grid compaction with indexed indirect draws, and fused motion ticks. They come back after step 1 as GPU-time work on the port's output, each with its own format-version bump and a picture-parity gate. The design documents stay the architecture's direction.
- integrate/design's **wire 5** replaces wire 4 under lane W. W rebases onto it when it lands, and its tape decls take wire-5 tags.
- New examples it adds join the corpus automatically (`examples/**/*.ts`). If the TS compiler refuses one, the Rust side must refuse it identically.

## 6. Risks to byte identity

Each risk names its owner.

- **Number spelling** (G1, SW):
  - expr.ts `lit`, which is shortest `toPrecision` that rounds to the same f32;
  - scene_wgsl.ts `lit` and recur.ts `lit`;
  - `toFixed`, where JS and Rust disagree on ties and on `-0`;
  - `JSON.stringify`: integers have no `.0`, and the exponent appears from 1e21 and below 1e-6.

  Port the spellings exactly. Do not use Rust's float `Display` or serde_json's float output.
- **`Math` on the CPU** (G2):
  - Bun runs JavaScriptCore, not V8.
  - JSC's `Math.sin/cos/exp/log` call the platform libm on macOS. Rust's `f64::sin` reaches the same libm, but `Math.pow`, `Math.hypot` and `Math.atan2` have their own JSC code.
  - Port JSC's `hypot` and `pow` paths, and keep `Math.fround` exactly where TS rounds.
  - `probe.eval` measures this on every corpus root before a draw lane relies on it.
- **Order** (B, S, LW):
  - JS `Map`/`Set` iterate in insertion order, so use `IndexMap`.
  - JS objects put integer-like keys first (`Object.fromEntries(stages)`; `mixedModules` keyed by id).
  - Sorts are stable.
  - Manifest keys follow lower.ts's object literals, so serde_json runs with `preserve_order`.
- **Hashes** (SW): `chunkName` and `nameId` are FNV over UTF-16 code units, not over bytes.
- **Construction modes** (G1):
  - `withIdentities`;
  - the constant-argument rule;
  - SameValueZero interning of -0.

## Proof

PROOF_PLACEHOLDER
