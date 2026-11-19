# LIB — shared WGSL

The extraction predicted zero byte differences in the TypeScript outputs for this checkout's 302 entries
(132 examples and 170 seeds). The baseline also recorded that the Rust stub would refuse every built artefact
and that the corpus included one film that the TS harness could not compile as a scene.

## Translation

All 34 WGSL texts are exposed as `pub const &str` in `rust/crates/loft-lower/src/wgsl.rs` (104 lines, including
the parity test). The source files under `src/wgsl/` contain 505 lines and 24,946 bytes of shader text.

| TypeScript source | Rust constants | WGSL files / lines | TS lines added / removed |
|---|---|---:|---:|
| `src/artefact.ts` | `PRELUDE_WGSL`, `SCENE_DEPTH_WGSL`, `SCENE_CLEARANCE_WGSL`, `POST_WGSL` | 4 / 64 | 18 / 68 |
| `src/compile/scene_wgsl.ts` | `CORE_WGSL` | 1 / 30 | 2 / 30 |
| `src/draw/ink.ts` | `MARK_WGSL`, `INK_POST` | 2 / 38 | 7 / 42 |
| `src/host/lower.ts` | `DEPTH_RESOLVE_WGSL`, `CLEARANCE_WGSL` | 2 / 63 | 11 / 65 |
| `src/lib/noise.ts` | The 16 WGSL functions, named after their WGSL symbols | 16 / 97 | 35 / 97 |
| `src/soft/gauss.ts` | `GAUSS_WGSL`, also used by TS `GAUSS_LIB` | 1 / 46 | 9 / 47 |
| `src/soft/medium.ts` | `BANK_SPLIT_WGSL` via TS `BANK_LIB` | 1 / 12 | 2 / 12 |
| `src/style/light.ts` | `LOFT_TOWARD_EYE`, `LOFT_LONG_DIR`, `LOFT_SCREEN_RATE` | 3 / 12 | 7 / 13 |
| `src/style/papercraft.ts` | `PC_FACET` | 1 / 65 | 5 / 73 |
| `src/style/shared.ts` | `POST_COMMON` | 1 / 61 | 2 / 62 |
| `src/style/whitewater.ts` | `LOFT_SLOPE_ON_WATER` | 1 / 8 | 2 / 8 |
| `src/text/raw_prelude.ts` | `CAMERA_RAY` | 1 / 9 | 2 / 9 |

Bun's text import serves both the runtime and the viewer's browser bundle. The actual viewer build for
`examples/bead.ts` embedded both host and compiler WGSL. A browser-target bundle of the parity exports matched
all 35 TS values byte for byte.

Each file preserves the original leading and trailing newlines. Both consumers assemble `POST_WGSL` from the
same scene-depth source and a post suffix beginning with the original separator newline. Git checks the
shader files out with LF. TS assertions check the numbers shared with host sizes and CPU calculations. The ink
search radius and papercraft facet values now live solely in their shader files.

## Gate

- Baseline `--mode lower --run lib-base` passed **60/302**. The 241 built entries first differed at
  `files.manifest.format` because the Rust LW stub refused. The other failure was `examples__serpent_clip`.
- Final unfiltered `--mode ts --baseline out/port/lib-base --run lib` passed **301/302** and exited 1.
  `examples__serpent_clip` exports a film and crashes when the TS compiler accesses `scene.style.post`. The same
  crash occurred before the extraction and has no first failing section. Zero compared sections differed.
  A direct comparison of all 9,701 recorded TS files, including outcomes, also found zero differences.
- `tools/dev.sh lint loft-lower`, `tools/dev.sh test loft-lower` (1/1), and
  `rtk proxy pnpm exec tsc --noEmit` passed. The worktree lacked its locked `wgsl_reflect@1.6.0` dependency
  initially. The check used a worktree-local pnpm install of that version.

## Remaining

The integrator needs to resolve how the corpus handles the film entry to get a zero exit from the TS gate.
The lowering lanes can consume the shared constants as they land. Shader generators with data holes remain
with their owning lanes.
