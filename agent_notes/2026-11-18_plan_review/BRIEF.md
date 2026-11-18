# Review brief: Loft's next plan and design (18 Nov 2026)

You are one of four independent reviewers (two Claude, two Codex). You do not see the others' reviews. Review the
plan and design, and the code already landed for it, and say what is wrong, what is missing and what you would change.

## Read
- `~/git/loft/docs/proposals/2026-11-18_next.md`: the design and plan under review (API, systems, plan steps 1–9).
- `~/git/loft/DESIGN.md` and `docs/` as needed for what Loft is today.
- What has landed on Loft main at `52c8d98` for steps 1, 3 and 4 and the serve step:
  - ranged checks: `src/interval.ts`, `src/ranged.ts`, `tests/ranged.test.ts`, `agent_notes/2026-11-18_ranged_checks.md`
  - PBR as parts: `src/style/pbr.ts`, `src/style/pbr_parts.ts`, `agent_notes/2026-11-18_pbr_style/NOTE.md`
  - solve: `src/geom/solve.ts`, `examples/second/heron_leap.ts`, `agent_notes/2026-11-18_solve/`
  - `loft serve`: `src/cli/serve.ts`, `agent_notes/2026-11-18_loft_serve.md`
  - interpreter probe (branch `loft/interp-probe`, not merged): `git -C ~/git/loft show loft/interp-probe:agent_notes/2026-11-18_interp_probe.md`
- Background, optional: `~/git/_designs/2026-11-18_loft_restart/` (`IDEAS_SO_FAR.md`, `judge/VERDICT.md`,
  `branch/*/DIRECTIONS.md`) and `~/git/creative/agent_notes/2026-11-16-render-fast/RESOURCES.md` (measured costs).

## The owner's binding positions (judge the plan against these)
1. Motion is programmable: solver, linear, polynomial, any equation. The solver must not be the only way.
2. The model should write as much code as it likes: all maths and algorithms, close to an arbitrary compute graph.
   Libraries save tokens but must not trap it in a framework. Raymarched space folding and shader tricks must still work.
3. Users roll their own styles; PBR is shared, swappable parts; everything is extensible.
4. Much faster training (per-scene shader compile is ~80% of a sample's CPU today), reliable, deterministic.
5. Host language and new names matter little to the owner, but every new name must buy a capability.

## Deliver: one Markdown file (the path is in your instructions), 1,500–3,500 words
1. **Verdict** in five lines: is this the right plan? fund, change, or stop?
2. **Defects**, ranked most serious first. Each: what is wrong, where (file:line or plan section), the concrete
   consequence, and the fix. Separate design defects from code defects in what landed. Verify claims against the code;
   mark anything you could not verify.
3. **Against the five positions**: for each, does the plan honour it? Where does it box the model in?
4. **Missing**: capabilities, risks or experiments the plan should have and does not.
5. **Order of work**: would you change steps 5–9? Name the next three things to build, each with a pass/fail check.
6. **Numbers**: recompute any cost or speed claim you rely on; say measured vs derived.

Be specific and blunt. Do not edit any repository file except your own review file. Do not commit.
