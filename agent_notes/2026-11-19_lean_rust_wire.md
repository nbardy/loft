# The wire: loft-ir adopts lane 1.2's layout (19 Nov 2026)

Plan: `docs/proposals/2026-11-19_lean_rust_loft.md` §3.2 (now pinned as implemented), §8.2 (lane 1.2's pass
condition). Branch `feat/lr-wire`. Follows "Wave 1 integration: the shadow check does not pass" in
`2026-11-19_lean_rust_lr12.md`.

## What

- `loft-ir/src/wire.rs`: decodes `src/record/wire.ts`'s `pack` byte for byte. Spans are sequential (length, zero pad
  word, bytes, zero padding to 8, nothing after the last), symbols are `symAt` (n + 1 offsets), decls are
  `[program root count, (tag, n roots, n words, words...)...]` and `roots` starts with the program roots. Op codes
  0..=11 are Lit, Time, Param, Input, Bind, Call, Fetch, Loop, Grad, Raw, Swizzle, Vec, and prims are 16 + the
  `src/expr.ts` `Fn` order.
- `loft-ir/src/graph.rs`: one tagged `Op`, with one variant per wire op. `Leaf` is gone: `Time`, `Param`, `Input`,
  `Bind` (flag 1, a loop or graph-function binder) and `Var` (flag 0, a free variable) are variants, and so is `Vec`.
  Decls are classified once into per-kind arrays: `fns: [GraphFn]`, `raws: [RawFn]`, `loops: [Loop]`,
  `kernels: [Kernel::{Table, Rows}]`, `params`, and `inputs`. `Call`/`Raw` index `fns`/`raws` (the wire numbers both
  as Fn ordinals; the decoder splits them), so no consumer reads a tag or a flag. `Prim` is in the TS order, which
  equals loft-core's `Prim`. `Prim::Vec` became `Op::Vec`. `Prim::Pick` and `Ty::Rec` are removed, because no
  recorder writes them and lane 1.2's Fn has one result.
- `loft-ir/src/print.rs`: the canonical print of `src/record/print.ts`, with calls inlined and loop binders as
  levels. A call argument is walked where its binder is first read, as lane 2.4's print.ts change does. That is the
  order in which the TS graph, which inlined the call, meets it. The FNV of raw text replicates print.ts `codes`
  exactly: per code point, the first UTF-16 unit.
- Decode refuses Draw, Pass, Medium, Check, Solve and Camera decls (`Fault::Tag`) until their lanes pin a layout in
  §3.2. `Fault::Scope` is new: the print found a binder read outside its owner. `SceneFault::NotFinite { node }`
  replaces `{ at: Leaf }`.
- **Found by the corpus:** a node's `flags` is 16 bits, not 8. Table lanes reach 474 (crab_lineup) and 632
  (the_strike). TS `head()` already wrote `flags << 16` unmasked, so `Op::Fetch.lane` and `Op::Loop.k` are now `u16`.
- TS: `FnBody` (`src/record/recorder.ts`, `src/text/lower.ts`) is `{kind: "body", ...} | {kind: "inline", why}`, so
  no string stands in for "no body". Job bytes are unchanged: the dust_veil, reeds_shadow and stormwing hashes are
  the same before and after.
- `loft-node`: `Report.decls` sums the decl arrays. `binding_check.ts` builds its chain job with `pack`, not with a
  second hand-written writer.

Lines (scc code): loft-ir src 1,414 (was about 860, of which print.rs is about 230).

## Pass/fail (M4, nice 12)

