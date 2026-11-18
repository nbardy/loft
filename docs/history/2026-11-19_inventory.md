# Loft archaeology and inventory (written 18-19 Nov 2026)

The owner asked: "Most of Loft is TypeScript? I swear we did it in Rust. When did that happen? Was it never Rust? I
swear at one point it was even Rust programs to compile, but Rust compiles slowly so we moved the runtime to Rust and
the programs' language to TS or something."

Short answer: **the owner's memory is right about the art and the game, and wrong about the Loft compiler.**

- Before Loft, the game's creatures **were Rust programs**: one Rust crate per boss or boat, compiled into the Keel game (1–10 Nov).
- The plan that named Loft (10 Nov, 14:51 JST) said **"Authoring host: TypeScript. Compiler and runtime: Rust."**
- About 6.5 hours later the Loft repo's first commit put **the compiler in TypeScript as well**. The agent changed that
  itself and reported it afterwards ("Earlier I said the compiler would be Rust. I've changed that").
- From then on, only the **game loader** has been Rust on main (`rust/loft-loader`, since 11 Nov).
- A Rust compiler core exists only on the unmerged branch `feat/rust-core` (18 Nov), and covers two kernels.

Sources: git history of `~/git/loft` (679 commits on main, 10–18 Nov), `~/git/room-runners-arena-lib`,
`~/git/creative`, `~/git/_designs/2026-11-18_loft_restart/`, and the Claude session transcript
`~/.claude/projects/-Users-nicholasbardy-git-wave-sim/e6432553-7bca-4903-b961-fca3d7f05ed4.jsonl`. That is the
session in which Loft was designed and built; quotes below give its JST date and its UTC timestamp.

The Codex sessions under `~/.codex/sessions/2026/11/` (about 600 mention Loft) are worker lanes, such as the
gpt-6.2-sol session of 18 Nov 22:15 that wrote the Rust plan. None of them holds a language decision the Claude
session does not.

## 1. Timeline

