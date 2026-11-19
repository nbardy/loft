# Port lane W: the port's input: the emission tape and scene decls

Branch `port/w`, worktree `~/git/_wt/loft-port-w`, plan `agent_notes/2026-11-19_port/PLAN.md`.

## Your lane: W, the wire for the port (TS recorder and Rust decoder)

**Blocker.**

- `src/record/port.ts` writes the wire-4 job that `LOFT_ENGINE=rust loft build` writes. It carries the graph and the
  table kernels.
- It does not carry what the TS lowering consumes. PLAN.md section 2 lists the gaps, numbered 2 to 16.
- `rust/crates/loft-lower/src/input.rs` decodes it with loft-ir.

**Target: step 1 of the cut (PLAN.md section 2, "The emission tape").** The job carries:

1. **The tape.**
   - Per thing, in `scene.things` order, what `t.draws(ctx)` returned (`Emitted`):
     - per shader: the key, `code`, `depthEntry`, `mixedCode`, and the LibFns it names (as raw Fn ordinals);
     - the draw specs (every `DrawSpec` field);
     - the posts it pushed;
     - its notes.
   - **One global ordered log of the side effects** `compileScene` performs while things draw and while the style
     emits: each `slots.register(roots)` with its roots, and each `tables.declare(t)` with the table's Kernel ordinal.
     Lane B replays this log to allocate slots and pack tables in TS order.
