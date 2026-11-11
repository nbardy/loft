# Loft: agent guide

- `bun` runs everything; `pnpm` installs. Nothing is compiled ahead of time.
- Typecheck with `rtk proxy pnpm exec tsc --noEmit`. A shell hook rewrites a bare `tsc` and reports errors that are
  not there.
- `bun test` is the suite. Under a loaded machine it can time out or fail once and pass on the next run; a shader
  module has also failed to build intermittently. Neither has been investigated.
- GPU time per draw: `bun src/cli/main.ts profile <scene> --view <camera>`. It also prints the pose stages (the
  per-frame tables, computed on the GPU since phase 1 of `docs/gpu_native.md`), the CPU pose time, and every table
  still filled on the CPU (`cpu`) with its reason. Pose stages alone: `bun tools/pose_time.ts [scene.ts:view ...]`. The GPU clock ticks in 0.065 ms steps; a reading below that is unresolved.
- Comparing two ways of drawing one thing in the same timing rounds: `bun tools/tracer_bench.ts`.
- Pictures and other output go under `out/`, which git ignores.
- Design as built: `DESIGN.md`. Research notes: `docs/`. Handoff prompts: `prompts/art_research/`.
- A scene as files: `bun src/cli/main.ts build <scene> --out out/files/<name>`, drawn without the compiler by
  `render --artefact <dir>`. Format: `docs/artefact_format.md`. `bun tools/check_loader_closure.ts` fails if the file
  loader's imports reach the compiler; `bun tools/files_proof.ts` compares files, scene and phase-1 pixels.
