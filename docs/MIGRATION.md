# Migrate Creative's frozen Loft scenes

The two Creative seed packs import `baseline38-retry1-preflight-20261113/frozen-loft`. All 157 paths and Git blob hashes under its `src/` match Loft commit `a3c76126f1716df4551753b2b141a3b59eaa098a` (source tree `c655c301e1869a1e104557337c130b9a397ea936`). Creative's `Frozen` variant in `training/loft/library.py` selects `training/loft/library`, a staged derivative of that snapshot. Its `FROZEN_DIFF.md` records the shared-shader and parallel-pipeline changes. The seed files and the `crab-actions-v1` manifest identify the archived copy directly.

This guide compares `bun <frozen>/src/cli/main.ts api` (773 listed exports) with `bun src/cli/main.ts api` at `e7c3c5a` (819). The [current API map](api.md) gives the signatures. Run `api <name>` on each checkout for the full entries, or compare the complete `draw` entries for its members. The November 18 [proposal](proposals/2026-11-18_authoring_api.md) describes interim APIs as well as planned work. The entries marked *interim* below apply to code written after the freeze and before the authoring changes landed. The API map and implementation are the reference for the shipped surface.

## Unchanged

Everything in the public scene-authoring API that is not listed below works as before.

- The TypeScript expression call form, `time`, `input`, `param`, vector constructors, curves and keyframes.
- The map algebra and shape constructors, including `surface`, `sweep`, `lathe`, `strand`, `chain`, `fan`, `spiralTube`, `coil`, `sheetOn` and `ropes`.
- Gaits, poses, attacks, telegraphs, grabs, routines and `design` slots, variants and comparison cameras.
- The `scene` constructor, `ground`, named cameras, `period` and default scene export. Export `checks` beside a scene or return checks from a design build.
- `draw.surface`, `draw.glazed`, `draw.curve`, `draw.field`, `draw.relief`, `draw.copies` and `draw.posed`. Existing paint functions can keep returning a plain colour.
- The six original styles and the build-to-files workflow. Rebuild directories from source to pick up compiler changes. [Guide](guide.md) · [Artefact format](artefact_format.md)

## Renamed