2. **Scene decls.**
   - The style's emit requests: every `emitFn` call `styleWgsl` makes (src/compile/scene_wgsl.ts), each with fn name,
     args (name, type), ret type, roots and pick. TS evaluates the style closures under `withIdentities`, exactly as
     today.
   - The lighting the compiler reads, after `lampShadows`.
   - The lamp passes.
   - Posts (label, wgsl, mixedWgsl).
   - Compute passes, every one (gap 12).
   - Recurrences (they are Kernel decls today; check they carry everything `compile/recur.ts` reads).
   - Pass defs, screen passes included (gap 13).
   - Numeric cameras, orthographic included.
   - The loop, `clear`, the outline method.
   - The authored-text registry (gap 16).
   - The cast text, until lane D4 lands (`castPlan`'s `wgsl(cpu, restFrame)` and its libs).
   - Raw Fn flags: the gpu-only flag and the rawCompute resource (gap 9).
3. **The Thing envelope for step 2.** A Thing decl holds kind, name, and an opaque per-kind payload (words, lits, Fn
   ordinals, blob ranges), which the draw lanes define. The tape entry stays beside the payload.
   `crate::draw::<lane>::tape` returns the lane's own generated `probe.tape.<k>.<kind>` text. Your TS side writes the
   recorded one, as `probe/tape.<k>.<kind>.txt`.

**Contract with the other lanes.**

- Keep `Input::graph()`.
- Add typed accessors for each item above, in `src/input.rs`, or an `input/` module beside it, which you own.
- Lanes B, S and D code against these accessors. Publish their signatures in your note as soon as you have them, and
  keep them stable.

**How to observe.**

- Add the smallest observer hooks to `src/compile/index.ts`, `src/compile/bind.ts` (`register`, `emitFn`),
  `src/compile/tables.ts` (`declare`) and `src/compile/scene_wgsl.ts`. A hook is a callback that is absent unless the
  port records.
- Do not move logic.
- Recording must not change one TS artefact byte (the `--mode ts` gate below).
- Recording may make Expr nodes, but only after the artefact is built (`tools/port/ts_side.ts` already orders this).

**Wire 5.** integrate/design brings wire 5 (src/record/wire.ts) to main.

- Use wire-5 tags where they land before you do. Rebase when the integrator says it landed.
- Add tags; never renumber the ones the redesigned path uses: it is parked, not deleted (PLAN.md section 5).
- The canonical print (`src/record/print.ts` `printJob` and `rust/crates/loft-ir/src/print.rs`) must print every new
  decl, byte-identical on both sides. That print is your gate's `probe.wire`. Extending loft-ir's print and decode
  for the new tags is in your lane.

**Files you may change:**

- `src/record/port.ts`, `src/record/tape.ts` (new), and `src/record/{recorder,wire,print}.ts`;
- the observer hooks named above;
- `rust/crates/loft-ir/src/{wire,graph,print}.rs` (new tags only);
- `rust/crates/loft-lower/src/input.rs` (and `input/`).

**Gate.**

1. Before you edit, run `bun tools/port/diff.ts --mode lower --run w-base`.
2. `bun tools/port/diff.ts --scope W --run w` must exit 0. Today an entry whose draws the wire-4 recorder cannot
   record has no job (its `input/refused.json` names what is missing; PLAN.md's proof counts them). After your lane,
   every entry the TS compiler builds or refuses while compiling has a job, and the job's print round-trips.
3. `bun tools/port/diff.ts --mode ts --baseline out/port/w-base --run w-ts` must exit 0: TS bytes unchanged.

**Estimate:** about 700 TS lines and about 1000 Rust lines. Lanes B, S and D1 to D5 wait on your accessors, so publish
the accessor signatures early.

## Context (read first)

Loft (`~/git/loft`) is a 3D rendering library. A scene, written in TypeScript (run by Bun), is compiled into an
**artefact**: a directory with `manifest.json`, `wgsl/` and `bin/` (`docs/artefact_format.md`). `rust/crates/loft-loader`
draws it. We are porting the TS compiler's **pure lowering** to Rust, **1:1**, in the crate `rust/crates/loft-lower`.
"1:1" means the same algorithms translated, with every byte of output identical to the TS compiler's on the whole
corpus (125 examples and 170 seeds). The TS side keeps running the author's closures.

- The plan is `agent_notes/2026-11-19_port/PLAN.md`. Read its sections 2 (the cut and the gaps), 3 (the harness), 4
  (your lane's row and its neighbours) and 6 (risks).
- The harness is `tools/port/diff.ts`, with `tools/port/sections.ts` (sections and owners) and `tools/port/ts_side.ts`
  (what the TS side writes). Read all three before writing code: your gate is that harness scoped to you.
- Read `AGENTS.md` and `CORE.md`.

## Rules (every lane)

- **Where you work.** Work only in your worktree, `~/git/_wt/loft-port-w`, on branch `port/w`. Never
  touch `~/git/loft` itself, another worktree, or another repository. Never kill a process you did not start.
- **Cargo.** Run Cargo only through `tools/dev.sh check|lint|test|build|run loft-lower`. It pins the toolchain, passes
  `--locked`, and takes the machine-wide compile lock shared with Keel. A waiter prints the holder and waits: wait, and
  do not work around it. Never run bare `cargo`. Build the gate's binary with `tools/dev.sh build loft-lower --fast`.
- **Focused checks only.**
  - Run `tools/dev.sh lint loft-lower`.
  - Run `tools/dev.sh test loft-lower`, only if you add tests. Write a test only when you can name the bug it catches;
    the harness is the main check.
  - Run `rtk proxy pnpm exec tsc --noEmit` if you touched TS.
  - Run your gate. While you iterate, use `--only <entry id>` and `--reuse-ts`. Run the unfiltered scoped gate once at
    the end.
  - Never run `pnpm test`, `tools/check.sh full`, or the unscoped whole-corpus harness. The integrator owns those.
- **Commits.**
  - Commit on `port/w`, staging by path (never `git add -A`).
  - End every commit message with these two lines:
    `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`
    `Claude-Session: https://claude.ai/code/session_01SYYs4irpi2G66yaosoU1vg`
  - **Never merge.** Never push to main, and never rewrite another branch.
  - When the integrator says a dependency landed on main, run `git rebase main` on your branch.
- **Translate, do not redesign.**
  - Keep the same algorithm and the same order of side effects. Keep the same names, in snake_case.
  - Write one Rust function per TS function, where the TS has one. Bring along every TS comment that says *why*.
  - Where TS builds WGSL from a string template, port the template's text exactly.
  - Constant WGSL library text belongs to lane LIB's shared `.wgsl` files: `include_str!` it, never retype it.
- **Errors.**
  - Each kind of thing is a variant of a sum type with one exhaustive dispatcher. The workspace denies `_ =>` on
    Loft's own enums.
  - No silent fallbacks. An input you do not handle yet is `Error::Unported { module, lane }`. A scene the TS
    compiler refuses is `Error::Scene(head)`, with the same head the TS compiler prints.
- **Numbers.** Bytes never come from Rust's float `Display`, `format!("{}", f64)`, or serde_json's float output.
  Use the JS-exact spellings: lane G1's `graph::lit` family for WGSL, lane SW's JSON writer for the manifest.
- **Order.** JS `Map` and `Set` iterate in insertion order: use `Vec` or an insertion-ordered map. Do not enable
  serde_json's `preserve_order` feature: feature unification would change loft-ir's and loft-core's JSON.
- **Frozen TS engine.** The TS engine is frozen (AGENTS.md). Change TS under `src/` only where this brief says so.
- **Your note.** Write `agent_notes/2026-11-19_port/W.md`. Cover:
  - what you translated (TS file to Rust file, line counts);
  - the gate's last unfiltered result: passing entries out of the total, and the first failing sections by name;
  - anything you could not make byte-identical, and why;
  - what is left.
- **Final message.** At most 150 words: done or not done (with the reason), and the gate numbers.
