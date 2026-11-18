# Loft: a guide

Loft describes animated 3D things as TypeScript and compiles the description to WGSL shaders plus tables of numbers
that any WebGPU host can draw. This guide is for someone who knows TypeScript and nothing else about Loft. Each section
is a few lines of code that run, and a pointer to a fuller example. Every snippet here is checked by
`bun tools/check_snippets.ts`.

For the same ground written for a model, with the whole list of exports, run `loft context`.

## Three ideas

1. **You write expressions, not numbers.** The maths of a function (a spine, a girth, paint, a field) is WGSL text in
   a `wgsl` template; the structure (which things, where, the cameras, the checks) is TypeScript. Both build one graph.
   The compiler emits it twice: as WGSL for the GPU and as JavaScript for the CPU, so the checks measure exactly what
   is drawn.
2. **A body is a coordinate system.** A swept body has coordinates (m along it from 0 to 1, v round it). Paint, fins,
   foam and the place a part is attached are functions of those coordinates, so they move with the body for free.
3. **Time is an argument.** `time` is an input like any other. Motion is a closed-form function of it, so any frame
   can be drawn without the frames before it, and a loop closes when the formulas say so.

## Install and the first scene

You need [bun](https://bun.sh) and pnpm; `cwebp` writes stills and `ffmpeg` writes clips. Rendering is headless
WebGPU through bun, so no browser is needed for pictures.

```sh
git clone <this repository> loft && cd loft
pnpm install
alias loft="bun src/cli/main.ts"
```

Put this in `examples/reed.ts` (a scene is a module whose default export is a `scene`):

```ts run
// examples/reed.ts
import { plane, scene, strand, toon, wgsl } from "../src/index.ts";

const shape = wgsl`
// The centre line, one point for every m from 0 (root) to 1 (tip). The tip sways with time.
fn spine(m: f32) -> vec3f { return vec3(m * m * sin(time) * 0.6, m * 4.0, 0.0); }
fn girth(m: f32) -> f32 { return 0.25 - 0.2 * m; }`;

const reed = strand({ name: "reed", spine: shape.spine, girth: shape.girth, tips: ["open", "point"], paint: [0.85, 0.55, 0.2] });

export default scene({
  ground: plane,
  things: [reed],
  style: toon(),
  sun: [-0.4, 0.8, 0.45],
  cameras: { three: { kind: "perspective", eye: [6, 4, 8], target: [0, 2, 0], fovY: 0.6 } },
});
```

Look at it:

```sh
loft render examples/reed.ts --out out/reed.webp --views three
loft frames examples/reed.ts --out out/reed_frames.webp --times 0,0.8,1.6,2.4 --view three
loft dev examples/reed.ts
```

`render` writes one picture per camera into one sheet. `frames` draws one camera at several times. `dev` serves a live
page on port 5173 with orbit, the scene's cameras, a time bar and a slider for every `param`; it reloads when you save.
Every command prints its timings and an `rgba` hash of each frame, so two versions of a picture can be compared exactly.

Today a scene must have a sea (see "The ground" below).

## Maths as WGSL text: `wgsl`

Maths inside a function can be written as a shader writer writes it. `wgsl` takes WGSL text and returns one callable
per `fn`: an ordinary function of expressions, so it fills any slot a TypeScript function fills.

```ts run
import { plane, scene, strand, toon, wgsl } from "../src/index.ts";

const eel = wgsl`
fn spine(m: f32) -> vec3f {                     // m: 0 head .. 1 tail
  let wave = sin(6.0 * m - 3.0 * time) * 0.25 * m;
  return vec3(2.0 - 4.0 * m, 0.6 + 0.2 * m, wave);
}
fn girth(m: f32) -> f32 { return 0.24 * sqrt(4.0 * m * (1.0 - m)) * (1.0 - 0.5 * m) + 0.02; }
fn paint(m: f32, v: f32) -> vec3f {
  let back = smoothstep(-0.3, -0.12, sin(v * 6.2832));
  let spot = step(0.25, noise3(vec3(m * 18.0, v * 4.0, 0.0)));
  return mix(vec3(0.85, 0.8, 0.6), mix(vec3(0.1, 0.25, 0.2), vec3(0.9, 0.7, 0.2), spot), back);
}`;

export default scene({
  ground: plane, style: toon(), sun: [-0.4, 0.8, 0.45],
  things: [strand({ name: "eel", spine: eel.spine, girth: eel.girth, paint: eel.paint })],
  cameras: { side: { kind: "perspective", eye: [1, 4.5, 5.5], target: [0, 0.6, 0], fovY: 0.6 } },
});
```

- **The same graph.** Each call lowers the function's body with the arguments it was given, through the same
  constructors the call form uses: `eel.girth` is the graph `add(mul(...), 0.02)` would be, with the same CPU twin,
  checks, intervals and `gradient`. Text holds what the call form cannot write well: loops with state, `if`, `break`,
  early `return`, swizzles (`p.zxy`, `v.xz = ...`) and operator precedence.
- **Names in scope**, in order: locals; the module's own `fn`s, `struct`s and `const`s; holes; `time`; the library's
  expression functions by their exported names (`noise3`, `hash22`, `sdRoundBox`, `smin`, `fold3`, `mod`, ...).
  `gradient(h, p)` differentiates `h` with respect to the vector `p`. `loft context` prints every builtin and prelude
  name.
- **Holes** bring in what the text cannot name: `${param("reach", 1.2, { min: 0.8, max: 1.6 })}`, an input, a solved
  curve, a TypeScript function of expressions (callable in the text), a number or an array (a constant). A string is
  refused: the template splices values, never text. `wgsl(source)` reads text kept in a file.
- **The subset** is `f32`, `vec2f`..`vec4f`, `bool`, `mat2x2f`, `mat3x3f`, constant arrays and structs of these; `let`,
  `var`, assignment, `if`/`else`, `for (var i = 0; i < N; i++)` (any `N`, or `i32(expression)`), `break`, `continue`,
  `return`, and the numeric builtins. A struct return fills a record slot by field name; a function may take fewer
  parameters than its slot passes.
- **WGSL text means WGSL.** `round` ties to even and `%` is the truncated remainder, as on the GPU (Loft's call-form
  `round` and `mod` floor). `fma` is a multiply and an add on both sides. `1` is an integer: `let n = 1; n * 0.5` is
  refused, with the fix `1.0`.
- **Raw functions.** A function using anything outside the subset (integers, bit operations, textures, `dpdx`,
  `while`, atomics, pointers) is emitted as written and still renders when graph code calls it; holes and `time` reach
  it as added parameters. The CPU twin and ranged checks refuse it by name and line: `raw function pcg at
  eel.ts:14: u32 integers`. What a shader stage knows of the screen is reached the same way, in one line each:
  `fn pixel_rate(x: f32) -> f32 { return fwidth(x); }` (the width to antialias an edge of `x` with, fragment stage
  only), `view_dir(p)`, `pixel_world(p)` and `frame.eye.xyz`.
- **Errors** point at the text: `examples/eel.ts:9:22: given unknown name "nois3"; ... Fix: did you mean noise3?`, the
  line quoted with a caret, and the TypeScript line that called into it.
- **Raw lines the GPU refuses** are the scene's too: only the GPU reads a raw function, so a typo there surfaces when
  the scene loads, as `loft: scene: examples/eel.ts:16: given raw function pcg with a line the GPU refused (unresolved
  value 'hsh')`, exit 4, with the line quoted and the GPU's message.
- **Seeing what the GPU compiles.** `loft show examples/eel.ts` lists the scene's shader modules; `--thing eel` prints
  the ones for the eel in full. Each statement lowered from your text ends with its line (`/* eel.ts:9 girth */`), each
  line of a raw function with its line (`// eel.ts:16`), and a comment says who wrote each chunk.
- **Call form** is the same graph built from TypeScript calls (`add(a, b)`, `mul`, `sin(time)`, `vec3(x, y, z)`,
  `select`, `iterate`); text lowers to exactly these constructors. Use it where TypeScript does the work: `.map` over
  legs, a list of stations, a helper generic over any vector size, `solve` specs. The library itself is written in it,
  so `loft api <name>` shows a function's TypeScript signature; text calls it with the same arguments.

## Rendering precision

Scenes default to `f32`. Use the same `--precision mixed` switch for `build`, `render`, `frames`, `clip`, `profile`
or `dev`. The renderer reports `mixed` when the device grants `shader-f16` and `f32-fallback` otherwise. A mixed build
is portable. Its manifest carries both choices, and `render --artefact` selects at load. To force full precision for
a comparison, pass `--precision f32`.

```sh
loft build examples/mixed_precision.ts --precision mixed --out out/copper
loft render --artefact out/copper --views three --out out/copper.webp
```

For a static colour, normal-map or paint-channel lookup, use `tabulate(name, fn, n, "static-shading")`. This gives
the compiler permission to store its samples in half precision in a mixed build. Samples must be finite and within
`[-1, 1]`. Lookup indices and interpolation are f32. Use `"static"` for geometry, signed distances, or general numeric
and HDR data. The example uses a 256-sample palette. Its GPU buffer occupies 2 KiB in mixed and 4 KiB in f32.

Built-in bloom and grain use f16 for bounded colour arithmetic at ordinary gains. Soft media use it for bounded lighting weights.
Ray geometry, optical depth, HDR lighting, geometric normals, camera transforms, time, depth, and arbitrary author
expressions keep f32. Small lookup errors can be amplified by sharp material thresholds, so compare the look in
both modes when deciding whether a lookup qualifies.

## Adding a shape

A shape is a function. The library has a few families, each a short composition over the level below, so leaving one
is a step down and not a rewrite. `spiralTube` is a tapered tube on a logarithmic spiral; `spiralLine` is its centre
line and radius in the form `strand` takes, so you can keep the curve and change the rest.

```ts run
import { draw, scene, plane, spacing, spiralLine, spiralTube, strand, surface, toon, vec3, wgsl } from "../src/index.ts";

// A spiral tube: one law ties the curl to the taper (read its doc comment for what it cannot do).
const frond = spiralTube({ name: "frond", length: 5, curl: 2.4, taper: 0.2, girth: 0.3, out: [0, 1, 0], toward: [1, 0, 0], paint: [0.3, 0.6, 0.35] });

// One step down: the same kind of centre line, another radius law (a club that swells near the tip).
// The library's line comes into the text through holes, as functions.
const line = spiralLine({ length: 4, curl: 1.2, taper: 0.5, girth: 0.25, out: [0, 1, 0], toward: [-1, 0, 0] });
const club = wgsl`
fn spine(m: f32) -> vec3f { return ${line.spine}(m) + vec3(-3.0, 0.0, 0.0); }
fn girth(m: f32) -> f32 { return ${line.girth}(m) * (1.0 + 1.5 * sin(m * 3.14159265)); }
// Any function of (u, v) is a surface: here a bowl, a profile turned round the y axis.
fn bowl(u: f32, v: f32) -> vec3f {
  let r = 0.4 + 1.2 * sin(1.4 * u);
  let a = 6.2831853 * v;
  return vec3(r * cos(a) + 3.0, 1.5 * u + 0.2, r * sin(a));
}`;
const bowl = surface(club.bowl, { u: ["cap", "free"], v: "periodic" });

export default scene({
  ground: plane,
  things: [frond, strand({ name: "club", spine: club.spine, girth: club.girth, up: line.up, tips: ["open", "round"], paint: [0.8, 0.5, 0.3] }), draw.surface("bowl", bowl, { paint: () => vec3(0.9, 0.85, 0.7), grid: { u: spacing.even(24), v: spacing.even(32) }, sides: "two", outline: "border" })],
  style: toon(),
  sun: [-0.4, 0.8, 0.45],
  cameras: { three: { kind: "perspective", eye: [4, 6, 12], target: [0, 1.5, 0], fovY: 0.7 } },
});
```

The families: `strand` (any centre line, girth, section and twist), `spiralTube` and `shell` (spirals), `limb` (a chain
of segments reaching a target), `fan` and `crest` (sheets), `surface`, `lathe`, `extrude`, `sheet`, `loft` (maps of
(u, v)), `draw.field` (a distance function traced per pixel, for folded and recursive solids), `draw.relief` (fine
form on a surface), `impostors` and `drawCopies` (many small solids), and `hull` and `ship`. Examples:
`examples/serpent.ts` (a swept body with parts placed on it), `examples/shell.ts` (a surface written directly),
`examples/coral.ts` and `examples/nest.ts` (traced folds), `examples/shell_relief.ts` (relief), `examples/brig.ts`
(a ship from numbers).

## Adding motion

Motion is a function of `time`. The library's motion helpers are the twelve principles of animation as formulas
(`docs/animation_principles.md`): an action has three beats, a wind-up that goes the wrong way first, an arrival that
overshoots, loose parts that arrive late and swing on, and a hold that never quite holds still.

```ts run
import { add, alive, anticipated, beats, dragged, mod, mul, scene, plane, strand, swing, time, toon, wgsl } from "../src/index.ts";

// Every 4 seconds: a 0.8 s wind-up, a 0.2 s bow, then 1.5 s to settle.
const b = beats(mod(time, 4), { anticipate: 0.8, act: 0.2, settle: 1.5 });
// Lean back through the wind-up, snap forward on the act, then swing and settle; never quite still.
const lean = add(add(mul(anticipated(b.anticipate), -0.3), mul(b.act, 1.0)), add(mul(b.settle, -1.0), swing(b.after, 0.4)));
const bend = add(lean, alive(time, 0.05, 1));

// The timing above is structure, composed in TypeScript; the body is maths, in text. `bend` and `dragged` come in
// through holes: the tip reads a later clock than the root (drag), so its sway arrives late.
const shape = wgsl`
fn spine(m: f32) -> vec3f { return vec3(m * m * (3.0 * ${bend} + 0.15 * sin(${dragged}(time, m, 0.4))), 4.0 * m, 0.0); }
fn girth(m: f32) -> f32 { return 0.3 - 0.22 * m; }`;
const stalk = strand({ name: "stalk", spine: shape.spine, girth: shape.girth, tips: ["open", "round"], paint: [0.4, 0.55, 0.85] });

export default scene({
  ground: plane,
  things: [stalk],
  style: toon(),
  sun: [-0.4, 0.8, 0.45],
  period: 4,
  cameras: { side: { kind: "perspective", eye: [0, 3, 12], target: [0, 2, 0], fovY: 0.6 } },
});
```

`period: 4` says the scene loops every 4 seconds; `alive` takes a `period` (or wrap the build in `looping`) when idle
motion must come round exactly. Other tools: `keys` and `through` (poses on a timeline or a path), `response` (a damped
spring's answer to a step), `retarded` (a clock delayed down a body), `struck` (a pose struck and held dead still),
`cast` and `score` (expressions as named poses scored against beats), `solve` (a curve from what must hold, below), `gait` and `footing` (feet planted while the
body moves), `pendulum` and `dart` (hanging and nervous parts), `routine` (a group on counts: unison, canon, mirror,
solos). Look at frames across an action before and after: `loft frames <scene> --times 0,0.4,0.8,1.2,1.6,2 --view side`.

## Motion from constraints: `solve`

When what you know is what must hold, not the curve (planted here until then, on the deck at 1.2 s, clear of the
rail always), `solve` finds the curve: the smoothest one of a family that meets every hold. It runs once when the scene
is built and returns ordinary expressions of `time` (`bspline(time, over, [numbers])`), so the result is edited, read
at another time and composed like any other curve. Holds are conditions, the same formulas a check takes: a comparison
(`ge`, `le`, `gt`, `lt`, or several joined by `and`) or `equal(a, b, within)` over any expressions of the unknowns. A
bare condition holds over the whole span; `always([time, a, b], condition)` holds it over an interval, and
`[time, t, t]` is an instant.

```ts run
import { always, distance, equal, ge, plane, scene, solve, spline, strand, time, toon, vec2, vec3, type F } from "../src/index.ts";
import { derivative } from "../src/grad.ts";

const BAR = [1.5, 1.2] as const;   // a bar to hop over, end on (x, y)
const hop = solve({
  over: [0, 2],
  vary: { x: spline(23), y: spline(23, 1) },   // a number per 0.1 s; `from`: start above the bar, not under it
  hold: ({ x, y }) => [
    always([time, 0, 0.4], equal(vec2(x, y), [0, 0.3])),                     // waits on the ground
    always([time, 0.6, 1.1], equal(vec2(derivative(derivative(x, time), time), derivative(derivative(y, time), time)), [0, -9.81], 0.001)),
    always([time, 1.5, 2], equal(vec2(x, y), [3, 0.8])),                     // on the block, still
    ge(distance(vec2(x, y), BAR), 0.45),                                     // clear of the bar: bare, so always
  ],
  prefer: { jerk: 1 },
});
const ball = strand({ name: "ball", spine: (m: F) => vec3(hop.curves.x, hop.curves.y, m), stations: 4, girth: 0.3, tips: ["round", "round"], paint: [0.9, 0.4, 0.2] });
const bar = strand({ name: "bar", spine: (m: F) => vec3(BAR[0], BAR[1], m), stations: 4, girth: 0.08, paint: [0.3, 0.3, 0.35] });

export default scene({
  ground: plane, things: [ball, bar], style: toon(), sun: [-0.4, 0.8, 0.45],
  cameras: { side: { kind: "perspective", eye: [1.5, 1.5, 9], target: [1.5, 1, 0], fovY: 0.6 } },
});
```

Use `checks: hop.holds` in a design build, or `export const checks = hop.holds` beside a scene. Each check is the
condition as written, proved by `always` over its window (the whole span for a bare one). `equal(a, b, within)` is
true where every component of `a - b` is within `within` (default `TOLERANCE`, 1e-5) in its own units:
`equal(accel, gravity, 0.001)` allows 0.001 m/s² per axis. `solve` pins an `equal` as a target. It knows one by the node
`equal` returns, not by its shape, so a band spelled another way (`le(length(a - b), w)`) is solved as the inequality
it literally is: correct, but less pinned. A hold
under `always` is named in errors by its line; a bare one by its place in the list (`hold 4 of the solve at file:line`).
`eventually`, `or`, `not` and an `always` over a param are refused: the search is local and cannot choose a time or a
branch, and params take their defaults while solving.

For a bound (`ge` and the rest), the solver targets a positive margin of `delta` (default 4e-6 in the margin's units).
That leaves four times the solver's 1e-6 settling allowance while fitting inside the 5e-6 interior target
of a coincident default equality, as when a planted foot also has to clear the ground. The ranged check
compares the authored margin to zero. `hop.report` gives the target, the solve time and each hold's worst
measured miss. Verification evaluates the returned f32 spline controls on the CPU twin at f32 frame times
on the fine grid and at refined peaks. The ranged checks cover the time between those samples.

Identical problems reuse their verified curves in the same process, which keeps the 128 most recently used solutions.
Set `budget: { ms: 2000 }` to give setup,
optimisation, verification and conflict diagnosis a shared two seconds. Expiry raises `unsolved` with reason
`budget`. A local search failure raises `unsolved` with reason `search`. A clearance constraint can be met on either
side of a rail, so use `spline(n, from)` to start on the intended side. `infeasible` names a certified contradiction,
such as two incompatible positions for a foot at the same time. For rigid motion, solve joint angles or targets
and build the parts with `turnedAbout`, `poseFrame` or `chain`.

`examples/second/heron_leap.ts` solves a heron's crouch, leap and landing; `agent_notes/2026-11-18_solve/NOTE.md` has
the method.

When you call `runChecks` from TypeScript, branch on `verdict.kind`: `"ranged"` (proved over its range: `holds` is
`true`, `false` or `"undecided"`, with `spent` for the reason, the proven `bounds`, best `witness` and search `work`)
or `"sampled"` (measured at instants: `pass` and `samples`; a pass means no failure at those samples, never that it
holds). `checkOutcome(verdict)` gives `"pass"`, `"fail"` or `"undecided"` for either. See `examples/ranged.ts` for a mix
of the two kinds in one scene.

## An action a game can read, with its checks

An attack is defined once, from what a game's simulation would give it: when it arms, how long the warning is, the
hit shape, the target. The pose through time, the warning on the water and the checks are derived from that one
definition. Export `checks` beside the scene and `loft check` runs them on the CPU, from the same expressions the GPU
draws, and exits 1 on a failure.

```ts run
import { add, animate, attack, budget, f, hit, landsInside, minScreenSize, mul, part, scene, sea, strand, sub, telegraph, toon, vec2, vec3, warned, type F } from "../src/index.ts";

// The rest pose: a tentacle standing out of the water. An attack moves the spine's m = 0 end, so that is the tip.
const rest = (m: F) => vec3(mul(sub(1, m), 0.5), add(-0.5, mul(sub(1, m), 4)), 0);
const lash = attack({
  name: "lash",
  arm: 3, // seconds: when it lands
  warn: 2, // seconds of warning before that (it must start before the wind-up does)
  target: { kind: "disc", centre: vec2(7, 0), radius: f(1) },
  part: "tip",
  motion: { kind: "lunge", hit: hit.capsule({ a: [1.5, 0], b: [6, 0], radius: 0.8 }), reach: 0.4, lead: 1, height: 0.6, coil: 1.2, rise: 1, approach: 0.3, neck: 1.2 },
});
const creature = animate(rest, [lash], (spine) => {
  const body = strand({ name: "tentacle", spine, girth: (m: F) => add(0.1, mul(m, 0.25)), tips: ["round", "open"], paint: [0.7, 0.3, 0.5] });
  return { things: [body], parts: { tip: part("tip", body, { from: 0, to: 0.15, tip: 0 }) } };
});

export default scene({
  ground: sea({ swell: [{ direction: [1, 0.3], length: 9, height: 0.15, speed: 1 }] }),
  things: [...creature.things, telegraph(lash)],
  style: toon(),
  sun: [-0.4, 0.8, 0.45],
  cameras: { three: { kind: "perspective", eye: [4, 9, 14], target: [3, 0.5, 0], fovY: 0.7 } },
});

export const checks = [landsInside(creature.strikes.lash!), warned(lash), minScreenSize(creature.parts.tip!, "three", 8, { at: 3 }), budget({ triangles: 100_000 })];
```

```sh
loft check examples/strike.ts
```

A condition over time has one spelling: a boolean expression (`ge`, `le`, `gt`, `lt`, `equal`, joined by `and`, `or`,
`not`) quantified by `always` or `eventually` over a range `[leaf, lo, hi]`, where the leaf is `time`, a param, an
input or a variable, and `[time, t, t]` is an instant. A quantity a check used to measure for you is now a measure, an
expression you compare: `clearance(body, target)` is how far a body's capsules are from a target (a disc, a column or
another body; negative inside), so "stays clear" and "stops at the target" are

```ts
const T = bite.first;                                                                 // the arming time
always([time, 0, T], ge(clearance(parts.body, ship), 0))                              // clear until it arms
always([time, T, T], le(clearance(strikes.bite.part, ship), 0.1))                     // reaches the ship at arming
always([time, sub(T, bite.span.before), add(T, bite.span.after)], ge(clearance(strikes.bite.part, ship), -0.05)) // never enters it
```

and each is proved over every t, not sampled. A clearance from a `skin` target is a search on the CPU, not an
expression: it is measured, a condition over it is sampled at 60 Hz, and its verdict says `no failure at N samples`.

The checks that are not conditions measure something no expression gives (a footprint's share outside a hit shape, a
triangle count, what reads on screen across a loop), so they sample and say so: `landsInside`, `warned`,
`gripReaches`, `releasesAt`, `throwLands`, `fliesClear`, `letsGoClean`, `shoves`, `frontFirst` (an attack's contract),
`minScreenSize`, `litBy`, `spansOnScreen`, `staysInView`, `expressionsRead` (what reads on screen), `limbsReach`,
`holdsStill`, a routine's `inSync`, and `budget`. `examples/strike.ts` is a serpent that bites one ship and sweeps its
tail at another, with a smear and a wake; `examples/grab.ts` takes hold of a ship and carries it.

`always([time, 0, 2], gt(gap, 0), { boxes: 2000, ms: 500 })` asks whether `gap` is strictly positive throughout two
seconds; touching zero violates it. A range end may use a held input or parameter (`bite.first`). Each decision
shares its budgets across nested ranges, point evaluations and case splits. `precision` (default 1e-4, in the margin's
units) is how tightly the margin is bounded; a costly measure proves sooner at 1e-3.

A ranged verdict's `holds` is `true`, `false` or `"undecided"`, along with `bounds` on the extreme margin, the best
sampled `witness`, its `margin`, `work`, and the stop reason in `spent.kind`.
An empty witness means the check obtained no point measurement. In JSON, non-finite bounds and missing numerical
measurements serialize as `null`. The enclosure follows the CPU twin's frame encoding and table stores.
See [Ranged checks in the error guide](errors.md#ranged-checks).

When a check fails, ask it why, and look at the frame it failed on:

```sh
loft check tests/fixtures/explain/heron_rail_raised.ts --explain     # the deciding line of each check that fails
loft check tests/fixtures/explain/heron_rail_raised.ts --at time=0.95 # every check's margin at t = 0.95
```

```
explain: always time in [0, 1.8]
  written at tests/fixtures/explain/heron_rail_raised.ts:22  `export const checks = holds(raised, ...`
  FAIL: margin -0.094868 at time=0.94301 (proven bounds [-0.094934, -0.094868])
  decided by ge(a, b)  at examples/second/heron_leap.ts:88  `ge(apart(rail, foot(0), add(foot(0), vec3(0.28, 0, 0))), 0.025 + pad),`
    a = 0.020132
    b = 0.115
  it reads, at the witness:
    (0.0021341, 0.020019)        examples/second/heron_leap.ts:43  `return length(sub(R, add(A, mul(ab, s))));`
  see it: loft render tests/fixtures/explain/heron_rail_raised.ts --out out/explain/heron_rail_raised.webp --views side --time 0.943
```

The deciding comparison is the one whose margin is the check's at the witness (an `and` is decided by its smaller
side). `--at` holds the named leaves (`time`, a param's name) at those values and leaves the others ranging.

## A creature with variants

A design is one subject with a record of numbers, slots with named options, and named variants. A variant is valid
when it builds and passes the subject's checks, whatever proportions it changes. Export it as `design`.

```ts run
// examples/beacon.ts
import { add, mul, scene, plane, strand, toon, vec3, type Drawable, type F, type Option } from "../src/index.ts";
import { design as makeDesign } from "../src/index.ts";

type N = { readonly height: number; readonly colour: readonly [number, number, number] };
type Top = Option<N> & { readonly draw: (n: N) => Drawable };
const tops = {
  base: { about: "a round bulb", draw: (n: N) => strand({ name: "top", spine: (m: F) => vec3(0, add(n.height, mul(m, 0.8)), 0), girth: 0.5, paint: n.colour }) },
  spike: { about: "a long point", numbers: { height: 3 }, draw: (n: N) => strand({ name: "top", spine: (m: F) => vec3(0, add(n.height, mul(m, 2.5)), 0), girth: 0.3, tips: ["round", "point"], paint: n.colour }) },
} satisfies Record<string, Top>;

const stem = (n: N) => strand({ name: "stem", spine: (m: F) => vec3(0, add(-0.3, mul(m, n.height + 0.3)), 0), girth: 0.15, paint: [0.3, 0.3, 0.3] });

export const design = makeDesign({
  name: "beacon",
  numbers: { height: 2, colour: [0.9, 0.6, 0.2] } as N,
  slots: { top: tops },
  variants: { tall: { about: "the spike, in a darker ink", pick: { top: "spike" }, numbers: { colour: [0.5, 0.3, 0.6] } } },
  compare: ["three"],
  build: (picked, n) => ({
    scene: scene({
      ground: plane,
      things: [stem(n), picked.top.draw(n)],
      style: toon(),
      sun: [-0.4, 0.8, 0.45],
      cameras: { three: { kind: "perspective", eye: [5, 4, 8], target: [0, 2, 0], fovY: 0.6 } },
    }),
    checks: [], strikes: {}, holds: {}, waves: {},
  }),
});
```

```sh
loft variants examples/beacon.ts
loft render examples/beacon.ts --variant tall,top=base --views three --out out/beacon_mix.webp
loft check examples/beacon.ts --combinations
```

`loft variants` draws every named variant through the design's `compare` cameras onto one labelled sheet; `--variant`
takes a name, `slot=option` picks, or both (`tall,top=base`). `docs/variants.md` is the full account, and
`examples/bosses/reefback_titan.ts` is a large design with five slots.

## Choosing and lighting a style

A style is a value of type `Style`: a set of functions in the same expression language as the shapes (how a surface
is shaded, its ink, the water, a cast shadow, the sky) plus full-screen post passes. Any scene draws in any style
without being edited.

```sh
loft render examples/serpent.ts --style woodblock --out out/serpent_print.webp
loft render examples/serpent.ts --style woodblock --light hatched --out out/serpent_hatched.webp
loft gallery --scenes "examples/serpent.ts,examples/storm.ts" --styles toon,papercraft
```

The named styles are `toon` (bands, an ink outline), `woodblock` (a print: flat inks, a printed shadow block, named
lightings in `docs/lighting.md`), `biolume` (a night sea), `papercraft` (folded paper), `sketch`, `neon` and `pbr`.
`pbr()` darkens traced field creases with ambient occlusion and follows a sun supplied by `movable(...)` or a
`vec3` frame expression. Swap the AO with `pbr({ occlusion: () => f(1) })` or replace water and floor with
`{ ...pbr(), water, ground }`. See `docs/styles.md` and the relief and moving disc in `examples/materials/`. In code, `withStyle(scene,
style)` and `withLight(scene, name)` do what the flags do, and `toon({ bands, outline, shadow, ink, water, sky, rim })`
takes options. Writing your own style is writing a `Style` value (below); `src/style/biolume.ts` (about 110 lines) is
the shortest complete one to copy.

## The ground

A scene stands on one of three grounds, given as `ground:`

```ts
scene({ ground: sea({ swell: [...] }), things, style, sun })   // water: any number of swells (an empty list is flat water)
scene({ ground: plane,   things, style, sun })                 // a flat floor at y = 0 in the style's ground colour
scene({ ground: nothing, things, style, sun })                 // no floor: the style's sky all round
```

A sea gives contact foam where bodies meet it and takes their cast shadows; `examples/maelstrom.ts` deforms it into a
whirlpool and `src/geom/afloat.ts` floats a body on it. A plane takes cast shadows and nothing else. On nothing there
is nowhere for a shadow to fall. Things that need water say so by name: a wake on a plane, or a telegraph on nothing,
throws a `GroundError`. The three smallest scenes that are not sea pictures are in `examples/first/`.

## Making a clip

```sh
loft clip examples/serpent.ts --out out/serpent.mp4 --seconds 8 --fps 30 --view three
loft frames examples/stormwing.ts --out out/sweep.webp --times 1 --count 6 --sweep strike=0..1
```

`clip` pipes raw frames to ffmpeg (H.264). A scene with `period` loops seamlessly when the clip is a whole number of
periods. `--input name=value` sets a host input (a ship's position, the sun) and `--sweep name=a..b` moves one across
the frames, which is the quickest way to see what a game's state does to a scene.

## What draws a thing, and what you can bring

Loft is a rasteriser with per-pixel tracers inside bounds, not a path tracer. What draws a thing (`src/draw/`,
`src/soft/`):

- **Rasterised surfaces**: any map of (u, v) on a grid, posed in the vertex stage, painted per pixel, with an ink
  outline (`draw.surface`, `strand`, everything swept).
- **Tracers inside a bound**, each drawn as its bounding box and solved per pixel: sphere-traced distance fields
  (`draw.field`), shell-traced relief on a surface (`draw.relief`), analytic impostors (one sphere, capsule or cone per
  lattice cell, `impostors`) and lists of solids (`drawCopies`).
- **Media integrated in closed form**: clouds and mist (`draw.density`), glows, lightning and rain (`src/soft/`).
- **Raymarched media**: an authored extinction field (`draw.volume`), lit by the sun and lamps up to opaque depth.
- **Many small specks** on your own paths (`draw.points`).
- **Derived layers**: the sky, cast shadows on the water (from capsules along each body's skeleton), contact foam, and
  the style's post passes.

The scene's sun and lamps feed the style's shading. Lamps can cast opaque shadows through depth maps. The sun casts
capsule-chain shadows onto the water and in styles that print a shadow block. Biolume's reflection and papercraft's
crease shading read the picture and its depth in post passes.

For smoke, dust or fog, write a function of world position `p` (read `time` for motion, as everywhere). The density
is extinction per world unit. You can pass a constant, an expression, or a `(p)` function for `albedo` and `emission`.
Loft clamps extinction and emission at zero and albedo to [0, 1]. Emission is radiance per unit extinction, so a
sample with zero density contributes no colour.

```ts check
import { draw, wgsl } from "../src/index.ts";

const fx = wgsl`fn dust(p: vec3f) -> f32 { return max(0.0, noise3(p - vec3(time, 0.0, 0.0))); }`;
export const dust = draw.volume("dust", fx.dust, {
  bounds: { min: [-3, 0, -2], max: [3, 4, 2] },
  steps: 64,
  albedo: [0.98, 0.72, 0.46],
  g: 0.6,
  shadow: { kind: "march", steps: 6 },
});
```

`g` controls the normalized Henyey–Greenstein phase. Zero scatters evenly and positive values brighten backlit
dust. `shadow: { kind: "march", steps: 6 }` integrates extinction toward each light inside this volume. Omitting
`shadow` selects unshadowed scattering. Opacity is independent of the style. Toon's volume look bands visibility,
woodblock prints the arriving light in its inks, and styles can override `Style.volume`.

Order volumes, Gaussian media, and translucent surfaces back to front in `things`. Each volume is a single
composited layer, so spatially interpenetrating transparent objects can show incorrect occlusion. The view march
stops at opaque geometry and the ground. Configured lamp maps can also shadow samples with opaque geometry. The
volume shadow march accounts for its own field only. Volume expressions run inside divergent ray loops, so use
analytic gradients or finite differences for sample-local derivatives.

### Many small specks on your own paths

`draw.points(name, count, speck, options?)` draws `count` specks; `speck(i)` returns each one's world centre,
radius, colour and opacity: a struct with those four fields, or a TypeScript record of them. The index is fixed for
each speck: hash it for stable variation, and write the path (ballistics, drag, a fan, an orbit) as your own equation
of `time`.

```ts check
import { draw, wgsl } from "../src/index.ts";

const fx = wgsl`
struct Speck { at: vec3f, radius: f32, colour: vec3f, opacity: f32 }
fn speck(i: f32) -> Speck {
  let h = hash22(vec2(i, 7.0));
  return Speck(vec3(time * mix(0.5, 4.0, h.x), time * mix(1.0, 2.0, h.y), 0.0), mix(0.002, 0.008, h.y), vec3(0.68, 0.27, 0.12), 0.4);
}`;
export const grains = draw.points("grains", 100_000, fx.speck, { stretch: 0.005 });
```

Radius is in world units. A zero radius or opacity hides a speck, and a speck under a plane or swell is clipped
by its centre. A sub-pixel speck distributes its fading footprint over neighbouring pixels as it moves.
Stretch in seconds elongates the footprint along projected travel. Omit it for specks.

Give `veil` the drawable returned by `draw.density` or `draw.volume` and list that cloud before the specks in
`things`. A speck takes on the colour of the cloud in front of it and is dimmed by the cloud's transmittance up to it,
with the sun and scene lights contributing to its own colour. With 32,768 specks or more the veil is read from a
32³ grid the cloud fills once a frame for the main camera (for a volume, when its functions read no table, input,
param or derivative; otherwise each speck pixel marches the volume, which costs its `steps` per pixel).
Opaque bodies depth-test against the specks. Specks blend by instance order and write no depth.
See `examples/spray_burst.ts` for a boot kicking up warm clay, with puffs and specks using the same flight equation (the burst, drag and fan are the example's own code), and `examples/dust_veil.ts` for grains inside a `draw.volume` cloud, written as WGSL text.

To draw copies of a shape (teeth, spikes, barrels) rather than specks, give `draw.posed` the poses: `posed(n, ...)`
turns and sizes each copy, `points(n, (i) => ({ at, scale }))` only moves and scales it.

Where you can bring your own:

- **Your own shaders.** `draw.shader(name, wgsl\`...\`, { vertices, instances, phase, blend, depthWrite, cull })`
  draws a module's `@vertex fn vs` and `@fragment fn fs`. They read `frame` (`view_proj`, `viewport`, `eye`), `time`,
  `camera_ray(px)` (a `Ray { o, d }` through a pixel) and holes: `${param(...)}` and `${input(...)}` become generated
  accessors, `${sim.state}(i, lane)` and `${baked.sdf}(uv)` read compute outputs. No slot index is ever written.
  A fragment stage that calls `scene_depth(px)` or `scene_position(px)` reads the opaque scene (transparent phases).
  It is unstyled and checks cannot see it; step down to it only for what a `draw.*` primitive cannot do. A library
  author who needs tables, posts or several draws writes a `Drawable` (`src/compile/context.ts`, `src/draw/sky.ts`).

```ts check
import { draw, param, wgsl } from "../src/index.ts";

const glow = param("glow", 0.6, { min: 0, max: 1 });
// A full-screen gradient behind everything: one triangle, no depth write.
export const backdrop = draw.shader("backdrop", wgsl`
struct Out { @builtin(position) clip: vec4f, @location(0) y: f32 }
@vertex fn vs(@builtin(vertex_index) i: u32) -> Out {
  let xy = vec2f(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
  return Out(vec4f(xy, 0.99999, 1.0), xy.y);
}
@fragment fn fs(in: Out) -> @location(0) vec4f {
  return vec4f(mix(vec3f(0.9, 0.8, 0.6), vec3f(0.3, 0.4, 0.7) * ${glow}, in.y * 0.5 + 0.5), 1.0);
}`, { vertices: 3, phase: "background", cull: "none", depthWrite: false });
```

- **A WGSL function inside expressions.** A function in `wgsl` text is a function of expressions; one outside the
  subset is a raw function, emitted as written (GPU only: the CPU twin, and so the checks, refuse it by name and line).

```ts check
import { mul, vec3, wgsl, type F } from "../src/index.ts";

const edge = wgsl`fn aa_stripes(x: f32) -> f32 {
  let s = fract(x * 8.0);
  return smoothstep(0.5 - fwidth(s), 0.5 + fwidth(s), s);
}`;
export const paint = (u: F) => mul(vec3(0.9, 0.7, 0.3), edge.aa_stripes!(u));
```

- **A style, and post passes.** A `Style` is a plain value (`src/style.ts`): write one, or spread an existing one and
  replace a function or append to `post` (a pass is `{ label, wgsl }` defining `fs(in: PostIn)`).
- **A host of your own.** `loft build <scene> --out <dir>` writes the scene as files: `manifest.json` (every WebGPU
  object to create, in WebGPU's own names, and the per-frame recipe), `wgsl/` and `bin/`. A loader executes the
  recipe and decides nothing about the scene (`docs/artefact_format.md`). Below.

### Photographing a scene

```ts check
import { depthOfField, filmGrain, pbr } from "../src/index.ts";

const look = pbr();
export const style = {
  ...look,
  post: [...look.post, depthOfField({ focus: 4, focalLength: 0.16, fStop: 1.8 }), filmGrain()],
};
```

Measure `focus` along the camera's forward axis. Give `focalLength` in the same world units and lower `fStop` for a
wider aperture. The perspective camera's field of view supplies the framing. The orthographic camera models a
telecentric lens. Both use a circular aperture and a blur radius capped at 16 pixels at 720p, scaled with height.
Transparent layers, including fog, rain, sea and the flat ground, use the opaque depth behind each pixel. Where the
opaque depth is clear they use infinity. `filmGrain()` uses frame time as the seed and adds the most grain around
middle grey. Set `amount` for peak-to-peak strength there, default `0.045`.

See `examples/materials/refocus.ts` for a copper-sphere study. For a still with multiple aperture views, render with
`--samples N --aperture r` and omit the depth-of-field post pass so the lens is applied once.

## Writing a compute pass

`compute(name, spec)` runs a `wgsl` module's `@compute` entry with declared outputs and dispatch. Put its return value
in the scene's `compute` list. Each named output in `writes` is a WGSL variable in the pass and a handle on the record:
call it to read the output, in TypeScript, in graph text, or through a hole in raw text.

```ts check
import { compute, param, vec2, wgsl } from "../src/index.ts";

const speed = param("speed", 2);
const stripes = compute("stripes", {
  when: { kind: "frame" },
  writes: { image: { kind: "texture", size: [64, 64], format: "rgba8unorm" } },
  dispatch: [8, 8],
  wgsl: wgsl`
@compute @workgroup_size(8, 8) fn fill(@builtin(global_invocation_id) id: vec3<u32>) {
  let phase = f32(id.x) * 0.25 + time * ${speed};
  textureStore(image, vec2<i32>(id.xy), vec4<f32>(0.5 + 0.5 * sin(phase), 0.3, 0.6, 1));
}`,
});

const paint = stripes.image(vec2(0.5, 0.5)).xyz;
```

`entry` names the kernel only when the module has more than one `@compute` fn (for steps: more than one besides
`when.start`). Holes are the only way Loft values enter the text, and a hole holding another pass's output is that
pass's dependency: the producer runs first, in whatever order the scene lists them.

- **Build** runs on the build GPU and stores the result in the artefact. A 48³ `r32float` volume adds 442,368 bytes.
  [SDF example](../examples/compute_volume.ts): the bake is drawn by `draw.field`, so it gets the scene's camera,
  depth, light and ink. The field reads a texture the CPU cannot, so it asks for `safety`.
- **Load** regenerates data on the loader's GPU once. Use deterministic WGSL with race-free writes. Read an earlier
  output through a hole: `${palette.tones}(0.0, 0.0)`.
- **Frame** runs from zeroed outputs before the pose and draw passes. Only a frame pass reads `time` (seconds),
  params and inputs, by name or hole. Use build or load for data shared by all frames.
  [Palette and frame texture example](../examples/compute_frame.ts)
- **Steps** seeds state 0 with `when.start`, then runs `entry` exactly `when.count` times. The step sees the preceding
  state at `prev_<output>` and writes into a freshly zeroed destination. `loft_dt` is `1 / rate` seconds
  (an `f32` constant); a step has no `time`, its clock is its count.
  Set `keep` to retain every nth state. It defaults to 1 and always keeps the seed and final state. Reads hold the
  latest kept state at or before the requested time, with the first and last states extending outside the simulation.
  [Particle example](../examples/compute_steps.ts), drawn by a `draw.shader` that reads `${sim.state}(f32(i), 0.0)`.

A buffer declares `{ kind: "buffer", entries, lanes }` and is `array<vec4<f32>>` in its pass. Index it as
`entry * lanes + lane`. Read it with `handle(entry, lane)`. A texture declares
`{ kind: "texture", size: [width, height] | [width, height, depth], format }`, where formats are `r32float`,
`rgba8unorm`, `rgba16float`, and `rgba32float`. Read it with `handle(uv)`, normalized coordinates, edge-clamped
bilinear or trilinear. Both read the same way in raw text through a hole, and a stepped output is read at the scene's
`time` everywhere.

A build pass may read build outputs. Load and steps may read build, load, or steps outputs. Frame may read any
earlier producer. CPU tools report `undecided: raw compute "<pass>.<output>"` when a check or solve hold reads an output.
The direct WGSL runs on the selected GPU. Each kernel is responsible for indexing and synchronizing its invocations.

## Drawing a scene elsewhere

```sh
loft build examples/serpent.ts --out out/files/serpent
loft render --artefact out/files/serpent --out out/serpent_from_files.webp --views three
```

The second command draws from the files alone, without the compiler or the scene module, and says so. In a browser,
`createHost(device)` from `src/host/index.ts` loads an artefact onto your own `GPUDevice`. In Rust,
`rust/loft-loader` loads the same directory onto a `wgpu` device a game already owns (`Loft::load`, `set_camera`,
`set_time`, `set_input`, `set_param`, `encode_frame`; its README has the calls). Use `Artefact::warm` during a
loading screen and `WarmedArtefact::load` to allocate the draw resources when entering the level. With a Vulkan
device that enables `PIPELINE_CACHE`, the game can keep the driver cache in a directory it owns.
A game sets inputs by name each frame
(where a target is, when an attack arms) and reads nothing back; what it needs to know about an attack, it computes
from the same numbers it gave. `loft bake <scene> --out f.glb` exports the bodies at one time as a glTF mesh.

## Commands

```sh
loft render   examples/serpent.ts --out out/a.webp --views front,side,top,three --time 1 --size 1280x800
loft frames   examples/serpent.ts --out out/b.webp --times 0,0.5,1,1.5 --view three
loft clip     examples/serpent.ts --out out/c.mp4 --seconds 8 --fps 30
loft dev      examples/serpent.ts --port 5173
loft check    examples/strike.ts
loft profile  examples/serpent.ts --view three --frames 30
loft variants examples/beacon.ts --views three
loft gallery  --scenes "examples/*.ts" --styles toon,woodblock
loft build    examples/serpent.ts --out out/files/serpent
loft serve    --compiles 2      # requests on stdin, answers on stdout, JSON lines (docs/handoff_training.md)
loft bake     examples/serpent.ts --out out/serpent.glb
loft show     examples/serpent.ts --thing serpent   # the WGSL the GPU compiles, your text's lines marked
```

`loft help` prints every flag; `loft context` prints the whole library for a reader in one go.

## Where to go next

- A spread of examples, short to long: `examples/shell.ts`, `examples/brig.ts`, `examples/pearls.ts`,
  `examples/coral.ts`, `examples/storm.ts`, `examples/serpent.ts`, `examples/maelstrom.ts`, `examples/scenes/`.
  Read them for mechanisms; do not start from their proportions (`loft context range` says why).
- `docs/components.md`: every export, with what its comment says it cannot do.
- `docs/animation_principles.md` (motion), `docs/lighting.md` (the print's lightings), `docs/variants.md` (designs),
  `docs/artefact_format.md` (the files). `DESIGN.md` is the record of decisions, not a tutorial.
