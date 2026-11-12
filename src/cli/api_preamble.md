<!--
Written orientation printed at the top of `loft api` (and so of docs/api.md and `loft context`): the layers, the
renderer, what a user can bring, the limits. Everything after it is generated (src/cli/api.ts). Snippets fenced
`ts run`, `ts check` and `sh` are run by `bun tools/check_snippets.ts`.
-->
Loft describes animated 3D things as TypeScript expressions and compiles them to WGSL plus tables of numbers that any
WebGPU host draws. Files in `examples/` import `../src/index.ts`; `src/lib/index.ts` adds plain functions (distance
functions, folds, noise, raw WGSL). `loft` is `bun src/cli/main.ts`.

**The layers**, bottom up. Each is written in the one below, so leaving a component is a step down, not a rewrite.

1. **Expressions and curves**: arithmetic on `Expr` values, `time`, `input` (a value the host sets each frame),
   `param` (a number tunable without recompiling), curves (`keys`, `through`, `wave`, `response`, `retarded`).
   Every expression runs on the GPU and, for checks, on the CPU.
2. **Maps**: a shape is a function. `surface((u, v) => point, topology)`, `sweep`, `lathe`, `extrude`, `sheet`,
   `loft`; placing one thing on another (`place`, `on`, `points`, `posed`); `tabulate` for what has no closed form.
3. **Drawables**: how a map reaches the screen (`draw.*`, below).
4. **Shapes**: `strand` (a swept body: any spine, girth, section, twist, tips, paint), `spiralTube` and `shell`,
   `limb`, `fan`, `crest`, `hull`, `ship`, symmetry (`mirror`, `array`).
5. **Motion**: the twelve principles as formulas (`beats`, `anticipated`, `overshot`, `swing`, `dragged`, `alive`),
   `struck`, expressions as named poses (`cast`, `score`), feet and carriage (`gait`, `footing`, `carriage`),
   hanging and nervous parts (`pendulum`, `dart`), grabs and throws.
6. **Actions, checks and designs**: `attack` from a hit shape, arming time and warning; `telegraph`, `smear`, `wake`;
   `routine` for a group on counts; checks run on the CPU (`loft check`); `design` for a subject with slots and named
   variants (`loft variants`).
7. **Styles and light**: a `Style` is a value of functions (shade, ink, water, shadow, sky) plus post passes. `toon`,
   `woodblock`, `biolume`, `papercraft`; `withStyle`, `withLight`; `movable` for a sun the host can move.
8. **Scene and compiler**: `scene({ sea, things, style, sun, cameras, period })`, `compile`.

**What draws a thing.** A rasteriser with per-pixel tracers inside bounds; not a path tracer.
- Rasterised surfaces: any (u, v) map on a grid, posed in the vertex stage, painted per pixel, outlined in ink
  (`draw.surface`, and everything swept).
- Tracers, each drawn as its bounding box and solved per pixel: sphere-traced distance fields (`draw.field`),
  shell-traced relief on a surface (`draw.relief`), analytic impostors, one solid per lattice cell (`impostors`),
  and listed solids (`draw.copies`).
- Media integrated in closed form: clouds and mist (`draw.density`), glows, lightning, rain.
- Derived layers: sky, cast shadows on the water, contact foam, the style's post passes.

One sun; shading is the style's. There are no reflections or refraction, no indirect light, and no shadow maps:
shadows are cast from capsule chains along each body's skeleton (onto the water always, onto bodies only in a style
that prints a shadow block), and a traced field casts a blob. What resembles reflection or occlusion (biolume's
reflection, papercraft's creases) is a post pass reading the picture and its depth.

**What you can bring.**
- A `Drawable`: `name`, `draws(ctx)` returning WGSL shaders and draws (each with a phase: `background`, `opaque`,
  `on-bodies`, `water`, `on-water`, `transparent`), `skeletons`, `surfaces`. `ctx.table` declares a per-frame table
  your WGSL reads; `ctx.post` adds a full-screen pass. The compiler changes for no new kind. `src/draw/sky.ts` is a
  30-line one; `docs/guide.md` has a runnable one.
- A WGSL function called inside expressions: `wgslFn` (`src/lib/wgsl.ts`). GPU only: the CPU, and so the checks,
  cannot evaluate it.
- A style: a plain `Style` value, written fresh or spread from another with a function replaced or a post pass added.
- A host: `loft build <scene> --out <dir>` writes `manifest.json`, `wgsl/`, `bin/` (`docs/artefact_format.md`), which a
  loader executes without deciding anything: `createHost` in `src/host/index.ts`, or `rust/loft-loader` on `wgpu`.

**What it cannot do today.**
- No ground but the sea: every scene has `sea({ swell })` (an empty list is flat water).
- No path tracing (above). One sun.
- No simulation: everything is a closed form of time and inputs; nothing carries state from frame to frame.
- No meshes in: shapes are functions (`loft bake` writes glTF out).
- Headless rendering needs bun; the package is not published; a large subject is megabytes of WGSL and seconds to load.
- Several components take only numbers, so their family is fixed (`shell`, `ship`, `whirlpool`, `cloud`, `pendulum`,
  `sea`): leave them by writing the map yourself one level down.
- Stance, routine, expression casts, struck poses, emotes, placing, the ring wave and the lineup each have one user so
  far; expect to cut one down when your subject fights it.

**Reading the entries below.** Each value has the first sentence of its comment; `cannot:` and `relatives:` lists
where its comment states them. A component with no `cannot:` list has not said where its family ends: assume it ends
sooner than you think, and read its whole comment with `loft api <name>`.
