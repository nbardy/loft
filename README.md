# Loft

Loft describes animated 3D things as TypeScript code. A compiler turns the description into WGSL plus tables of numbers (an
`Artefact`, `src/artefact.ts`) that any WebGPU host can draw. Bodies are coordinate systems, so paint, fins and foam are
functions of body coordinates; time is an argument, not a system. Design and rules: `DESIGN.md`.

## Install and run

```
pnpm install            # bun runs everything, pnpm installs
bun test && pnpm exec tsc --noEmit
```

Every command takes a scene module (default export is a scene) or `--fixture`, a hand-written artefact that needs no compiler.

```
pnpm loft render examples/serpent.ts --out out/s.webp [--views front,side,top,three] [--time 1.0] [--size 1280x800] [--param name=value ...] [--construction]
pnpm loft frames examples/serpent.ts --out out/m.webp --times 0,0.5,1,1.5 [--view three]    # one view, several times
pnpm loft clip   examples/serpent.ts --out out/s.mp4 [--seconds 8] [--fps 30] [--view three] # raw RGBA piped to ffmpeg
pnpm loft dev    examples/serpent.ts [--port 5173]                                           # live viewer
```

`render` writes one contact sheet (row-major, views in the order given); stills are WebP q90 (needs `cwebp`), clips are
H.264 (needs `ffmpeg`). Timings print on every run. `dev` serves a WebGPU page with orbit (drag, wheel), the scene's named
cameras, a time bar, one slider per param (no recompile), a construction toggle and a readout; it rebuilds on edits to
`src/`, `examples/`, `viewer/` or the scene's directory, and keeps camera, time and sliders across reloads.

## Where things live

- `src/expr.ts`, `src/artefact.ts`: expressions and the compiler/host contract
- `src/curve.ts`, `src/geom/`, `src/scene.ts`, `src/style.ts`, `src/compile/`: authoring API and compiler
- `src/host/`: WebGPU host (browser canvas and bun headless)
- `src/cli/`: the commands; `viewer/`: the live page; `tests/`: tests; `out/`: generated output (gitignored)
