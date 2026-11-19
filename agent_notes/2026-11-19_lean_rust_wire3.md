# Lean Rust: wire 3, recurrences and author compute on the wire, the TS side's cost (19 Nov 2026)

Plan: `docs/proposals/2026-11-19_lean_rust_loft.md` §3.2, §8.1. Follows `agent_notes/2026-11-19_lean_rust_wire2.md`
("Left" 4 and 6) and `agent_notes/2026-11-19_lean_rust_driver.md` (§8.1 table). Branch `feat/lr-wire3` from main
`7f48109`. Machine: M4, 10 CPUs, load1 8–12 throughout (other lanes' gates), so every timing here is `loaded`.

## Brief (written before the edits)

- **Blocker.** (1) 9 of R0's 238 builds are refused for things the wire cannot say: `loft_passtex_*` 3, `loft_cast` 2,
  `soft_box` 1, `loft_compute_*` 2, `t_clay_clusters` 1; and a recurrence's tables are `Fill::Recurrence { name }`
  with no stepping. (2) §8.1 fails on the TS side before any Rust phase: Owl 61/60/29, Titan 63/394/383, heron
  64/43/29 (start / link / job, ms).
- **Owner:** the recorder (`src/record/*`), loft-ir wire, decode and print. Not mine: emit binding, draw lowering,
  loft-gpu, loft-draw (feat/lr-draw); the driver's Check/Solve programs and `solve.rs` (feat/lr-solve2).
- **Cheapest checks:** `score.ts` print equality per example (TS graph print = job print), `corpus_prints_equal`
  (Rust print = TS print), job bytes unchanged by any speed change (sha), then `tools/parity/measure.ts` once.
- **Stop rule:** two failed fixes in one subsystem. **Claim boundary:** counts and times on this loaded machine.

## 1. What is on the wire now (wire 3)

| Refused in wire 2 | Wire 3 | Still refused by the driver, and why |
|---|---|---|
| recurrences (`Fill::Recurrence { name }`) | kept states carry the recurrence: cells (list n, grid w h), rate, from, until, first, steps, every (checkpoints), its `prev` kernel, and `start`/`step` lanes over the kernel's binders `K{k}i` (f32 or vec2) and `K{k}t`; prev/even/odd name the kept kernel | nothing new: built, counted `unfilled` (the driver plans no recurrence kernel yet) |
| `loft_compute_*` (author compute, 2) | `Kernel` fill 5: name, when (build, load, frame, steps start/count/rate/keep), entry, dispatch, text, libs (raw Fns), writes (key, symbol, buffer entries/lanes or texture dims/size/format), roots = the holes' values (`loft_value_k()`) | `unsupported: the author compute pass "…": on the wire, not yet lowered` |
| `t_clay_clusters` (table accessor, 1) | a raw Fn lists its reads: `0 table [kernel]` (accessor `t_<name>`), `1 output [kernel, output]`, `2 pass [pass]`; the veil sampler declares the medium tables it reads | Naga/prelude: `t_<name>` is not defined in emitted modules |
| `loft_passtex_*` (offscreen passes, 3) | Pass gets a target: `0 view` or `1 texture [name, size (view scale or fixed w h), keep (colour, depth, light), scene]`; its camera is `Camera` kind 2, `table [kernel]` (the pass's 12-lane camera row) | `unsupported: the offscreen pass "…": on the wire, not yet lowered` |
| `loft_cast` (2), `soft_box` (1) | **not on the wire:** they are TS draw modules' own WGSL; they come with lane 3.1's library rewrite (hit, place and paint graph functions in Draw decls) | as before (`Needs::Draw`) |

A recurrence's step emitted by the TS compiler (`compile/recur.ts`) is no longer a program root
(`rec.unwatched`): the job carries those lanes in the recurrence's kernel, over binders, and the same free variable
cannot be both. Map fills' TS emission is still captured as roots (unchanged from wire 2).

Print: `kernel TABLE recurrence list N|grid W H rate … first F steps S every E [#start] -> [#step]`, `kernel TABLE
recurrence prev|even|odd of TABLE`, `kernel compute:NAME WHEN entry … values [#v]`, `raw … reads [READ …]`, `camera NAME
table TABLE`, `pass … target view|texture NAME …` (grammar in `src/record/print.ts`). `sample.ts` now also records a
recurrence, three compute passes (load, steps reading the load pass's buffer, frame reading a param), an offscreen
mirror pass and a raw table read. WIRE = 3, addon stamp `abi 4 wire 3`.

### For feat/lr-draw (GPU-side lowering these need)

1. **Compute kernels:** loft-core `kernel::Fill::Raw` already models a raw kernel; bind each output (`Output.symbol`;
   buffers `array<vec4<f32>>`, textures storage-write in the pass and sampled/loaded by readers, steps keep a history
   of `1 + ceil(count / keep)` states), emit `loft_value_k()` accessors for the values, schedule by `When`. Readers
   are raw Fns with `Read::Output`.
2. **Table accessors:** for each `Read::Table(k)`, emit `fn t_<name>(i: u32) -> vec4<f32>` over the table's binding.
3. **Offscreen passes:** a texture resource per `Target::Texture` (`loft_passtex_<name>` at the size, format by keep),
   `loft_pass_sampler`, a camera from the table row (`Lens::Table`), and `scene: true` meaning every subject draw of
   the view passes. Clear is written `load`; what the texture starts as is its keep's (the TS host clears colour to
   0, depth to far).
4. **Recurrences:** a bake kernel per kept table (start at step 0, `step` reading `prev`), checkpoint replay into
   even/odd at `loft_step.<name>` (an Input the host writes).

## 2. The TS side's cost

### Where the time went (measured before any change, loaded)

- **start:** Bun plus parsing the library from source. A bare Bun process is ~27 ms; the rest is module load.
- **link:** for owl and heron almost all module parse (owl records 1.3k constructions at link, heron 68). For Titan
  52% is the TS engine's inlining of graph-function calls (`point`, 244 calls: author evaluation of each surface map for
  the TS engine), the rest parse and construction. Per-node JS objects (Node, trie entries) are a minor share:
  `leafOf` + `trieOf` ≈ 4% of a profile.
- **job:** not the walk alone. Titan's job (1,440 ms loaded) was ~1,000 ms of recording done at job time — graph
  function bodies lowered once (`settle`: 304k constructions) and table fills — and ~440 ms of writing at ~5 µs a node
  (spread pushes into JS arrays, a Map per node id, a TextEncoder per symbol).

### Why Titan recorded 638k (706k at 7f48109)

R = 75k at link + 327k inside the TS compile (draw modules building their expressions, counted) + 304k at job time.
The two causes found and fixed at the source:

1. **The hull's loft was unrolled.** `through(u, stations.map(s => s.curve(v)))` wrote all 17 station sections
   (~420 nodes each, ~7.1k a body) into `hull_skin`, and again into `hull_edge0`/`hull_edge1` (a constant `v`, so
   inlined), for 6 hulls: 129k at job time and ~170k more inside the TS compile. `through(m, count, point)` now sums
   the stations in one loop (`iterate`), the same folds in the same order (each a select of the fold or an exact 0):
   one section per body.
2. **Every binary with a plain number counted its constant twice** (lifted once to type it, again to build it).
   Lifted once now: same nodes, same ids, same job bytes.

R now: 64k + 138k + 107k = **308k** (U unchanged in kind; the job of every checked scene is byte-identical except
Titan's, whose hull is a loop).

### What changed

- **Bundle** (`tools/bundle.ts`, `tools/bundle_load.ts`, `tools/bundle_stamp.ts`): every src module in one Bun
  bytecode bundle (CommonJS, lazy `m<k>()` per module, `import.meta` rewritten per file); the preload resolves any
  import of a src file to its namespace there, one instance per module, and refuses a stale bundle (stamp of src
  paths, sizes and mtimes) with the rebuild command. `measure.ts` builds it and preloads it.
- **Recorder:** pushes allocate nothing (`Grow.push1/push4/pushAll`); the writer writes typed arrays, keeps node and
  call maps in growing `Int32Array`s, encodes symbols once. Same bytes (sha checked on owl, flock, pool,
  compute_frame, the sample job).
- **Interning** (src/expr.ts): a node of one or two children is one number key (`-1 - id`, `id0 · 2^26 + id1`) in a flat
  map, no trie objects; three or more keep the trie. Operands lifted once, loops for `map`/`forEach` on the hot path.
  Creation order unchanged, so ids and the TS engine's WGSL are unchanged.

Not done, and why: the canonical job is still written by a walk from the roots (now ~1 µs a node loaded); it is no
longer the cost. Recording graph-function bodies and fills at job time stays: the TS engine names its WGSL by node id,
so a body made earlier would rename it. Both go with the TS engine.

### Prediction (written before scoring; unloaded-equivalent ms CPU, §8.1)

From the component measurements: bundle start ×0.5, owl link ×0.35, heron link ×0.1, Titan link ×0.85 (bundle) ×0.8
(hull inlining) ×0.93 (interning); job: writer ×0.55, Titan's job-time recording ×0.44 (R 304k → 107k).

| | start | link | job | Rust (given) | rest | total | gate |
|---|---:|---:|---:|---:|---:|---:|---:|
| Owl | 30 | 21 | 17 | 10.5 | 4.7 | ~85 | 80 |
| Titan | 30 | 250 | 120 | ~130 | 22 | ~560 | 300 |
| heron | 30 | 4 | 18 | 27 | 7.1 | ~90 + design (TS solve ~1,958) | 200 |

Expected verdicts: Owl near the gate (FAIL by a few ms or a pass within noise); Titan FAIL (link is the TS engine's
inlining, ~2× the whole budget); heron FAIL on its TS solve alone.

### Scored once (`bun tools/parity/measure.ts`, 3 alternating runs, `out/parity/measure/609f7fa.json`)

load1 11.6–12.7 on 10 CPUs during the run, so every number is about 2× an unloaded one. To compare, the same tool ran
right after on wire 2's head (`d020e61`, the library from source; load1 13–14). Medians, ms CPU:

| | start | link | job | Rust (decode..bundle, loaded) | projected (loaded) |
|---|---:|---:|---:|---:|---:|
| Owl, wire 2 / wire 3 | 134.5 / **76.2** | 128.7 / **44.9** | 127.5 / **55.8** | 52 / 64 | 491 / 242 |
| Titan, wire 2 / wire 3 | 130.3 / **75.1** | 896.8 / **574.1** | 1,109.9 / **454.4** | 930 / 920 | 3,047 / 1,865 |
| heron, wire 2 / wire 3 | 136.4 / **80.9** | 82.4 / **9.4** | 167.3 / **120.0** | 197 / 226 | 5,435 / 5,329 (design 4,852: the TS solve) |

Ratios wire 3 / wire 2 under the same load, applied to the driver note's unloaded figures (61/60/29, 63/394/383,
64/43/29), against the prediction:

| | start | link | job | TS side | + Rust (given) + rest | gate | verdict |
|---|---|---|---|---:|---:|---:|---|
| Owl, predicted / measured | 30 / 35 | 21 / 21 | 17 / 13 | 68 / 69 | 83 / **84** | 80 | FAIL by ~4 (within the load's noise) |
| Titan, predicted / measured | 30 / 36 | 250 / 252 | 120 / 157 | 400 / 445 | 565 / **610** | 300 | FAIL |
| heron, predicted / measured | 30 / 38 | 4 / 5 | 18 / 21 | 52 / 64 | 86 / **98** + design | 200 | FAIL (TS solve, ~1,958 unloaded) |

The prediction held for start and link within 6 ms; Titan's job is 37 ms over (the job-time recording of the 298
`point` bodies and the fills fell less than the R count did). Counts: Owl R 20.3k, Titan R 308k (was 707k), heron R 93k;
Titan's U 236k is 1.29× the model (`COUNT MISS` on U only; R is 0.56× the model's 0.55M).