| Date (JST) | What happened | Language | Source |
|---|---|---|---|
| until mid-Oct 2026 | Room Runners' game client, server and netcode are TypeScript (`packages/client`, `server`, `shared`, `netcode`) | TS | room-runners `keel/README.md` |
| 16 Oct | **Keel starts: the game is rebuilt in Rust** against a frozen TS reference ("Start Keel: Rust workspace and TS-conformant keel_math") | Rust | room-runners `18ab905d`, `1abc2d8a` (wgpu browser client) |
| 1 Nov | **Art as Rust programs.** 15 asset crates (10 boats, 5 bosses) under `keel/lab/art_*`, each drawing triangles into `Scene3D` every frame; a `[profile.lab]` with no LTO and parallel incremental codegen is added "for edit loops" | Rust | room-runners `b11a56b6`, `1c6b842c`, `keel/lab/SHOWROOM.md:19-21` |
| 4 Nov | Bosses enter the game through their art crates (`keel/games/art/art_boss_*`, bosses M1b–M5) | Rust | room-runners `cdbf6bec` … `992b8266` |
| 10 Nov, 04:30–05:28 UTC | Owner complains about compile loops. He asks why a fresh worktree "compiled 209 crates from scratch" and why "a render round [is] a minute", and whether "we might want to move code out of rust". He also remarks: "The 18 s is dependency shape … I thought we fixed this?" | — | transcript, 2026-11-10T04:30, 05:19, 05:28 |
| 10 Nov, 14:26 JST | `notes/design/2026-11-10_creature_pipeline_options.md` §3 measures the loop: 18.4 s to rebuild after touching one boss (it drags in `game_client`), 0.4–0.8 s to compile a boss crate alone, 2.5–3.3 s edit-to-pixels with `preview_probe`. Verdict on the owner's question: "*Move code out of Rust to go faster?* Not for speed." | Rust | room-runners `2c4bd4b1` |
| 10 Nov, 14:51 JST | §12 of the same note names Loft and splits languages: **"Authoring host: TypeScript (PROPOSED)** … no compile step … the same code runs in a browser beside a WebGPU viewer. **Compiler and runtime: Rust.** Graph to shader code, graph to a CPU evaluator … and the loader inside the game … also builds to WASM." | TS authoring, Rust compiler (plan) | room-runners `74f22b5c`; transcript 2026-11-10T05:51 |
| 10 Nov, day | Rust prototypes of Loft ideas in Keel's lab: `preview_probe/strand_kit.rs` (a serpent from 62 named numbers), `loft_sea`, `loft_spacetime`, `loft_blend` (about 5,900 lines). They were committed in the evening | **Rust** | room-runners `8033a3c5`, `4e76b713`, `437052c8`, `ad205e2a` (21:12–21:47 JST) |
| 10 Nov, 21:29 JST | **Loft repo first commit.** DESIGN v0 reads "authoring code (TypeScript) → the compiler (TypeScript) → a host … later: Rust/wgpu inside the game". The artefact holds WGSL plus **a generated JavaScript per-frame pose function** | **TS compiler** | loft `be38e3a` (`DESIGN.md:11-18`) |
| 10 Nov, 12:31 UTC | The agent explains the switch: "Our compiler, also written in TypeScript … Rust comes in later, and only as a small loader … **Earlier I said the compiler would be Rust. I've changed that**: keeping the compiler in TypeScript means the Rust side needs no compiler at all." | — | transcript 2026-11-10T12:31 |
| 10 Nov, 12:46 UTC | The agent offers the owner a choice, framed as Rust *authoring*: "if you'd rather have the whole thing in Rust, authoring included … a creature edit would cost the 0.4–0.8 s Rust compile … I'd stay with TypeScript authoring and a Rust loader … Say so if you want it all-Rust." The owner never answers it. His next messages ask about Unity and Blender, then "Lets ge it all implimented". | — | transcript 2026-11-10T12:46, 12:59 |
| 11 Nov | First redesign. `docs/redesign_proposals.md` lists the problems: "Runs in the game's Rust client: no: an artefact contains a JavaScript function". Owner: "no not on 3 js, gpu native loft sseems right". **The pose moves to GPU compute** (`gpu/pose`) and the artefact becomes files (`gpu/files`) | TS compiler, WGSL runtime | loft `9dfc603`, `fbe6242`, `f85a079`, `2776620`; transcript 2026-11-11T07:03 |
| 11 Nov, 08:10–08:58 UTC | Owner: "so we compile to sahders now? from a TS lang? Any reason to not just write the shaders directly then?" Then: "**load and compile isn't bad we're mostly worried about runtime right now**." Compile speed is deprioritised from here on | — | transcript 2026-11-11T08:10, 08:58 |
| 11 Nov | **Rust loader**: `rust/loft-loader` (wgpu 30) plus `loft-view`. Same pictures as the Dawn host to within 98 edge pixels; GPU frame 8–14% slower, cause not measured | **Rust** (loader only) | loft `15a6b0d`, `b75a2c5`, `8b6249a` |
| 12–13 Nov | Library extraction, a second subject, and the `loft context` training surface; README and a comparison with three.js. Tag `train-1` = `a2d82a2` (13 Nov) | TS | loft `dca8074`, `1c3d0ac`, `ef4033b` |
| 13 Nov | Creative freezes **a copy of the TS library** for the first RL baseline (`frozen-loft` at loft `d80eaed`, retry at `a3c7612`) and adds its own TS host with a sandbox and a literal-index rule | TS copy | creative `training/loft_reward_v2/*/manifest.json` (`loft_head`) |
| 13–16 Nov | Five proposals (light, materials, geometry and rays, state, headroom), each "nothing built". Then built: geometry and rays, `recur`, `pass`, light and materials | TS | loft `a2d82a2`, `6d458e3`, `1a20151`, `f01ca2c` |
| 14 Nov | Rust loader reads artefact format v2. `rust_compare` names `--variant stilt` (15 Nov) | Rust | loft `29cf3b0`, `526f03c`, `aa10d80` |
| 15–16 Nov | Creative speeds up **its own copy** (shared draw shaders, concurrent pipelines, compile interning, a warm render server); loft ports it back. A fourth branch (`loft-fast-shaders`, a fixed interpreter shader) is blocked at its gate | TS | creative `c6ac48f3`, `bc65ff4b`, `050b995a`, `84b897b9`; loft `8b9c82e`, `8d70774` |
| 18 Nov, 02:57–05:51 UTC | **Restart review**: five Claude and ten Codex designs plus a blind judge. Three of the five Claude designs converge on "no per-scene compile … Rust/wgpu/Vulkan". The ten Codex designs converge on 4-D convex hulls. Plan: "keep Loft and add to it" | — | `~/git/_designs/2026-11-18_loft_restart/IDEAS_SO_FAR.md`, `judge/VERDICT.md`; loft `88164f1` |
| 18 Nov, 10:03 UTC | Owner: "what you mean gated on linux? … we use WGPU and rust and its buildable and runnable cross system right?" Agent: "It's not Rust end to end: The compiler … is TypeScript on Bun … Rendering in training … goes through Dawn … The game side has a Rust loader" | — | transcript 2026-11-18T10:03–10:04 |
| 18 Nov | Nine feature lanes, then authoring-API waves L1–L7 merged, all TS: +9,480 `src` lines in one day (36,459 → 45,939). `feat/pipecache` adds a Vulkan pipeline cache to the Rust loader | TS (+ Rust loader) | loft `07eb673` … `87c8ad1`; `abfa3f9` |
| 18 Nov, 12:10 UTC | Owner: "why is the typescript compile so slow? … Can't we impliment that in rust and make it like 100ms?" | — | transcript 2026-11-18T12:10 |
| 18 Nov, 21:59 JST | **Rust compiler core** on `feat/rust-core` behind `LOFT_CORE=rust`: `solve` rounds and shader sharing only. Heron 2,274 → 791 ms CPU; Titan 2,128 → 1,475 ms; Owl 315 → 286 ms. The agent: "my ~100 ms theory was wrong … most of the time is the scene's own code building the graph in JavaScript." **Not merged.** | Rust (kernels) | loft `4ec1422`, `af5cf24`, `d703340`; transcript 2026-11-18T13:12 |
| 18 Nov, 13:14 UTC | Owner: "did we end up implimeting a bunch of the code in typesirpt? I thouhgt fully implimented in rust but the DSL is in TS". Agent: "No. Almost all of Loft is still TypeScript, and the only Rust is two small pieces." | — | transcript 2026-11-18T13:14–13:15 |
| 18 Nov, 22:26 JST | Plan for the full port (`docs/proposals/2026-11-19_rust_loft.md`, branch only). TS keeps the scene language as a recorder; Rust owns the graph, CPU twin, proofs, `solve`, WGSL, tables and artefact writing, plus a headless wgpu draw worker | plan | loft `97bc1eb` |

