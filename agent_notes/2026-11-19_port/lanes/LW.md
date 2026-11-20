# Port lane LW: lower.ts: the Artefact to the manifest

Branch `port/lw`, worktree `~/git/_wt/loft-port-lw`, plan `agent_notes/2026-11-19_port/PLAN.md`.

## Your lane: LW, src/host/lower.ts and the artefact types

**Target.**

- **`rust/crates/loft-lower/src/artefact.rs`.** src/artefact.ts's types, read from compile.json (the shape
  `tools/port/sections.ts` `plain` writes). Typed arrays arrive as base64. String-keyed maps keep the text's key order
  (`Draw.constants`, `cameras`, `shaders`): use `Vec<(String, T)>` with a visitor, not a sorted map.
- **`rust/crates/loft-lower/src/lower/`.** `lower(compile_json, baked, out)`. Translate src/host/lower.ts 1:1:
  - `checkArtefact`, `formatVersion`, `checkPasses`;
  - chunk interning (lane SW's `chunk_name`);
  - blobs;
  - buffers, in id order;
  - layouts and bind groups;
  - pose dispatches;
  - author compute (build, load, frame, steps; the `baked` map);
  - modules and render pipelines (lane SW's `share`), with the dedupe key;
  - passes: depth resolve, clearance (text from lane LIB), view and off-screen passes;
  - `gridBounds`/`slotsUsed` from src/host/detail.ts;
  - the binding-limit check;
  - the manifest, in lower.ts's key order;
  - the mixed overlay.
  - src/host/manifest.ts's types become Rust structs whose field order is the manifest's key order, with absent keys
    skipped.

**Baked inputs** (`input/baked/<name>.bin`): `recurrence.<name>` holds a recurrence's baked states, which overwrite
its static data, and `build.<key>` holds a compute build's output (the second `lower` call in buildTs).

**Interface with lane SW.** You call SW; SW does not call you.

- `crate::files::chunk_name(text: &str) -> String`
- `crate::files::write_bundle(dir, manifest_json: &str, chunks: &[(String, String)], blobs: &[(String, Vec<u8>)]) -> Result<(), Error>`
- `crate::files::to_json<T: serde::Serialize>(&T) -> String`: JS-exact `JSON.stringify`.
- `crate::share::share(&ShareInput) -> Sharing`.

`ShareInput` is narrow: `scene_wgsl`, `shaders` in order, and per draw `shader`, `constants`, `instances`. `Sharing`
mirrors src/host/share.ts: `module_of(shader)`, `draw_of(k)`, `rows`. Until SW lands, stub them locally in your branch,
then rebase.

**Files you may change:** `rust/crates/loft-lower/src/lower/`, `src/artefact.rs`.

**Gate.** `bun tools/port/diff.ts --mode lower --scope LW --run lw` must exit 0. This mode lowers the TS compiler's own
compile.json, so you never wait for the compile lanes.

**Estimate:** about 1500 Rust lines, translated from lower.ts (950), manifest.ts (263) and detail.ts (30).

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

- **Where you work.** Work only in your worktree, `~/git/_wt/loft-port-lw`, on branch `port/lw`. Never
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
  - Commit on `port/lw`, staging by path (never `git add -A`).
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
- **Your note.** Write `agent_notes/2026-11-19_port/LW.md`. Cover:
  - what you translated (TS file to Rust file, line counts);
  - the gate's last unfiltered result: passing entries out of the total, and the first failing sections by name;
  - anything you could not make byte-identical, and why;
  - what is left.
- **Final message.** At most 150 words: done or not done (with the reason), and the gate numbers.
