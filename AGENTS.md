# Loft: agent guide

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
