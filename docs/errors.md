# When a scene fails

<!-- `loft context` prints this file down to "## Every refusal" (src/cli/context.ts). Keep that part short. -->

`render`, `frames`, `clip` and `check` end with one line on stderr, `loft: <kind>: <message>`, and one exit code:

| Exit | Last line starts | Meaning | Change the scene? |
|---|---|---|---|
| 0 | (none) | drew, or every check passed | no |
| 1 | `loft: internal` | an untyped error inside Loft: a bug, or a mistake no constructor catches yet (stack printed above) | maybe; report it |
| 2 | `loft: usage:` | the command line asked for something absent: a flag, camera, variant, option, input, style | no, the command |
| 3 | `loft: load:` | the module did not load as a scene: syntax, an unknown import, a throw at top level, wrong exports | yes |
| 4 | `loft: scene:` | a constructor refused the scene; the message says given, expected and the fix, and ends `(at file:line)` | yes |
| 5 | `loft: check:` | the scene drew, and the checks named on the line failed | yes |
| 6 | `loft: environment (not the scene's fault):` | the GPU, the WebGPU binding, or a missing `cwebp`/`ffmpeg`; a shader or pipeline that failed to build was already tried on 3 fresh devices | no |
| 7 | `loft: budget:` | (`loft serve` only) a stated limit ran out before an answer: a compile past `--compile-seconds`, a CPU limit, or `--pending` full; the line names it | no; decide whether it is a penalty |

Does it type-check: `pnpm exec tsc --noEmit`. Does it render, and what to change:

```sh
loft render examples/shell.ts --out out/shell.webp --views three
loft check examples/strike.ts
```

The most likely refusals, each `given ...; expected ... Fix: ...`:

