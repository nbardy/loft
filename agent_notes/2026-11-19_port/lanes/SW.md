# Port lane SW: share.ts, chunk names, the writer

Branch `port/sw`, worktree `~/git/_wt/loft-port-sw`, plan `agent_notes/2026-11-19_port/PLAN.md`.

## Your lane: SW, src/host/share.ts and the artefact writer

**Target.**

- **`rust/crates/loft-lower/src/share.rs`.** src/host/share.ts 1:1:
  - the tokenizer (its `TOKEN` regex, char for char);
  - `numberKind` and `numberValue`: decimal to f64 to f32, never parsed straight to f32;
  - accessors;
  - `read`, the holes and `folds`;
  - `groupWgsl` and `plumb`;
  - `share`, with group keys `shared N (members)` and the row dedupe on `JSON.stringify(row)`.
- **`rust/crates/loft-lower/src/files.rs`.**
  - `chunk_name`: src/host/lower.ts `chunkName`, two FNV-style hashes over **UTF-16 code units**.
  - `write_bundle`: src/host/files.ts `writeBundle`. It writes `manifest.json` compact with no trailing newline,
    `wgsl/<name>.wgsl` and `bin/<name>.bin`. It refuses `cpu` tables by name, refuses stray files, and recreates
    `wgsl/` and `bin/`.
  - `to_json<T: Serialize>`: a serde Serializer that writes exactly what `JSON.stringify` writes:
    - integers without `.0`;
    - the exponent from 1e21 up and below 1e-6 (`1e+21`, `1e-7`);
    - `-0` written `0`;
    - JS string escaping (lone surrogates);
    - non-finite numbers refused, as files.ts refuses them.

**Interface.** It is fixed, because lane LW calls it in parallel: see LW's brief, "Interface with lane SW".

**Files you may change:** `src/share.rs`, `src/files.rs`.

**Gate.**

- `bun tools/port/diff.ts --mode lower --scope SW --run sw` must exit 0. It covers the `format`, `version`, `chunks`,
  `blobs` and `authored` keys, shared chunks, `manifest.json` whole, and the file set.
- Your sections need LW's lower to exist. Until it does, test `to_json` and `chunk_name` directly:
  - a Rust test over every `out/port/<run>/entries/*/ts/artefact/manifest.json` (parse with serde_json, re-emit with
    `to_json`, compare bytes; this needs struct field order, so do it through `serde_json::Value` with care, or over
    LW's structs once they land);
  - a Rust test over every chunk file name (`chunk_name(text)` equals the file's stem).

  Name the bug each test catches.

**Estimate:** about 900 Rust lines, translated from about 380 TS lines plus the JSON writer.

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

- **Where you work.** Work only in your worktree, `~/git/_wt/loft-port-sw`, on branch `port/sw`. Never
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
  - Commit on `port/sw`, staging by path (never `git add -A`).
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
- **Your note.** Write `agent_notes/2026-11-19_port/SW.md`. Cover:
  - what you translated (TS file to Rust file, line counts);
  - the gate's last unfiltered result: passing entries out of the total, and the first failing sections by name;
  - anything you could not make byte-identical, and why;
  - what is left.
- **Final message.** At most 150 words: done or not done (with the reason), and the gate numbers.