- Creative's absolute `…/frozen-loft/src/index.ts` or `…/training/loft/library/src/index.ts` → `loft`. Archived layer imports such as `…/src/core.ts`, and the layer subpaths `loft/core` through `loft/styles`, → `loft`: the package has one entry, and every layer name is its name. Public deep imports can use `loft` when it exports the same declaration. The SDF `round` and `extrude`, fold `compose`, and overlapping similarity types still have their own meanings under `src/lib/`.
- `src/sun.ts` public imports `movable` and `Sun` → `loft`, now declared in [`src/light.ts`](../src/light.ts).
- `draw.points(name, points, thing)` → `draw.posed(name, points, thing)`. Keep the `points(count, spec)` constructor. [Copies and specks](guide.md#many-small-specks-on-your-own-paths)
- `Points` → `Poses`. Code that constructs poses by hand must return `{ frame, size: size.even(scale) }` from `at(i)` in place of `{ frame, scale }`. Code that reads `at(i)` must account for both size variants. [Pose declarations](../src/map/types.ts)
- `Instanceable` and `thing.instanced(name, points)` → `Poseable` and `thing.posed(name, poses)`. Review custom implementations against the new pose result. [Drawable declarations](../src/draw/index.ts)
- *Interim* `draw.specks` → `draw.points`. [Specks](guide.md#many-small-specks-on-your-own-paths)
- *Interim* `SpecksOptions` → `PointsOptions`. [Speck options](api.md#srcsoftspecksts)

## Removed

- `wgslFn({ name, ret, wgsl, deps })` → `wgsl(text)[name]` for a standalone raw function. Supply dependencies as functions in the module or use supported template holes. `WgslFnSpec` has no corresponding options object. The codemod only translates independent literal functions that Loft classifies as raw with the same return type. [WGSL text](guide.md#maths-as-wgsl-text-wgsl)
- `fwidth(x)` → a raw `wgsl` function returning WGSL `fwidth(x)`. It still runs in the fragment stage under uniform control flow. [Screen functions](../src/lib/screen.ts)
- `viewDir(p)` → a raw function returning `view_dir(p)`. [Screen functions](../src/lib/screen.ts)
- `pixelWorld(p)` → a raw function returning `pixel_world(p)`. [Screen functions](../src/lib/screen.ts)
- `eyePosition()` → a raw function returning `frame.eye.xyz`. [Screen functions](../src/lib/screen.ts)
- `staysClear(body, target, { from, until, margin, label })` → `always([time, from, until], ge(clearance(body, target), margin), { label })`. Defaults are `from = 0` and `margin = 0`. Resolve arming objects and deliberate sample rates by hand. The current range includes both ends. [Clearance](api.md#srccheckts)
- `stopsAt(strike, target, opts)` → two checks using `gap = clearance(strike.part, target)`. Require `le(gap, contact)` at `[time, T, T]` and `ge(gap, -tolerance)` over `[time, T - before, T + after]`, where `T` is the first arming and the span comes from the attack. Defaults are `contact = 0.6` and `tolerance = 0.05`. Choose how to expose the two verdicts where the caller expected one. [Checks](guide.md#an-action-a-game-can-read-with-its-checks)
- `flash({ at, power, colour, reach })` and `Flash` → `lamp(at, { colour: mul(colour, power), reach })` and `Light`. Move `medium(..., { flashes })` entries to `scene({ lights })`. This widens where the light is evaluated, so review the composition. [Lighting](lighting.md) · [Medium](api.md#srcsoftmediumts)
- `Drawable.surfaces` and `Bakeable` → `Drawable.forms` and tagged `Form` variants. An empty `surfaces: []` becomes `forms: []`. For actual geometry, describe a `surface`, `mesh`, `field`, `medium` or `hit` and update consumers that assumed bakeable surfaces. [Form](api.md#srccompilecontextts)
- *Interim* `atLeast(a, b, { from, to })` → `always([time, from, to], ge(a, b))`. An `{ at }` option uses `[time, at, at]`, and a hold with no timing uses `ge(a, b)` across the solve span. `When` and the old hold record are replaced by the boolean condition and its optional quantifier. [Solve](guide.md#motion-from-constraints-solve)

## Changed meaning

- *Interim* `equal(a, b, { at, within })` → `always([time, at, at], equal(a, b, within))`. An interval uses `{ from, to }` in the old code and `[time, from, to]` in the new code. For `{ within }` alone, pass just the numeric third argument. `equal` now returns a boolean expression and retains its role as a target in `solve`. A plain comparison in `hold` applies across the solve span. The solver verifies the f32 curves it returns and exposes the authored conditions as `solved.holds`. [Solve](guide.md#motion-from-constraints-solve)
- Sampled checks report a `sampled` verdict and describe the observations. `ranged` reports proof bounds and a witness when available, with `undecided` for unsupported operations or exhausted search. `raw-compute` reports `undecided` for a check that reads a compute output without a CPU twin. Dispatch on `kind` or use `checkOutcome` from `src/check.ts`. [Verdict](api.md#srccheckts) · [Ranged refusals](errors.md#ranged-checks)
- `clearance` from a disc, column or body is a graph measure over the skeleton. Clearance from a `skin` uses a CPU mesh search and a sampled verdict under a single time quantifier. Review any thresholds that depended on the old sampled gap or its exclusive upper endpoint. [Clearance](api.md#srccheckts)
- *Interim* `draw.volume` density, albedo and emission take `(p)`, and the `draw.points` speck slot takes `(i)`. Read the imported `time` expression inside each. Loft substitutes the sample's time, including for motion stretch. [Volumes and specks](guide.md#what-draws-a-thing-and-what-you-can-bring)
- `points()` and `warp.points()` now return poses. In code that reads copies directly, use `at(i).size` and narrow its `kind` before using `k`. [Copies and specks](guide.md#many-small-specks-on-your-own-paths)
- `cellular2` returns a vec4. `xy` are the old distances and `zw` is the nearest feature point. Project to `.xy` wherever a frozen caller used the whole vec2. The tool can infer this for archived imports. [Noise API](api.md#srclibnoisets)
- `boltFlash(...)` and `sheetFlash(...).flash` return `Light`. Register them in `scene({ lights })`. `draw.density` uses the scene's lights and supplies defaults for all options except `bounds`. [Lightning](api.md#srcsoftlightningts) · [Density](api.md#srcdrawindexts)
- `add`, `mul`, `min` and `max` now consume extra operands. Other binary helpers refuse a third operand. Review the expression if an old call relied on the extra argument being ignored. [Expression API](api.md#srcexprts)
- Unknown surface topology names, malformed bounds, invalid station counts and several other malformed inputs now give typed scene refusals. A numeric style outline of zero skips the ink pass. Still seas without swell omit swell lines. [Refusals](errors.md) · [Styles](styles.md)
- `Paint` can return `Painted` as well as RGB. Update custom consumers of a general paint function to read `base` from a record. Custom shading contexts gained light and sky data, and a hand-constructed `Scene` now carries `lighting`. [Paint and materials](styles.md#what-a-style-is) · [Scene API](api.md#srcscenets)
- WGSL text uses WGSL rounding and remainder rules. Existing expression calls keep their `round` and `mod` meanings. [WGSL text](guide.md#maths-as-wgsl-text-wgsl)
- Serve replies expose `identity` and per-frame `id`. Read the selected tier from `identity.build.tier`, requested precision from `identity.build.precision`, and drawn precision from `identity.frame.precision`. `render` and `frames` print a frame ID alongside the pixel hash. Author WGSL rejected by the GPU is a scene error at its source line. [Identity](handoff_training.md#identity) · [Exit codes](errors.md)
- Creative's frozen host wrapped `export default drawables` and `export const renderStyle`. To run such a module directly with Loft, export `scene({ things: drawables, ... })`. The codemod copies the host's subject-render defaults for sea, sun and the `fixed` camera. `smooth` becomes `toon({ bands: 32, outline: 0 })`. The preset export can stay alongside the scene. [First scene](guide.md#install-and-the-first-scene)

## New

- `wgsl` and `WgslModule` accept text functions in ordinary expression slots. [WGSL text](guide.md#maths-as-wgsl-text-wgsl)
- `gradient` differentiates a scalar expression with respect to a vector. [WGSL text](guide.md#maths-as-wgsl-text-wgsl)
- `always` and `eventually` quantify comparisons over time, inputs, params or variables. See `Range`, `Ranged`, `RangedSettings` and `Decision`. [Checks](guide.md#an-action-a-game-can-read-with-its-checks)
- `clearance` exposes a gap as a measure. [Checks](guide.md#an-action-a-game-can-read-with-its-checks)
- `litBy` checks light reaching a stretch of a body. [Lighting guide](lighting.md)
- `solve` fits motion to conditions, with typed failures and a report. See `SolveSpec`, `Solved`, `SolveError`, `SolveFailure`, `SolveReport`, `HoldLine`, `HoldReport` and `Prefer`. [Solve](guide.md#motion-from-constraints-solve)
- `spline` declares an unknown curve (`Family`) for `solve`. [Solve](guide.md#motion-from-constraints-solve)
- `bspline` constructs a curve from known controls. [Solve](guide.md#motion-from-constraints-solve)
- `draw.points` draws camera-facing grains, dust or sparks. See `Speck` and `PointsOptions`. [Specks](guide.md#many-small-specks-on-your-own-paths)
- `draw.volume` marches an extinction field and integrates light and emission. See `VolumeOptions` and `VolumeCtx`. It can also veil specks. [Volumes](guide.md#what-draws-a-thing-and-what-you-can-bring)
- `draw.mesh` draws author-supplied triangles. See `MeshData`, `MeshDrawable` and `MeshOptions`. [Drawables](guide.md#what-draws-a-thing-and-what-you-can-bring)
- `draw.traced` intersects user-defined solids. See `Ray`, `Traced`, `TracedDrawable` and `TracedOptions`. [Drawables](guide.md#what-draws-a-thing-and-what-you-can-bring)
- `march` supplies a sphere-traced hit function. [Drawables](guide.md#what-draws-a-thing-and-what-you-can-bring)
- `castRay` queries scene geometry along a ray. [Drawables](guide.md#what-draws-a-thing-and-what-you-can-bring)
- `Form` describes drawables for readers such as the baker. [Custom drawables](guide.md#what-draws-a-thing-and-what-you-can-bring)
- `lamp` supplies point lights with optional depth-map shadows. See `Light` and `SceneSpec.lights`. [Choosing and lighting a style](guide.md#choosing-and-lighting-a-style)
- `Painted` adds material channels including roughness, normal, emission and scatter. [Styles](guide.md#choosing-and-lighting-a-style)
- `pbr` ships a swappable material and light model, with field AO and a sky environment that can follow an animated sun. `PbrParts` includes slots for `ggx`, `smithGgx`, `schlick`, `lambert`, `skyLight`, `daylight` and `aces`. [Choosing and lighting a style](guide.md#choosing-and-lighting-a-style) · [PBR slots](styles.md#pbr-a-style-from-swappable-parts)
- `glossed` adds a sky reflection to a body. [Choosing and lighting a style](guide.md#choosing-and-lighting-a-style)
- `depthOfField` adds a thin-lens post pass. [Photographing a scene](guide.md#photographing-a-scene)
- `filmGrain` adds time-seeded monochrome grain. [Photographing a scene](guide.md#photographing-a-scene)
- `pass` renders a picture for another drawable to sample. See `Pass`, `PassCamera` and `PassSpec`. [Offscreen passes](guide.md#what-draws-a-thing-and-what-you-can-bring) · [Pass API](api.md#srcframepassts)
- `recur` bakes a stepped expression state for playback. [Drawing a scene elsewhere](guide.md#drawing-a-scene-elsewhere) · [Recurrence API](api.md#srcmaprecurts)
- `compute` adds WGSL passes at build, load or frame time, or as stepped state. `fetch` can read their output. [Writing a compute pass](guide.md#writing-a-compute-pass)
- `iterate` adds an `until` callback for early exit. [WGSL text](guide.md#maths-as-wgsl-text-wgsl) · [Expression API](api.md#srcexprts)
- `tile` accepts a chain of folds, and `fold2` and `fold3` are available from `loft`. [Shape guide](guide.md#adding-a-shape) · [Fold API](api.md#srclibfoldts)
- `--precision mixed` selects opt-in f16 shader overlays and reports `f32-fallback` on devices without shader f16. [Rendering precision](guide.md#rendering-precision)
- `loft show` lists shader modules and `--thing` prints the ones for a named drawable with source sites. [WGSL text](guide.md#maths-as-wgsl-text-wgsl)

At this revision, `draw.shader` and `loft check --at … --explain` appear in the proposal. They are absent from the shipped draw and CLI command surfaces. Use a custom `Drawable` for a raw drawable and `loft check` for the current witness output. [Draw API](api.md#srcdrawindexts) · [Commands](guide.md#commands)

## Run the codemod

~~~sh
bun tools/migrate.ts old.ts --out migrated.ts
bun tools/migrate.ts old.ts --json
bun tools/migrate.ts --batch seeds.jsonl --out migrated.jsonl
~~~

The default writes source to stdout and prints any diagnostics to stderr. `--json` writes `{code, changed, unported}` to stdout, and `--out` in single-file mode writes the TypeScript source. Batch mode accepts `{id, code}` and emits `{id, code, changed, unported}` per row. Either mode accepts `-` for stdin. Diagnostics refer to the original lines. Run the output somewhere that can resolve the `loft` package. When a deep import has a different meaning from the root, file mode rewrites archived paths relative to the output file and JSONL mode uses the local Loft source path.

| Reported reason | What to review |
| --- | --- |
| `typescript-syntax` | Repair the input syntax before migrating. |
| `loft-import-path` | Find the current module or public equivalent for the unresolved path. |
| `condition-options`, `legacy-hold-type` | Turn computed option records, first-class legacy helpers and old hold types into comparisons with explicit time ranges. |
| `clearance-window` | Resolve old arming times or sample rates before adopting a closed range. |
| `stops-at-contact-and-span` | Add the first-arming contact and attack-span clearance checks. |
| `raw-wgsl` | Choose WGSL graph or raw semantics and wire up dependencies. |
| `drawable-forms` | Describe custom geometry with `Form` variants and update its consumers. |
| `scene-lights` | Move the light into the scene and review how it lights other things. |
| `point-instancer` | Identify the old drawable, its poses and any custom instancing implementation. |
| `slot-time` | Review a shared or computed callback, an options object the tool cannot inspect, or a callback that mutates or reflects on its parameters. |
| `cellular-result` | Decide where a first-class `cellular2` callback needs the old vec2 projection. |
| `creative-scene` | Choose the scene settings for a computed preset or a default export the tool cannot identify as a drawable array. |

## Appendix — CPU-only seed score

| Creative seed pack | Input rows | Changed | Compiled cleanly | Needs a hand fix |
| --- | ---: | ---: | ---: | ---: |
| `crab-actions-v1` | 103 | 103 | 103 | 0 |
| `crab-improve-v1` | 67 | 67 | 67 | 0 |
| **Total** | **170** | **170** | **170** | **0** |

Each pack was read in place. Each migrated row was written to a temporary source file under Loft's `out/migration/` and built individually with `LOFT_NO_GPU=1 bun src/cli/main.ts build <row.ts> --out <bundle-dir>`. The score requires exit 0 and an empty `unported` array. All 170 use the frozen package entry and Creative's drawable-array contract. The focused codemod tests cover API changes that the seed corpus does not exercise.

To reproduce, run the batch command above once per pack, using the `seeds.jsonl` in `training/loft_envs/seed_programs/<pack>/`. Materialize each resulting `code` field in this checkout and run the CPU-only build command for every row. This verifies scene loading, compilation and bundle emission. GPU shader validation and pixel comparison are outside this score.
