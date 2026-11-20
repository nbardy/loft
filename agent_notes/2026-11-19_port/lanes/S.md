# Port lane S: scene assembly: compileScene after the drawables

Branch `port/s`, worktree `~/git/_wt/loft-port-s`, plan `agent_notes/2026-11-19_port/PLAN.md`.

## Your lane: S, src/compile/index.ts and its scene-level helpers

**Target.**

- `rust/crates/loft-lower/src/compile/mod.rs`: `compile`. It replaces the `Unported` after `tables::replay`.
- `compile/scene_wgsl.rs`: `styleWgsl`, `CORE_WGSL` through lane LIB, `outlineOf`.
- `compile/passes.rs`: `discover`, `planPasses`, and the WGSL reach regexes.
- `compile/frame.rs`:
  - the lowering in src/frame/pass.ts (camera tables, accessors) and src/frame/lamp_shadow.ts;
  - src/draw/grid_detail.ts `measureRest`;
  - compile/index.ts `detailSlots` and `refuseSpread`.

**What `compileScene` does after `t.draws(ctx)`, in TS order:**

- the ink post;
- style WGSL from lane W's recorded emit requests, emitted with lane B's slots;
- pass discovery, with camera tables declared through B;
- the cast plan (until lane D4: the recorded cast text);
- the stage reader check;
- `seal`;
- the shared code and `sceneWgsl`, with own libs per module and `textOf`;
- the shaders and the mixed shaders;
- the posts;
- draws stably sorted by phase, then detail slots (rest-frame measurements through lane G2), constants
  (`LOFT_DETAIL_SLOT`) and `depth`;
- passes;
- stages with the compute tail;
- triangle stats;
- the notes;
- `refuseSpans`.

Push the remaining Artefact fields: `version`, `clear`, `sceneWgsl`, `mixed`, `notes`, `shaders`, `draws`, `post`,
`passes`, `cameras`, `loop`, `stats`.

**Drawables.** During step 1 their `Emitted` comes from lane W's tape. When a draw lane lands, take its kinds'
generated `Emitted` instead. The dispatcher is one exhaustive match over the Thing kinds.

**Files you may change:** `compile/mod.rs`, `compile/scene_wgsl.rs`, `compile/passes.rs`, `compile/frame.rs`. Also
`src/main.rs`, `src/lib.rs` and `src/error.rs`, for driver changes only.

**Gate.** `bun tools/port/diff.ts --scope S --run s` must exit 0. When B, LW and SW have also landed, the unscoped
`bun tools/port/diff.ts` is step 1's finish line.

**Estimate:** about 1100 Rust lines, translated from about 900 TS lines. It starts when B's `Replayed` exists.

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

- **Where you work.** Work only in your worktree, `~/git/_wt/loft-port-s`, on branch `port/s`. Never
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
  - Commit on `port/s`, staging by path (never `git add -A`).
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
- **Your note.** Write `agent_notes/2026-11-19_port/S.md`. Cover:
  - what you translated (TS file to Rust file, line counts);
  - the gate's last unfiltered result: passing entries out of the total, and the first failing sections by name;
  - anything you could not make byte-identical, and why;
  - what is left.
- **Final message.** At most 150 words: done or not done (with the reason), and the gate numbers.
