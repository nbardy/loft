# Loft: agent guide

## How to work

1. **Theory first, to the number.** Work the problem out completely before building: the discrete version, the cost,
   the instrument that will measure it. A theory is finished when it says in advance what a run will show. Then run
   once, to score the theory. If the run misses, the theory is wrong: fix the theory, do not patch the code.

2. **Be bold.** Start from what is special about the problem, not from what has been published. Do not keep an idea
   because the best people use it. Understand why it won, replace it if you can do better, and say plainly which ideas
   are inherited and which are yours.

## Facts about this repository

- `bun` runs everything; `pnpm` installs. Nothing is compiled ahead of time.
- Typecheck with `rtk proxy pnpm exec tsc --noEmit`. A shell hook rewrites a bare `tsc` and reports errors that are
  not there.
- `bun test` is the suite. Under a loaded machine it can time out or fail once and pass on the next run; a shader
  module has also failed to build intermittently. Neither has been investigated.
- GPU time per draw: `bun src/cli/main.ts profile <scene> --view <camera>`. It also prints CPU pose time, which has
  been the largest cost in a game frame. The GPU clock ticks in 0.065 ms steps; a reading below that is unresolved.
- Comparing two ways of drawing one thing in the same timing rounds: `bun tools/tracer_bench.ts`.
- Pictures and other output go under `out/`, which git ignores.
- Design as built: `DESIGN.md`. Research notes: `docs/`. Handoff prompts: `prompts/art_research/`.
