# Loft handoff: state on 20 Nov 2026, 16:00 (after a machine restart)

For whoever picks this up. This page should be enough to continue without the conversation that produced it.
Repo: `~/git/loft`, public at https://github.com/nbardy/loft (main). The text-only `review` branch is for
model import under 100 MB.

## 1. What Loft is, and what we were doing

Loft is a **3D meta-engine designed for LLM generation** (README top: the owner's words). It has four parts:
- **Lang:** E+ `.loft`, a thin layer over WGSL and wgpu with escape hatches;
- **Libs:** the std library;
- **Motion;**
- **Core:** a Rust compiler plus a wgpu runner, native or WASM.

The owner's stance is in [docs/PHILOSOPHY.md](../../docs/PHILOSOPHY.md): hard code is cheap and lock-in is expensive,
everything is forkable, and the frame time on the GPU comes first.

Timeline, 19–20 Nov:
1. Rewrote Loft from scratch in Rust to the chief scientist's spec (`docs/spec/`). Deleted the old TypeScript engine
   and the old Rust port; old code survives only at tag `legacy/2026-11-19` and `archive/*` tags.
2. Released **v1.0-alpha** (tag) and ran about 15 Codex waves overnight and during the day: phases 2–5, fixes,
   design passes, perf, lean, stdlib.
3. Ran a critique round with the chief scientist. The result is **v1.5**: [docs/V1_5_GOALS.md](../../docs/V1_5_GOALS.md),
   with decisions R1–R9 at the end superseding the earlier rows.
4. At about 15:36 the machine restarted mid-wave. Nothing is running now.

How the work runs (agent_notes/2026-11-20_codex/):
- **Workers are Codex CLI:** gpt-6.2-sol implements lanes, and gpt-6.2-astra does design, review and integration.
  Claude only plans and summarises, to save credits.
- **`run.sh <step> <model> <effort> <cwd>`** runs one step from `<step>.brief.md`, logs to `<step>.log`, and writes
  `<step>.final.md` and `<step>.done` (the exit code). It is **resumable:** it skips a step that has a `.done`.
  It also:
  - honours a first-line `Effort: low|medium|high|xhigh`;
  - waits while free memory is under 15%;
  - scales CARGO_BUILD_JOBS with free memory;
  - caps concurrent steps at `LOFT_MAX_STEPS` (default 6). Before the restart the load average reached 41–56 on
    10 cores, so use 4.
- **Conductors** (`wave4.sh`, `wave15.sh`, …) run lanes in parallel, then one integrator. The integrator audits,
  merges the passing lanes, runs one GPU gate and updates OPEN.md.
- **Lane rules** are in the template `w1.brief.md`:
  - a cambium workspace per lane under `~/git/_wt/`;
  - iterate `cargo check` until it compiles; at most 2 full test or GPU reruns;
  - short notes; one paired timing run;
  - never self-approve looks.
- **Picture approvals belong to the lead** (owner's delegation), recorded in [APPROVALS.md](APPROVALS.md).

## 2. Goal and decision documents (read in this order)

| Doc | What it is |
|---|---|
| [docs/V1_5_GOALS.md](../../docs/V1_5_GOALS.md) | **The current plan:** checkable targets P0–P7, the pending-branch table, and decisions R1–R9 |
| [OPEN.md](OPEN.md) | Every open target with measured values; routing; in-flight claims; std anti-patterns |
| [NOTES_AND_TODO.md](NOTES_AND_TODO.md) | The reasoning behind decisions (syntax, macros vs passes, commands, escape hatches), the TODO |
| [INTEGRATION_TODO.md](INTEGRATION_TODO.md) | Three.js, Unity, glTF export, material export, motion-twin library |
| [APPROVALS.md](APPROVALS.md) | Look changes approved or rejected |
| [AB_OLD_VS_NEW.md](AB_OLD_VS_NEW.md) | Old vs new engine: unmatched, then matched (equal on light lighting, 2× slower on heavy lighting, measured under contention) |
| [docs/spec/README.md](../../docs/spec/README.md) | Spec precedence (07 > PATCH_V1_2 > 06 > 00–05) and lead rulings, including **exactness is opt-in** |
| [docs/PHILOSOPHY.md](../../docs/PHILOSOPHY.md), [CORE.md](../../CORE.md), [WHY_LOFT_LANG.md](../../WHY_LOFT_LANG.md), [README.md](../../README.md) | Stance, priorities, why a language, the four parts |
| docs/proposals/2026-11-20_*.md | meta_engine_plan, backends_and_escape_hatches (bk1), capability_gaps (cap1), consumer_passes (+_astra) |
| design/*.md | d1, d2 (proved causes), pf1 (perf diagnosis), lean1 (size audit), tok1 (token study), std1 (stdlib audit plus tree), c1, scientist_review |
| scientist_*.md, dump_nov_19th_*.md, owner_notes_platform_power.md | Chief-scientist and owner inputs, verbatim |
| [docs/handoff/2026-11-20_critic/00_CRITIC.md](../../docs/handoff/2026-11-20_critic/00_CRITIC.md) | The lead's critique (round 1); the answers are in scientist_round1_answers.md |

## 3. What is on main (origin/main, at or after 474f0a6)

v1.0-alpha, plus:
- the runner cut;
- w1–w4 (38 native tests fixed);
- shadow-map reuse (hermit-20 at 3.75 ms);
- the Shrine Lipschitz fix and re-pinned approved looks (Pictures 19/19);
- the Three.js demo (web/threejs_demo);
- the old-vs-new A/Bs;
- the stdlib design;
- the capability-gaps design;
- the v1.5 goals.

Gates on main: Twin 36/36, Truth 34/34, 157 native tests. **Speed and Load readings were all contended; re-measure
quietly before trusting them.** The Rust is about 33k lines (target 18k, then ~15k). The `.loft` std is 607 lines.

## 4. Interrupted by the restart (logs exist, no `.done`): rerun

| Step | Goal | Disk state |
|---|---|---|
| v15a | P0 (R1): `shade_tiles` exposes (hit, px, world) plus RAY_QUERY; uncontended matched A/B | `~/git/_wt/loft-v15a`, **11 uncommitted files** (branch tip = main) |
| v15b | `kernel allow(...)` for any name plus the NonDet physics tier, continuing loft/s1-escapes | `~/git/_wt/loft-v15b`, 3 dirty, branch +4 commits, **unpushed** |
| v15c | Two-tier picture gates (R8) plus quiet-GPU timing (R9) | `~/git/_wt/loft-v15c`, **6 uncommitted files** |
| port1 | 5-crab lineup dance, crab_live, hermit look | `~/git/_wt/loft-port1-legacy` (the legacy reference checkout); no port branch yet |
| build1 | Make the cargo-deps seed real: every seed layer was empty, so every lane rebuilt wgpu and naga | branches loft/build1-measured, -seeded and -verified (+1 each, "Publish Loft's Cargo dependency seed", unpushed); worktrees `loft-build1-measured` (2 dirty) and `loft-build1-verified` |
| cam1 | Cambium: loud cold seeds, `seed warm`, harvest on remove, Rust default | `~/git/_wt/cambium-v010`: commits fb755e9 (installed) and **d5b56b9 "Warm and harvest Cargo dependency seeds"** (not yet installed or verified end to end, unpushed); a temp worktree `~/git/loft/.git/cambium/worktrees/.seed-warm-*` (branch cambium/seed-warm-*) |

## 5. Stalled: a decision is needed before rerunning

- **std1a (stdlib restructure: lib/ → stdlib/ tree) finished but did NOT merge.** The candidate is held at `loft/std1a`
  (+10 commits, pushed, 5f27dd8). Exact RGBA matched 18/19 gate frames and 920/935 corpus pairs
  (`out/meta/std1a/comparison.json`). It was held under the old bit-exact rule. **Check the diffs against the
  deterministic tolerance (≤2/255 on ≥99.9% of pixels, no structural change); if they pass, merge.** Everything in
  the std port waits on this.
- **std1b, std1c and std1d are marked done but did no work** (they waited for std1a on main). Delete
  `std1{b,c,d}.done` and `.final.md` before rerunning. std1e and the wave integrator ix4 have not started.
- **Deferred:** stdaot1 (its `.done` is only a marker) and std1f (corpus regeneration, approach R5).

## 6. Branches

**Unmerged branches with work worth finishing** (the V1_5_GOALS "Pending branches" table):

| Branch | Ahead | State and next step |
|---|---|---|
| loft/std1a | +10 | Held stdlib restructure. Merge after the tolerance check (§5) |
| loft/x2a | +7 | One execution path, partial (execution code 6,717 lines). R2 says one owner redoes this with a wgpu-free loft-std |
| loft/x2b | +7 | Motion twin 481/481 ticks, but the 1,000-crab dispatch is over budget. R7: swap the soft-float for a contraction barrier |
| loft/x2c | +1 | Sky bake; blocked on the missing Shade cube binding (−1.0 to −1.3 ms on hermit) |
| loft/s1-escapes | +1 | Whole-pass `pass`, `kernel allow`, bake sampling. v15b continues it |
| loft/s1-controls-fix, s1-controls | +4, +1 | Parity-gate controls; Hermit HDR mismatch |
| loft/pf1b | +1 | Grouped marches; re-measure under quiet timing |
| loft/lean1a | +2 | Lean lane; superseded by the R2 sequencing |
| loft/d2a, d2b, d2c, d2-probe, d2-timing | +1..3 | Earlier attempts at x2b, Tier 0 and x2a: evidence only; superseded |
| loft/c1a, c1b, c1c, c1d | +1..2 | Cook cycle 1 lanes the integrator did not merge: evidence only |
| loft/i1-prebuild-candidate, i1-scored-candidate | +4..5 | Old integration candidates (local): evidence only |
| loft/phase2 | +4 | Old Phase 2 timing lane; worktree `~/git/_wt/loft-phase2` has 2 dirty files (crates/loft-std/src/edits.rs, web.rs). Check before removing |
| loft/v15b | +4 | In flight (§4) |
| loft/build1-* | +1 | In flight (§4) |
| review | +1 | Text-only snapshot for model import; refresh it from main |

**Merged; safe to delete once the handoff is accepted:** audit/bk1-scientist, docs/backend-escape-proposal,
loft/{ab2, build1, delete-legacy-2, int8, intc1, intlean, intpf, intx2, intx3, lean1-audit, pf1, pf1a, port1-legacy,
s1-lip, std1-design, tok1-study, x2d, x2d-legacy, x2e}, and loft/v15a and loft/v15c (their work is uncommitted in
the worktrees: do not delete those worktrees).

**Archives:** about 33 `archive/*` tags on origin hold every old pre-rewrite branch. `legacy/2026-11-19` is the old
engine. `v1.0-alpha` is the first release.

## 7. Worktrees (`cambium status`)

| Worktree | Branch | State |
|---|---|---|
| ~/git/_wt/loft-v15a | loft/v15a | in flight, 11 dirty |
| ~/git/_wt/loft-v15b | loft/v15b | in flight, 3 dirty, unpushed |
| ~/git/_wt/loft-v15c | loft/v15c | in flight, 6 dirty |
| ~/git/_wt/loft-port1-legacy | loft/port1-legacy | reference checkout of the old engine for port1 |
| ~/git/_wt/loft-build1-measured, -verified | loft/build1-* | in flight (build1) |
| ~/git/_wt/loft-phase2 | loft/phase2 | old; 2 dirty: check, then remove |
| ~/git/_wt/loft-port-g2 | (none) | orphan directory; git metadata gone; branch archived. Remove once nothing holds it |
| .git/cambium/worktrees/.seed-warm-* | cambium/seed-warm-* | cam1's temp seed-warm worktree |
| ~/git/_wt/cambium-v010 | release/v0.10 (the cambium repo) | the cam1 lane; clean; commits unpushed |
| ~/git/_wt/keel-loft-rr1 | (Keel repo) | Room Runners first light: Loft rendering inside Keel's gallery; unmerged |

Never touch the primary checkout's uncommitted `tools/crab_concepts.py` and `tools/crab_faces.py`, which belong to
another session. Remove worktrees only with `cambium remove --safe NAME`; never `rm` them.

## 8. Resume, in order

1. **Merge std1a** after the tolerance check (§5). Then clear `std1{b,c,d}.{done,final.md}`.
2. Relaunch with `LOFT_MAX_STEPS=4`, each with nohup, from ~/git/loft:
   - `agent_notes/2026-11-20_codex/wave15.sh`: v15a–c resume in their existing worktrees, then iv15.
   - `agent_notes/2026-11-20_codex/wave4.sh`: std1 and std1a are skipped as done; then port1 and std1b–e; then ix4.
   - `agent_notes/2026-11-20_codex/run.sh build1 gpt-6.2-sol medium ~/git/loft`.
   - `agent_notes/2026-11-20_codex/run.sh cam1 gpt-6.2-sol high ~/git/_wt/cambium-v010`. Then the lead runs the
     cambium tests, installs the binary (keeping a backup of `~/.local/bin/cambium`) and checks that new
     workspaces start with warm dependencies.
3. **Next v1.5 wave** (V1_5_GOALS R1–R9):
   - R2: one owner does one execution path plus a wgpu-free loft-std;
   - R1: the x2c sky-bake binding, then Shrine cluster culling;
   - R7: the x2b contraction barrier;
   - R3: material families plus MaterialTable;
   - R5: corpus regeneration from seeds;
   - R6: the 3-rung wgpu bridge.
4. A supervisor: re-create the 20-minute check-in. In Claude Code it is a CronCreate prompt, which is session-only
   and died with the restart. Or run a shell loop.

## 9. Owner calls still open
- Push and release the cambium fixes (fb755e9, d5b56b9) from release/v0.10.
- Merge the Room Runners first light into Keel (a PR).
- Owner preferences:
  - don't ask, just do;
  - picture approvals are the lead's;
  - no paid or remote GPU runs without approval;
  - pnpm/uv only;
  - disk cleanup only through ~/git/cleanup_tools.
