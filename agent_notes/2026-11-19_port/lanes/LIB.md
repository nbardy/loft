# Port lane LIB: shared WGSL library text

Branch `port/lib`, worktree `~/git/_wt/loft-port-lib`, plan `agent_notes/2026-11-19_port/PLAN.md`.

## Your lane: LIB, one .wgsl source for every constant WGSL library text

**Target.** Constant WGSL lives in one `.wgsl` file under `src/wgsl/`. TS reads it, and Rust `include_str!`s it in
`rust/crates/loft-lower/src/wgsl.rs` (one `pub const NAME: &str` per text). No lane ever retypes WGSL.

**What is constant** (move it):

- `PRELUDE_WGSL`, `SCENE_DEPTH_WGSL`, `SCENE_CLEARANCE_WGSL`, `POST_WGSL` (src/artefact.ts);
- `CORE_WGSL` (src/compile/scene_wgsl.ts);
- `MARK_WGSL`, `INK_POST` (src/draw/ink.ts);
- `DEPTH_RESOLVE_WGSL`, `CLEARANCE_WGSL` (src/host/lower.ts);
- `POST_COMMON` (src/style/shared.ts);
- `GAUSS_WGSL` (src/soft/gauss.ts);
- `GAUSS_LIB`, `BANK_LIB` / `BANK_SPLIT_WGSL` (src/soft/medium.ts);
- `camera_ray` (src/text/raw_prelude.ts);
- the `LibFn.wgsl` texts of src/lib/noise.ts, src/style/light.ts (3), src/style/papercraft.ts (`pc_facet`) and
  src/style/whitewater.ts.

**Constant holes.** A text with a hole that is itself a constant has the constant written into the `.wgsl` file:
`${DETAIL_SLOTS}` and `${CURVE_N}` in the prelude, `${CLEARANCE_TILE}` and the others. A TS test or a const
assertion keeps the TS constant and the file's number equal.

**Data holes.** A text whose holes are data (`src/style/post.ts`, `soft/rain.ts`, `soft/specks.ts`) is a generator.
Leave it: the lane that owns it ports the template.

**How TS reads the files.**

- Bun reads them with `import text from "../wgsl/x.wgsl" with { type: "text" }`. Check the browser viewer's bundler
  too (`viewer/`, and src/host is used in the browser; `src/host/share.ts` says so).
- If one mechanism does not serve both, choose the one that does, and say why in your note.
- No file may change a byte of what TS emits: no trailing newline appears or disappears. Strip or keep the newline
  exactly as the TS literal had it.

**Files you may change:** `src/wgsl/` (new), the constant sites listed above (the constants only), and
`rust/crates/loft-lower/src/wgsl.rs`.

**Gate.**

1. Before you edit, run `bun tools/port/diff.ts --mode lower --run lib-base`. This records the TS side of every entry.
   Its failures are expected: lane LW has not landed.
2. After your change, run `bun tools/port/diff.ts --mode ts --baseline out/port/lib-base --run lib`. It must exit 0:
   every TS byte unchanged.
3. Run `tools/dev.sh lint loft-lower`.
4. Add one Rust test that each `wgsl.rs` constant equals the TS export, compared through a `bun -e` print. This catches
   a file edited on one side only.

**Estimate:** about 150 TS lines touched and about 120 Rust lines. It does not depend on P0.

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

- **Where you work.** Work only in your worktree, `~/git/_wt/loft-port-lib`, on branch `port/lib`. Never
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
  - Commit on `port/lib`, staging by path (never `git add -A`).
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
- **Your note.** Write `agent_notes/2026-11-19_port/LIB.md`. Cover:
  - what you translated (TS file to Rust file, line counts);
  - the gate's last unfiltered result: passing entries out of the total, and the first failing sections by name;
  - anything you could not make byte-identical, and why;
  - what is left.
- **Final message.** At most 150 words: done or not done (with the reason), and the gate numbers.