| Message (start) | Cause | Fix |
|---|---|---|
| `scene(): given sea: ... (the old key) and no ground` | `sea:` is now `ground:` | `ground: sea({ swell: [...] })`, `ground: plane` or `ground: nothing` |
| `load: ... Export named 'x' not found` | an import Loft does not export | the nearest name it suggests, or `loft api` |
| `add: given a vec3 and a vec2` | arithmetic on vectors of two sizes | make both one size: `vec3(v.xy, 0)`, or `.xy` |
| `vec3: given 2 components` | too few or many numbers | exactly 3, or one for all |
| `draw.surface "x": given a surface whose point is an f32` | the surface function returns a number | return `vec3(x, y, z)` |
| `draw.surface "x": given paint that returns an f32` | paint returns a number | return `vec3(r, g, b)` |
| `scene(): given a Surface ... at things[k]` | a shape put in `things` undrawn | `draw.surface("name", shape, { paint, grid, sides, outline })` |
| `draw.surface "x": given undefined for the surface` | the shape is not defined yet on that line | define it before drawing it |
| `draw.surface "x": given sides "both"` (or `outline`) | an option misspelt | `sides: "one" \| "two"`, `outline: "hull" \| "border" \| "none"` |
| `scene(): camera "c": given fovY 60` | degrees for radians | `fovY: 60 * Math.PI / 180` |
| `scene(): given two things named "x"` | names key shaders | rename one |
| `draw.field "x": the field can overshoot` (or `march: ...`) | the field changes a little faster than distance (a fold's stretch) | fix the field, or the `safety` it suggests |
| `draw.field "x": the field is not a distance` (or `march: ...`) | measured above 2: a degenerate primitive or an unscaled field | fix the field; `safety` would hide it |
| `draw.volume "x": given bounds ...` | an empty, unordered or non-finite axis at f32 precision | supply finite `min` and `max` world-space corners with each max above its min |
| `draw.volume "x": given steps ...` (or `shadow.steps ...`, `g ...`) | an invalid sample count or phase anisotropy | use a positive whole count such as 64 or 6, and keep `g` strictly inside (-1, 1) |
| `draw.volume "x": given density as ...` (or `albedo as ...`, `emission as ...`) | a field returns the wrong type | return an f32 for density or a vec3 for a colour |
| `X needs a sea; this scene's ground is a plane` | a wake or foam on dry ground | `ground: sea(...)`, or drop the thing |
| `usage: --style "x": no such style` / `input "x": ... no such input` | the command names what is absent | a name from the list printed |
| `pbr sky: given a sky that reads time` (or a param; or `the CPU cannot evaluate`) | `pbr` evaluates its sky, tone and lobe on the CPU once, when the style is built | a sky of its direction and the sun alone; for a GPU-only sky, `pbr({ sky, clear: [r, g, b], environment })` |

## Every refusal

Each row is a fixture in `tests/fixtures/mistakes/`; `bun tests/fixtures/mistakes/run.ts` prints each one's exit code
and last line.

| # | Message (after `loft: <kind>:`) | Cause | Fix |
|---|---|---|---|
| 01 | `draw.surface "sheet": given a surface whose point is an f32` | the surface returns one number | return `vec3(x, y, z)` |
| 02 | `scene(): given two things named "ball"` | two drawables share a name | rename one |
| 03 | `scene(): camera "three": given eye and target both at [...]` | the camera looks at its own eye | move the eye back |
| 04 | `draw.surface "ball": given paint that returns an f32` | paint returns a grey as one number | `vec3(g, g, g)` |
| 05 | `draw.surface "ball": given grid.u with 0 cells` | an empty grid | `spacing.even(32)` |
| 06 | `draw.surface "body": given undefined for the surface` | the shape is used before it is defined | define it first |
| 07 | `input "aim": given a default of 2 numbers for a vec3` | the default has the wrong size | `input("aim", "vec3", [0, 1, 0])` |
| 08 | `usage: input "wind": the scene declares no such input` | `--input` names an undeclared input | drop it, or declare `input("wind", ...)` |
| 09 | `usage: --style "inky": no such style (styles: ...)` | an unknown style | one of the styles listed |
| 10 | `draw.field "blob": the field is not a distance` | the field changes ten times faster than distance | fix the field |
| 11 | `tabulate "lift": given 10000000 samples` | a table larger than one GPU binding | a few thousand samples |
| 12 | `a number in an expression: given Infinity` | a division by zero in a constant | the line named at the end |
| 13 | `scene(): given things: undefined` | no `things` | `things: [draw.surface(...)]` |
| 14 | `usage: slot "colour" of design "d" has no option "purple"` | `--variant` names an absent option | one of the options listed |
| 15 | `scene(): given sun [0, 0, 0], which points nowhere` | a zero sun | `sun: [0.3, 0.8, 0.4]` |
| 16 | `add: given a vec3 and a vec2` | vectors of two sizes | `vec3(v.xy, 0)` or `.xy` |
| 17 | `vec3: given 2 components` | a short vector | `vec3(x, y, 0)` |
| 18 | `scene(): given a Surface (a shape, not yet drawn) at things[0]` | a shape in `things` | `draw.surface("name", shape, {...})` |
| 19 | `scene(): camera "three": given fovY 60` | degrees for radians | `60 * Math.PI / 180` |
| 20 | `draw.surface "ball": given sides "both"` | a misspelt option | `sides: "one"` |
| 21 | `load: ...: the default export is a design` | a design exported as default | `export const design = design({...})` |
| 22 | `scene(): given sea: ... (the old key) and no ground` | the key before 2026-11 | `ground: sea({...})` |
| 23 | `load: ... Export named 'spiralTub' not found` | a misspelt import | `spiralTube` (suggested) |
| 24 | `recur "drift": given a read of the host input "push"` | a baked state's step reads what a game changes | a constant, or a closed form that looks like state (`jolt`) |
| 25 | `passes: given a cycle: "mirrorLeft" reads "mirrorRight"; ...` | two passes each hold a draw that reads the other (two mirrors facing) | let one of those draws stop reading its pass |
| 26 | `recur "drift": given a read of the pass "above"` | a baked state's step reads a picture drawn this frame | compute it from closed forms (`project` is arithmetic) |
| 27 | `scene(): given lights: 8 lights; expected at most 7 besides the sun` | each light is a term in every lit fragment | one function for many sources (a row by repetition) |
| 28 | `ball: paint: given "gloss" as vec3; expected "gloss" as f32, as toon reads it` | a channel given as another type than the style reads it | give it as the style's comment says |
| 29 | `"stroke": given a paint with its own shade or scatter on a flat look` | a flat look's paint is the final colour: nothing to wrap | return the colour, or draw a lit surface |
| 35 | `solve: given holds that cannot all be met: equal from t = 0 to 1.3 (file:7); equal at t = 1.2 (file:7) ...` | two bounds on the same spline component contradict each other where their time windows overlap, even allowing the equality tolerance | drop or loosen one of the holds named |

`SolveError.failure` distinguishes three outcomes (exit 4 for each).

- `infeasible` names a certified contradiction, such as incompatible bounds on a planted foot at landing.
- `unsolved` with reason `search` means the local search stopped. Give a rough starting curve with `spline(n, from)`
  on the intended side of the obstacle. Reason `verification` names the hold, time and amount by which the
  returned f32 curve missed its bound. For an equality, set `within` to an acceptable error in the quantity's
  units. For a clearance, increase `delta` or change the spline family. Reason `budget` means the shared `budget.ms` expired. Increase it or simplify the holds.
- `unreadable` names the hold and, for a non-finite margin or derivative, the evaluation time. Check the domain of the
  expression at that time. Holds must also have a CPU evaluator and derivatives, and read each unknown at the hold's own time.

Refused the same way, without a fixture each: a step reading a param, a curve row, a per-frame table or a checkpointed
recurrence; a span that is not a whole number of steps; a history over 120 x 64 MiB; a checkpointed state read at a time
other than `time`; a looping scene whose state span does not cover its loop (`scene() with period P`). A clip past the
shortest `until` of a scene that does not loop is a usage error (exit 2). Baking a recurrence for `build` or a sampled state check requires a GPU (exit 6 without one).

## Ranged checks

A ranged check can pass, fail, or finish **undecided**. `loft check` exits 5 for a failed or undecided check and
counts the two separately. Its table gives the proven margin bounds, the witness when available, and the stop reason.

- `undecided: boxes` or `undecided: ms` means the shared enclosure count or wall-clock budget ran out. Raise the indicated budget or narrow the domain.
- `undecided: width` identifies a box the search could no longer split. An unbounded operator is named when known. Narrow its operands or rewrite the condition around its domain.
- `undecided: unbounded` names a range end that could not be evaluated, including numeric overflow. Supply finite, defined ends.
- `undecided: recur` identifies the scene's baked recurrence. A ranged read of raw WGSL or a GPU-only function names the function. A library function without an enclosure, a CPU table and a skin target also report their respective reasons. Use a supported expression or a sampled measurement.

A malformed range or comparison is a scene error (exit 4). Give the ends independently of every quantified leaf,
bind free variables, and use `ge`, `gt`, `le` or `lt`. Express approximate equality as a band.