Size today (main `87c8ad1`): `src` **45,939** TS lines, `examples` 16,469, `tests` 8,853, `tools` 6,030; Rust 2,933
lines (2,226 of them the loader written on 11 Nov). Growth of `src` TS by day:

| 10 Nov | 11 Nov | 12 Nov | 13 Nov | 15 Nov | 16 Nov | 18 Nov |
|---:|---:|---:|---:|---:|---:|---:|
| 7.5k | 23.5k | 28.2k | 30.3k | 34.8k | 36.5k | 45.9k |

## 2. Was it ever Rust?

What ran in Rust, and when:

| Piece | In Rust? | When | Where |
|---|---|---|---|
| The game (sim, net, client, UI, wgpu renderer) | Yes, ported from TS | from 16 Oct | `room-runners-arena-lib/keel/` |
| **Creature and boat art as programs** | **Yes**: one crate per asset; triangles rebuilt on the CPU every frame; private kits totalling 18.9k lines for five bosses | from 1 Nov; still the game's art today | `keel/lab/art_*`, then `keel/games/art/art_*` |
| Loft-concept prototypes (strand kit, curved scene graph, space-time attacks, junction blends) | Yes, lab binaries | 10 Nov | `keel/lab/{preview_probe,loft_sea,loft_spacetime,loft_blend}` |
| Loft compiler | **Planned in Rust (14:51 JST, 10 Nov) but never built in Rust on main.** Built in TS from the first commit | 10 Nov → today | `src/compile`, `src/expr.ts`, `src/host/lower.ts` |
| Loft per-frame pose | JS function on 10 Nov; GPU compute from 11 Nov | — | `f85a079` |
| Loft game loader | Yes | from 11 Nov | `rust/loft-loader` |
| Loft compiler kernels (`solve`, shader sharing) | Yes, unmerged, behind a switch | 18 Nov | `feat/rust-core`: `rust/loft-core` via `bun:ffi` |

