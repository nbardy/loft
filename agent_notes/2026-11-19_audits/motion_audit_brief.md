# Audit brief: Loft motion design (read-only; write one file)

Repo: ~/git/loft. Read CORE.md (the goal and priorities), then docs/proposals/2026-11-19_motion.md (current motion
and a first proposal), then the code it names: src/geom/{gait,stance,limb,placing,solve}.ts, src/curve.ts,
src/map/{recur,table}.ts, src/expr.ts (iterate), rust/crates/loft-core/src/solve.rs, and an example or two
(examples/second/heron_leap.ts, the crab under examples/). Do not edit any file except your output.

Task: audit the motion design against the goal (natural, weighted, alive motion; the picture first; GPU frame time;
fewest tokens for the author; never false; lean), then be ambitious: propose THREE genuinely different alternative
designs and directions (not variations of one), e.g. analytic closed-form dynamics, spacetime/trajectory
optimisation with physics, GPU-simulated muscles/rigid bodies with controllers, learned motion priors, or something
better you invent. For the proposal in §3 and each alternative give:
1. The author's code first (a crab walk with a turn and secondary claws/antennae; a heron hop; a boss reacting live
   to a player hit in a game), read cold.
2. Performance: GPU cost per frame (flops/part, state, passes), build/precompute cost, memory, determinism and
   scrubbing.
3. API consumption: new names, tokens for the examples, what a model must learn, what it replaces or deletes.
4. Systems effects: where it lives (TS recorder / Rust core / GPU kernels / host), wire and bundle changes, checks
   and solve (can it still never say something false?), Keel/live-game integration, the Rust port.
5. Risks and the cheapest experiment that would decide it.
End with a ranked recommendation and a phased plan.

Write ~/git/loft/agent_notes/2026-11-19_audits/motion_audit_astra.md (3,000-6,000 words). Final message: a 200-word
summary.