| Check | Result |
|---|---|
| Corpus print equality, Rust `print(decode(job))` against the TS graph's `--print` | **124/125**. All 124 jobs are equal: 71 built, 53 `refused` at load (library files, plus `serpent_clip`'s TS compile `TypeError`). The 125th file, `second/heron_leap_refused.ts`, throws `SceneError` at import, before score.ts can write a job (lane 1.2's open item) |
| Shadow check on owl, Titan (reefback_titan), heron_leap and dust_veil | Through `wire::decode` + `print`, the same decode the addon's `drive` calls: all four decode (no `Wire{Args, Bounds}`) and print equal. **Not run through the built addon**: `tools/dev.sh build loft-node --fast` waited over 50 min on the machine's heavy compile lock (Keel `check.sh quick`) and was stopped. Pending: `tools/dev.sh build loft-node --fast --features probe`, then `bun out/wire/shadow.ts rust/target/lab/libloft_node.dylib out/wire/corpus/{second__owl,bosses__reefback_titan,second__heron_leap,dust_veil}.job` and `bun rust/crates/loft-node/binding_check.ts` (which now uses `pack`) |
| `tools/dev.sh test loft-ir` | 8/8. The fuzz refused 91,345 of 10⁵ mutated jobs, all typed, with no panic; the 8,655 that decode also print without a panic. The fault table has 20 named edits plus pad, trailing and program faults. The print known answer has a call inlined and a loop |
| `tools/dev.sh lint loft-ir`, `lint loft-node`, `cargo fmt --check` | clean |
| `tsc --noEmit`; `bun test tests/text.test.ts tests/expr.test.ts` | clean; 8/8 |

Reproduce: run `out/wire/dump_all.sh` (score.ts per examples file, about 3 min), then
`LOFT_WIRE_CORPUS=$PWD/out/wire/corpus tools/dev.sh test loft-ir --run-ignored only --no-capture`.

## Wave-2 merge (main 5272308 merged in; no text conflicts)

The wave-2 code moved to the per-kind graph:

- **2.1 kernels**: `rate`, `kernel` read `Op::{Time, Param, Input, Bind, Var, Vec}` and `g.fns` / `g.loops`
  (`Loop::nodes()` in loft-ir lists every node a loop names). `rate::binders` is gone. **Not mechanical:**
  `Fill::Map`/`Fill::Scan` held `Fn` decls with several results (a row's lanes; a step's next states and lanes),
  which a one-body `GraphFn` cannot say. A fill now holds its own roots: `Map { index, lanes }`,
  `Scan { init, state, index, next, lanes }`. The corpus plan is unchanged: 71 scenes, 4,020 tables, 215 vs 446 frame
  dispatches, and the ignored const-fill check is the same red (79 fills over 1 ulp, worst 9.2e-6 relative).
- **2.2 emit**: `module` takes `Slots { params, inputs, kernels }` (layout's slots, indexed like the graph's decl
  arrays) instead of one per-decl `[u32; 2]` table. `Op::Raw(r)` reads `g.raws[r]`: the name is the declared one (no
  "last function in the source" guess), and the module includes a raw function's `deps` before it. `Own`/`Pick` and
  the several-results refusal are gone; `Op::Vec` is written by the body.
- **2.3 analyses**: no change needed.
- **2.4 library**: `graph_fn.ts` returns `kind: "body"`. The job print's loop cache key gains the call frame
  (`Scope.frame`), so a graph function's loop that reads an argument gets one line per call, as the Rust print does.

| Check (after the merge) | Result |
|---|---|
| Corpus print equality (`out/wire/dump_all.sh` re-run on merged TS) | **124/125**, all 124 jobs equal; `heron_leap_refused.ts` still throws at import |
| Lab addon `tools/dev.sh build loft-node --fast --features probe`; `binding_check.ts` | built; `fails: []` (28 MB job, decode 82 ms) |
| Shadow check through the addon: owl, reefback_titan, heron_leap, dust_veil | all four `run` as `Scene` (5,906 / 53,955 / 12,537 / 613 nodes) and print equal |
| `tools/dev.sh test loft-ir`, `test loft-core`; lint both; `cargo fmt --check` | 8/8 + corpus; 13/13 (1 ignored, known red); clean |
| `pnpm typecheck` | clean |

## Left

- The program driver: loft-node `drive` still decodes and reports. A `print` program or driver hook in loft-core
  would let the shadow check run through the addon, not the test.
- `Fault::Scope` is found only by the print. Decode does not prove that a bound binder is read only inside its owner.
- `heron_leap_refused.ts`: score.ts does not catch import errors (lane 1.2), so the corpus is 124 of 125.