So the owner remembers two moves, and both happened, though not as he recalls them.

**First move: the game moved TS → Rust (October).**

**Second move: the creature programs moved Rust → TS (10 Nov).** The reason given was iteration, not raw compile
speed. The agent had measured that the slow loop came from dependency shape. Its words: "the cause of the 18 s is
dependency shape, not Rust".

TS was chosen because:

- authoring needs "no compile step";
- its types can describe the graph;
- the same code runs beside a browser WebGPU viewer with sliders.

The intended split was "TS authoring, Rust compiler and runtime". The compiler going to TS as well was an agent
decision. The agent justified it as "the Rust side needs no compiler at all". The owner was offered "all-Rust,
authoring included" but not "TS authoring with a Rust compiler", and he never answered. The runtime did move off TS:

- on 11 Nov, pose went to GPU compute and artefacts became WGSL plus tables;
- Rust/wgpu loads them in the game.

Keel itself still does not consume Loft. The 18 Nov weapon-art note says that the `loft_*` lab binaries have no game
crate depending on them, and records "nothing moved from LOFT". The game's art crates are still Rust (room-runners
`notes/design/2026-11-18_tidewar_class_weapon_art.md:158-160`).

## 3. Where it went wrong

Each item is marked as deliberate (D) or drift (R).

1. **(D, by the agent) The compiler went to TS against the written plan** (10 Nov, 12:31 UTC). The agent stated it
   after the fact. The choice it then offered mixed up the authoring language with the compiler language. That one
   choice decided where every later line of code went.
2. **(D, then a rewrite) The first artefact carried a JavaScript pose function.** The Rust client could not run it
   ("Runs in the game's Rust client: no", `docs/redesign_proposals.md`). Day 2 went into GPU pose and artefacts as
   files (`f85a079`, `2776620`). That was the right move, but it was a rewrite of a day-1 shortcut.
3. **(D, owner) Runtime over compile.** On 11 Nov the owner said "load and compile isn't bad". So compile cost in TS was
   never a gate while 38k lines were added. It came back on 18 Nov as the training bottleneck: about 1–1.5 CPU s per
   sample, and 21 µs and 2.8 KB per node on Titan (`docs/proposals/2026-11-18_compiler_speed.md`).
4. **(R) Two GPU executors.** Training and `loft serve` draw through Dawn (C++, via bun-webgpu); the game draws
   through wgpu (Rust). Pixel parity and features (pipeline cache, f16, format versions) have to be built twice. The
   Rust loader's 8–14% GPU gap was never explained (`8b6249a`).
5. **(R) Forks in creative.** Creative did four things:
   - froze the TS library on 13 Nov;
   - tracked a copy on 15 Nov;
   - optimised that copy (four lanes);
   - added host rules of its own (literal-index-only, a stations clamp).
   By 16 Nov, 58 files shared a path but differed (creative `agent_notes/2026-11-18-variance-and-reward/NEXT_RUN.md`).
   The fixes were all after the fact: a port back (`8d70774`), a submodule pin, and a 170-seed migration on 18 Nov.
6. **(D, then drift) Proposals reversed.** On 13 Nov all five proposals agreed on "Physically based light only as a
   separate offline tool" (`docs/proposals/README.md`, decision 6). On 18 Nov PBR shipped as a style (`a5573e7`).
   Likewise:
   - Tier I (the GPU interpreter) passed its probe and was then frozen the same day (`2026-11-18_next.md` §8);
   - creative built a fixed interpreter shader separately (`loft-fast-shaders`, blocked).
