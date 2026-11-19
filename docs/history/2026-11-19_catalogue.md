# Catalogue at the stop (19 Nov 2026)

On 19 Nov the owner stopped all work and asked for a first-principles re-plan of Loft as a meta engine
(`docs/PHILOSOPHY.md`, `docs/proposals/2026-11-20_meta_engine_plan.md`). The old TypeScript authoring API, the TS
compiler and host, the lean Rust port, and the earlier DSL syntax are left behind. This page records what exists, so
nothing is lost and nothing is redone.

Main at the stop: `acb7e8b`. Tag it before the new tree lands: `legacy/2026-11-19`.

## Done and on main

| Work | Where | What it gives the new plan |
|---|---|---|
| History, inventory, size audit, room-runners devex study | `docs/history/2026-11-19_{inventory,size_audit,room_runners_devex}.md` | Why the compiler was TS from day 1; the Keel devex (locks, per-worktree targets, lane.sh) we adopted |
| Philosophy, CORE, design goals | `docs/PHILOSOPHY.md`, `CORE.md`, `docs/design/{render,motion}.md` | The stance: meta engine, hard code cheap and lock-in expensive, GPU frame time first |
| Third-party code reviews (Astra, Fable), code-cut audits, motion audit | `agent_notes/2026-11-19_audits/` | What an outsider would and would not adopt; where the lines were |
| First-principles critiques and designs (render, motion; Fable, Astra, philosophy pass) | `agent_notes/2026-11-19_design/` | Kinds, six queries, schedules; body/state/pose, laws, pins, fused ticks |
| Consolidated architectures | `docs/proposals/2026-11-19_{render,motion}_architecture.md` | The kinds-and-rows principle, verified on a slice |
| Render and motion Phase 1 slices, integrated | main `ab80222` (feat/design-render, feat/design-motion) | 10, 100 and 1,000 things in 6 modules; live crab bit-equal to its bake |
| Lean Rust port waves 0 to 4 and lanes (wire 1 to 5, kernels, emit, check, solve in Rust, loft-gpu, loft-draw, recorded draws) | main; notes `agent_notes/2026-11-19_lean_rust_*.md` | Parts worth mining: naga emit, interval checks, solve in Rust, packed tables, GPU cameras, the parity tool |
| Keel-style devex | `tools/dev.sh`, `tools/check.sh`, `tools/lane.sh`, `tools/lib/build_gate.sh` | Keep as is |
| Parity tool (strict: every outcome and frame scored) | `tools/parity` | Reusable gate machinery |
| 1:1 port plan, byte-diff harness, lane briefs | `agent_notes/2026-11-19_port/`, `tools/port` | Abandoned as a direction; the compiler map is a useful index of the old TS lowering |
| Creative training migrated to Loft c8502c2 (170 seeds, reconstruction) | creative main `09d27924` | Stays pinned to the old Loft until the new corpus is regenerated |

## Measured lessons (the part that must survive)

- **Speed is the shape of the compiled output.** Five crabs: old Rust loader on TS artefacts 3.0/3.5 ms (305
  modules); TS on Dawn 4.3 ms; redesigned Rust path 97/109 ms (5,539 modules, 4,884 per-kernel dispatches). Frame time
  across 229 scenes ≈ 1.3 ms + 15 µs per GPU command. Level batching and per-kind sharing are what made the old path
  fast.
- **Load time is the driver's pipeline compile**: five crabs 32.9 s cold, 3.6 s warm. Module count must be O(kinds).
- **Host tracing without staging is the root cause** of the node blowups (Titan 707k nodes), the module explosion and
  the cold compiles.
- **A tape interpreter costs +2.0 ms (63 ops) to +5.5 ms per frame** (branch `loft/interp-probe`): fine for the one kind
  being edited, never for a whole frame.
- **Redesigning and porting at once fails**: nothing ran end to end until the last lane. Build vertical slices end to
  end from day one, with gates live from day one.
- **Gates must be strict**: the parity runner once passed with missing frames.
- **Motion**: solve near hand keys plus world-space gaze lock and neck IK reads best for the heron; weight-driven
  stepping (capture point, steps from wherever the foot is) beats a gait clock for the crab; motion for 1,000 crabs
  costs 0.07 to 0.10 ms p95 when fused.

## Not done, and now abandoned (branches kept, nothing deleted)

| Work | Branch / worktree | State at the stop |
|---|---|---|
| 1:1 port wave 0 (P0, W, G1, G2, LW, SW) | `port/*`, `~/git/_wt/loft-port-*` | 1 to 2 commits each, stopped mid-lane; LIB finished (34 shared WGSL sources, 9,701 outputs byte-identical) |
| Old Rust loader as default runtime | `feat/loader-default`, `~/git/_wt/loft-loader-default` | 1 commit plus 189 uncommitted files; parity run in progress at the stop |
| Loader measurement | `scratch/loader-measure` | Done; note on the branch (the 3.0 ms number) |
| Motion look rounds | `feat/motion` (pushed) | Done; comparison page published |
| feat/rust-core (sol's earlier plan) | `feat/rust-core` | Superseded twice |
| Code cuts from the audits | none | Not applied; moot under the new tree |
| Corpus switch to Rust, compact graph, PBR audit | none | Not started; replaced by the new plan |
| Creative re-pin | none | Waits for the new corpus |

## Disposal

Worktrees under `~/git/_wt/loft-*` stay until the owner says otherwise; remove them only with `cambium remove` from
`~/git/cleanup_tools` rules. Branches stay; the tag `legacy/2026-11-19` marks the last state of the old tree.
