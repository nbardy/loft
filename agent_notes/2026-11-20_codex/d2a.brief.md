You are Loft’s Sol engineer for deterministic authored motion. Repo ~/git/loft. Your starting engine is loft/v2 at 6776d49. You have no other context.

Read CORE.md, docs/PHILOSOPHY.md, docs/spec/README.md and 06 §§8–10. Precedence is 07 > PATCH_V1_2 > 06 > 00–05. Read the motion page of agent_notes/2026-11-20_v1/design/d2.md on origin/main and the v2 lane note on loft/v2.

Rules

- Create your workspace with `git cambium add -b loft/d2a -name loft-d2a -ephemeral ~/git/_wt/loft-d2a loft/v2`. Read the design with `git show origin/main:agent_notes/2026-11-20_v1/design/d2.md`.
- Own crates/loft-passes/src/{motion.rs,motion_source.rs,fusion.rs,solver.rs,plan.rs,plan_source.rs,eval.rs,check.rs,emit.rs}, deterministic helper files included from those modules, crates/loft-std/src/compute_twin.rs, engines/{act,roam}.loft, lib/motion.loft, scenes/{crab,heron,boss}.loft, and crates/loft-cli/tests/d2_motion.rs. Your note is agent_notes/2026-11-20_v1/lanes/d2a.md.
- Record predicted words, footprints, pictures and time first. Write all code and probes, then compile once with `CARGO_BUILD_JOBS=2 tools/dev.sh build loft-cli --fast --workspace --all-targets`. Run the resulting focused executables under tools/gpu_lock.sh, one GPU probe at a time. Integration owns the combined suite.
- Classify at the boundary with exhaustive variants and source-spanned errors. Never edit or re-approve references. Push loft/d2a with your measured lane note. Keep main for integration. End the commit message with `Co-Authored-By: GPT (codex) <noreply@openai.com>`.

Task

The 0/481 replay result compares GPU groupings. The ordinary CPU/Metal law rows differ on 371, 177 and 447 ticks. Three one-site controls identify Crab tick 2 at the x integration add (3c162fca/cb), Heron tick 19 at the inner crouch Horner add (hip 3f51b2fb/fa), Boss tick 8 at the outer windup Horner add (3c593be3/e1). Explicit FMA makes both twins take the Metal word. Enforcing a rounded product makes both take the CPU word. Tick-zero verdict margins differ in the reachable-end IK calculation even before a law runs.

Lower transitively reachable solver, law, check, event and prior arithmetic to one specified sequence of binary32 operations. Give ordinary multiply/add two roundings and explicit FMA one. Fix vector operation and gather order, polynomial coefficients/range reduction, exceptional values, casts and cross-workgroup phase visibility. Emit strict operations guaranteed by the target or common u32 binary32 helpers. Reuse the CPU interpreter over that same entry. Verify final IR and raw `wgsl det` bodies against a closed operation/effect set. Return E504 with a span for an unverified construct. Cover new raw helpers, float atomics, f16, unordered races and unbounded loops. Bind the solved 97-row Heron LUT on both execution and source-evaluation paths. A changed non-knot prior sample must reach the hip. Preserve exact stored knots and failed-plan status.

Run the actual authored solvers and their BODY/STATE/POSE and verdict/outbox buffers. Record entry names, layouts, counts and first differing words at every tick 0–480. Require 0/481 for each table and for 30/60/120 Hz, six-tick stall, seek and replay. Check true solver participation by perturbing a constraint and a prior separately. Require Crab reversal ≤0.10 s, toe drift ≤0.05 mm, the 20 mm mutant caught and 1,000 crabs ≤0.15 ms p95. Heron must crouch to 0.59 m, pitch ≤12°, neck error ≤0.2 mm and zero rear flips. Boss must reach 0.69 m and slip on hits 60/75, with exact event words and zero drops. Compare each 0.5 s frame to retained main byte for byte. Report prediction versus measurement and the status of the numeric and picture gates separately.

Final message ≤120 words. If the single build or a gate fails, preserve the smallest reproducer, both words, source op and branch head. Finish with ESCALATE