7. **(R) The TS surface grew fastest on the day the owner wanted Rust.** On 18 Nov, +9.5k TS lines (nine lanes plus
   waves L1–L7) landed while the Rust core lane worked under the rule "behind a switch, without rewriting existing
   files". So it could only port kernels. Its own measurement shows the cost sits in JS graph construction, which no
   kernel port touches.
8. **(R) Graph inflation and duplicated work.** Every function call is inlined (183k nodes for Titan), and operand
   checks re-run on each arithmetic call. These are algorithmic costs that came from fast lane-by-lane building, not
   from the language.
9. **(R) Keel was the stated customer but was never integrated.** Loft's examples re-did Keel's bosses in a new style
   (`boss/*` branches, `examples/bosses/`). The game kept its Rust art crates, and no Keel crate links `loft-loader`.

## 4. Inventory: pending, unfinished and planned work concerning Loft

### Loft (`~/git/loft`)

| Item | State | Source |
|---|---|---|
| Rust compiler core (`solve`, share) behind `LOFT_CORE=rust` | branch, 4 commits, unmerged | `feat/rust-core` `4ec1422`…`97bc1eb` |
| Full Rust port: TS recorder, Rust graph/twin/checks/solve/WGSL/tables/writer, wgpu headless worker | plan only | `docs/proposals/2026-11-19_rust_loft.md` (on `feat/rust-core`) |
| Make emitted WGSL names independent of creation order (unblocks bytecode bundle, warm workers, the port's byte baseline) | queued | same plan, step 1; transcript 2026-11-18T13:12 |
| Faster TS graph construction (operand checks, repeated callbacks) | queued speed lane | transcript 2026-11-18T13:12 |
| Speed lanes: Dawn disk pipeline cache, warm confined compile worker, `solve` cache across processes, `steps` baked at build, branch-local `if` | not started | `docs/proposals/2026-11-18_next.md` §8 step 6 |
| Training budgets enforced as typed `budget` exits | with step 6 | same, step 6b |
| Blind trial (24 briefs, Loft vs three.js vs raw shaders; pass at 17) | can start, not started | same, step 7 |
| `loft check --explain`, exact structural cache | `--explain` merged (L7); cache open | same, step 8; loft `87c8ad1` |
| Later: f32-padded enclosures, checks over player inputs, picture facts, `recur` bake in serve | deferred | same, step 9 |
| and/or of two quantified checks | not built | same, step 1 note |
| Tier I GPU interpreter (whole-scene probe) | frozen 18 Nov; paint probe on unmerged `loft/interp-probe` | `2026-11-18_next.md` §8; branch `loft/interp-probe` |
| f16 `--precision mixed` | merged, opt-in, frozen | `2026-11-18_next.md` §8 |
| Authoring API wave 4, L8: `loft context` and guide taught text-first | worktree `_wt/loft-api-l8`, 6 dirty files, no commits | `docs/proposals/2026-11-18_authoring_api.md`; `git worktree list` |
| Migration worktree (`feat/migration`) | 5 dirty files, no commits | `_wt/loft-migration` |
| Uncommitted work on main (41 paths, incl. `agent_notes/2026-11-18_cook/*`, `tools/crab_*.py`) | dirty | `git status` |
| `context-version`: first line of `loft context` names version and commit | unmerged, 1 commit | branch `context-version`, rescue branch |
| `training-host-contract` (view cameras resolved before GPU load) | unmerged, 5 commits | branch `training-host-contract` |
| Body plans art direction (paused work) | unmerged, 2 commits | branch `art/body-plans` (`docs/art_direction/DIRECTION.md`) |
| Explain the Rust loader's 8–14% slower GPU frame | open | `8b6249a` |
| Outsider blockers: 380-name flat entry point, no reader ladder, private package, load time grows with content, history names in public types | open | `docs/inventory.md` §10 items 2–7 |
| Extractions: shots/deliverables (`src/film/`), style parts, legged body kit, creature kit, one shell, tool checks, predict-before-render helper, music | open (item 1 done) | `docs/inventory.md` §9 |
| Missing short examples (custom style, scene as files incl. the Rust loader, clip to share) | open | `docs/inventory.md` §11 |
| Differentiable Loft | deferred | `docs/differentiable.md`; `docs/handoff_training.md` "Deferred" |
| Not doing: space-time convex hulls, Bevy authoring, path tracer, checks as reward | decided | `2026-11-18_next.md` §9 |
| Restart-review experiment (64 engineer-hours, exact-and-affordable test of D10) | proposed, unfunded | `~/git/_designs/2026-11-18_loft_restart/judge/VERDICT.md` |
| LOFT vs LOFT-Scene split (objects vs rendering; glTF/three.js export) | raised by owner 14 Nov, unresolved | transcript 2026-11-14T04:18, 04:24 |

### Creative (`~/git/creative`)

| Item | State | Source |
|---|---|---|
| `loft/vendor-submodule` (pin `b5c8abe`, allowlist ~750 names) | unmerged; push blocked by a 477 MB file | branch; `2026-11-18_next.md` step 5b |
| `loft/migrate` (training and seed generation on pinned Loft `e7c3c5a`, 170 seeds) | unmerged | branch |
| `loft/reconstruct-migrate` (reconstruction pipeline on the training Loft API) | unmerged | branch; transcript 2026-11-18T13:12 |
| `loft/expedition-base`, `wrap-variance`, `wrap-order`, `wrap-improve`, `wrap-drex`, `staging-proof`, `staging-final` | unmerged (5–21 commits each) | branches |
| `loft-fast-shaders` / `-int` (shaders as data, fixed interpreter shader) | blocked: fails `render.same()` on 2 of 30 seeds | branches; transcript 2026-11-16T13:05 |
| `loft-fast-server` evidence, `loft-seeds-features`, `loft-seeds-styles` reports | unmerged | branches |
| `loft-curriculum-order`, `-order2` (stations clamp moved to host) | unmerged; run stopped on budget | branches; transcript 2026-11-16T13:06 |
| `research/loft-unified`, `research/loft-gallery` | unmerged | branches |
| Six `rescue/*` branches of uncommitted Loft worktrees (14, 16, 18 Nov) | unreviewed | branches |
| Next training run: decision criteria, Inkling baseline, cost cap | pending owner authorisation | `agent_notes/2026-11-18-variance-and-reward/NEXT_RUN.md` |
| Resume `reconstruct-curated-v1` (3 of 1,504 done) on the new Loft | paid, waits for go | `art/reference_sculptures/loft/README.md` |
| Mixed-task RL first; SFT warm-up only if few runnable candidates | decided 18 Nov | same README, "Saved seed and training decision" |
| Retire the frozen library copies (`training/loft/library`, `frozen-loft` at `d80eaed`/`a3c7612`) | pending migration | `training/loft/library/FROZEN_DIFF.md` |

### Room Runners (`~/git/room-runners-arena-lib`)

| Item | State | Source |
|---|---|---|
| Keel consuming Loft artefacts (link `loft-loader`, one boss integration) | not started; "nothing moved from LOFT" | `notes/design/2026-11-18_tidewar_class_weapon_art.md:158-160`; restart verdict "one Keel boss integration" |
| Lab Loft prototypes `loft_sea`, `loft_spacetime`, `loft_blend`, `preview_probe` | lab only; clippy errors noted 11 Nov | `agent_notes/2026-11-11_swarm-ai.md:44-45` |
| Creature pipeline designs A/B/C (lean Rust kit, bake-and-skin, GPU SDF) and Rust engine options | superseded by Loft, never closed in that repo | `notes/design/2026-11-10_creature_pipeline_options.md` |
| Art edits recompile `game_client` (dependency shape) | fix proposed (sim-free art module), state unknown | same note, §3 and around line 241 |
| Literal-index rule report from creative | answered (creative's host rule, not Loft's) | `agent_notes/2026-11-13_loft-literal-index-report.md` |
| `room-runners-arena-lib-lean-scope`, `room-runners-evidence` | no Loft content | searched |
