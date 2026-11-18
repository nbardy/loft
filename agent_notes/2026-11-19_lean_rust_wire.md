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

## Wave-2 migration (not edited here; the lanes own the files)

The graph API changed. These are the mechanical edits each lane needs when it rebases:

- **2.1 kernels** (`loft-core/src/{rate,kernel}.rs`, `tests/it/kernels.rs`):
  - `Op::Leaf(Leaf::Time | Leaf::Param(_) | Leaf::Input(_))` becomes `Op::Time | Op::Param(_) | Op::Input(_)`, and
    `Leaf::Bind` becomes `Op::Bind(_) | Op::Var(_)`.
  - `Prim::Vec` becomes `Op::Vec`.
  - `g.decls[d]` with `Tag::Fn`/`Tag::Loop` and `roots_of` become `g.fns[f].{binders, body}` and
    `g.loops[l].{count, init, state, index, next, stop}`. A Fn has one body, so `results(f)` is `[g.fns[f].body]`.
  - Test graphs build `sym_at` (n + 1 starts) and the per-kind arrays instead of `sym_ends` and `decls`.
    `Op::Fetch.lane` is `u16`.
- **2.2 emit** (`loft-core/src/emit/{mod,body}.rs`, `tests/it/emit.rs`):
  - The `Tag::Param`/`Tag::Input` fold over decls becomes `g.params.len()` / `g.inputs.len()`.
  - `Op::Raw(sym)` becomes `Op::Raw(r)`, with `g.raws[r].{name, text, deps}`. The "same name, two sources" check can
    key on `name`.
  - "A graph function with several results" cannot be written any more.
  - Loop arity is `g.loops[l].init.len()`.
  - Tests drop `Prim::Pick` and `Ty::Rec(_)`.
- **2.3 analyses**: loft-ir is used only for `Error`/`Site`, so the only possible change is `NotFinite { node }`.
- **2.4 library**:
  - `src/record/graph_fn.ts` must return `{ kind: "body", binders, body }`. It is a type error after the merge, not
    a text conflict.
  - Its print.ts change (lazy call arguments) matches the Rust print. Its loop cache key
    `${k}:${level}:${bound}` no longer separates calls, because call arguments moved out of `bound`. A graph
    function whose body holds a loop that reads an argument, called twice with different arguments, would get the
    first call's loop line. The Rust print keys loops by (loop, level, call frame). Fix: add a frame id to the key.

## Left

- The program driver: loft-node `drive` still decodes and reports. A `print` program or driver hook in loft-core
  would let the shadow check run through the addon, not the test.
- `Fault::Scope` is found only by the print. Decode does not prove that a bound binder is read only inside its owner.
- `heron_leap_refused.ts`: score.ts does not catch import errors (lane 1.2), so the corpus is 124 of 125.