What still stands between Titan and 300 ms: the link (252) is the TS engine's eager inlining of graph-function calls
(author evaluation of every surface map at every read, for the TS engine's WGSL); a non-inlining recorder removes it,
which needs the TS engine gone (or lazy, which changes its WGSL). The Rust phase given (143) leaves 157 for start, link
and job together.

## Pass / fail

| Check | Result |
|---|---|
| TS graph print = job print, every example (`score.ts`, `out/wire3/score.jsonl`) | **pass**: 124 jobs (71 built, 52 library files `load`, 1 TypeError; heron_leap_refused has no job) |
| Rust print = TS print (`corpus_prints_equal`, `LOFT_WIRE_CORPUS`) | **125/125**: the 124 example jobs + `sample.ts`'s job (every wire-3 layout) |
| Job bytes unchanged by the speed work | owl, flock, pool, compute_frame and the sample job byte-identical; Titan changes only by its hull loop |
| `tools/dev.sh test` loft-ir 10/10, loft-core 20/20 (fixtures regenerated); clippy and rustfmt clean; `pnpm typecheck` | pass |
| `check.sh quick` (25 Bun test files incl. ships, curve splines, expr, text; crates loft-core, loft-ir, loft-gpu, loft-node) | pass |
| Corpus through the driver (`bun tools/parity/run.ts rust`) | **232 built**, 283 end as R0 did (unchanged in count). The 9 R0 builds still refused, each now by what it needs: compute 2 and offscreen 3 `unsupported: … on the wire, not yet lowered`; `t_clay_clusters` 1 (the read is on the wire, the accessor is not emitted); `loft_cast` 2 and `soft_box` 1 (not on the wire: lane 3.1's library) |
