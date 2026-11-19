# State and lessons (20 Nov 2026)

## Where things stand

- **Old Loft** (tag `legacy/2026-11-19`): authors wrote TypeScript that built a hash-consed expression graph, with
  maths as WGSL text inside `wgsl` templates; a TypeScript compiler emitted artefacts; a Rust/wgpu loader or Dawn ran
  them. About 47k lines of TS in `src`, 19k of Rust. 125 examples, 170 training seeds, a creative training pipeline
  pinned to it.
- **Lean Rust port** (abandoned): a redesign and port at once; it reached 233/295 corpus scenes drawing, 13 passing
  every frame, at 30× the frame time of the old path.
- **Design slices** (on main, kept as evidence): render "kinds" drew 10, 100 and 1,000 things with the same 6 shader
  modules and pipelines (2.40 ms for 10 things, 4.60 ms for 30 at 1280×800); motion `roam` ran a live crab on Rust
  bit-equal to its bake.
- **New direction**: the meta engine in `.loft` (your draft), Phase 0 (the spine) starting now.

## Numbers that decide designs (Apple M4)

| What | Number |
|---|---|
| Five crabs, old Rust loader on TS artefacts | 3.0 / 3.5 ms p50/p95, 305 modules, 422 render + 24 compute pipelines, 32.9 s cold / 3.6 s warm load |
| Same, TS engine on Dawn | 4.3 ms |
| Same, redesigned Rust path | 97 / 109 ms, 5,539 modules, 4,905 compute pipelines, 23 s load |
| Pose stages | level-batched 24 dispatches 0.83 ms vs per-kernel 4,884 dispatches 13.7 ms |
| Draws | shader sharing 2.2 ms vs one pipeline per draw 17.3 ms |
| Frame-time model across 229 scenes | ≈ 1.3 ms + 15 µs per GPU command |
| Titan recording | 707k nodes (unrolled loft, constants counted twice, every call inlined) → 308k after fixes |
| Tape interpreter in one WGSL shader | +2.0 ms (63 ops), +5.5 ms (larger) per frame |
| Analytic branch list vs bricked march (3,069-branch coral, 1280×800) | 3.08 vs 25.89 ms close, 0.72 vs 1.90 ms far; compile 7.2 s → 31 ms; 1.5–9.5% of pixels changed |
| 100,000-grain veil | 120.8 ms marching per grain pixel vs 1.56 ms grain draw with a 32³ grid |
| Motion, 1 / 256 / 1,000 crabs fused | 0.070 / 0.101 / 0.086 ms p95; bake 5.3 ms; 216 tokens of author code |
| Rust emit with naga validation | most of emit time; validation done twice (ours and wgpu's) |

## What went well

- **Writing maths as WGSL text** inside the graph: models write it fluently, and the CPU twin, checks and solve still
  read it. Raw functions as the escape hatch, named opaque.
- **Kinds as the unit of code identity** (design slice): module count independent of instance count, proven at 1,000
  things.
- **Level batching and shader sharing** in the old compiler: they are why the old path was fast.
- **Honest checks**: interval proofs, `Unproved`/`Undecided` verdicts, refusals that name the fix; a strict parity
  runner once it was made strict.
- **Motion results**: solve near hand keys plus world-space gaze lock and neck IK (the heron the owner liked);
  weight-driven stepping from capture point (the crab that does not moonwalk); live equals bake to the bit.
- **Escape ladder**: graph function → raw WGSL function → `draw.shader` → compute pass at build/load/frame/steps.
- **PBR as swappable parts**: `pbr(parts)` takes any of distribution, visibility, fresnel, diffuse, environment,
  occlusion, sky, tone, lobe albedo; a swapped lobe re-integrates its environment albedo.
- **Keel-style devex**: compile locks, per-worktree targets, lane scripts, one full suite per wave.

## Where we turned poorly

1. **Redesign and port at once.** New IR, new wire, new unified types and a new runtime together; nothing ran end to
   end until the last lane; the speed tricks of the old compiler were lost.
2. **Lanes cut by layer**, each passing a unit check, integrating late: two incompatible wire formats; zero draws
   recorded until wave 5.
3. **Host tracing without staging**: JS loops unrolled, calls inlined, instance numbers baked into code: the root of
   the node, module and cold-compile blowups.
4. **Optimised CPU build time** (recorder, solve) before measuring GPU frame time, which is what matters.
5. **Measured frame time last.** The 97 ms result arrived after a day of work.
6. **A weak gate**: picture parity exited 0 with missing frames.
7. **Frozen layers inside the "open" design**: the material model reached the renderer as one paint struct read by
   one style; light transport welded to the style's shade function; volumes composited in author order (red over blue
   gives a different answer from blue over red, both wrong); ambient occlusion that depends on the drawing method.
8. **Shadows were stand-ins**: casts from capsule skeletons and fitted grids, sometimes sampled at time zero; a scene
   query (`castRay`) that missed things without skeletons.
9. **No asset import**, sea/sun/style mandatory in the core, about 800 public names.
10. **Solve without posture holds**: correct maths, wrong performance (the 67° heron).

## Constraints of the machine and the team

- One Apple M4 Mac, 24 GB, shared with a game studio's builds (Keel); two restarts on 19 Nov.
- Implementers: Codex gpt-6.2-sol lanes for bulk, Opus for design and integration, Astra for audits.
- Targets: native wgpu (Metal, Vulkan, DX12) and browser WebGPU; embeddable on a device a game owns (Keel,
  room-runners).
- Training uses Loft scenes written by models, scored on looks; build budget 2 s CPU, draw budget 50 ms GPU per frame
  at 512² × 16 frames.
