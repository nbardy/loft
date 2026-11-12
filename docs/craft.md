<!--
The craft guide: what we learned about making 3D pictures and motion that read, practised on Loft. It is the first
half of `loft context`; the API reference (`loft api`) is the second. Every snippet fenced `ts run`, `ts check` or
`sh` is run by `bun tools/check_snippets.ts`. Kept near 4,000 words so that, with the compact API, `loft context`
can be read whole.
-->

# Craft: making things that read, on Loft

This is general help for making 3D pictures and motion, written for Loft as the surface you practise it on. It assumes
no reference pictures and no one to ask: you set the target yourself, you make the range yourself, and your own
renders are the only eye you have. Each point is the principle, why it works, and the Loft function or command that
does it.

## The surface

Loft describes animated 3D things as TypeScript expressions: `add(a, b)`, `sin(time)`, `vec3(x, y, z)` build a graph
that runs on the GPU to draw and on the CPU to check. A swept body is a coordinate system (m from 0 at its root to 1 at
its tip, v round it), so paint and attached parts are functions of those coordinates and move with it. `time` is an
argument: motion is a closed form, any frame draws alone. A scene is a module whose default export is a `scene`:

```ts run
// examples/reed.ts
import { add, mul, scene, sea, sin, strand, time, toon, vec3, type F } from "../src/index.ts";

const reed = strand({
  name: "reed",
  spine: (m: F) => vec3(mul(mul(m, m), mul(sin(time), 0.6)), mul(m, 4), 0), // a point for each m; the tip sways
  girth: (m: F) => add(0.25, mul(m, -0.2)),
  tips: ["open", "point"],
  paint: [0.85, 0.55, 0.2],
});

export default scene({
  sea: sea({ swell: [{ direction: [1, 0], length: 8, height: 0.1, speed: 1 }] }),
  things: [reed],
  style: toon(),
  sun: [-0.4, 0.8, 0.45],
  cameras: {
    whole: { kind: "perspective", eye: [6, 4, 8], target: [0, 2, 0], fovY: 0.6 },
    tip: { kind: "perspective", eye: [2.5, 4.5, 3], target: [0, 3.6, 0], fovY: 0.5 },
  },
});
```

```sh
loft render examples/reed.ts --out out/reed.webp --views whole,tip --size 640x400
loft frames examples/reed.ts --out out/reed_t.webp --times 0,0.8,1.6,2.4 --view whole --size 480x300
```

`loft` is `bun src/cli/main.ts`. `render` puts one picture per camera on one sheet; `frames` one camera at several
times; `dev` is a live page with orbit and a slider per `param`.

## 1. Set the target in words before building

Without a written target, the first thing that renders becomes the target, and it is always the nearest example with
new numbers. Write an art direction in the file's top comment before any code:

- the **silhouette** in one sentence: what it is from the main camera with paint switched off;
- the **three largest forms** and their proportions to each other ("a body twice as long as it is tall, a head a
  third of the body, a tail longer than both");
- **what makes it unlike** the nearest example in the repository;
- the **palette**: two or three inks and one accent, and the one place the accent marks;
- the two or three **beats of its motion**, with the wind-up's length against the action's;
- **predictions** a render can be scored against: a part's size in pixels from a named camera, what is in front of
  what, how long a beat lasts.

A reference picture, when a person supplies one, helps fill this in; it does not replace it.

## 2. Range in shape: change the construction, not the parameter

Tuning a component's numbers explores one family. Every family has a law, and the law, not the numbers, is what a
viewer recognises: ten spiral tubes with different curls are one shape. Range comes from changing what the shape is
made of.

**Make three to five far-apart candidates before refining any**, each from a different construction, side by side
in one scene, judged on one sheet. Write the weakest thing about each in one sentence. Pick one and push it further
than feels safe. Here, three candidates for "a tall thing standing in water", each a different law:

```ts run
import { add, cos, draw, lathe, mul, scene, sea, sin, spacing, spiralLine, strand, toon, vec2, vec3, type F } from "../src/index.ts";

// A: swept. A spiral's centre line, but a section that flattens to a blade toward the tip.
const line = spiralLine({ length: 5, curl: 1.6, taper: 0.4, girth: 0.35, out: [0, 1, 0], toward: [1, 0, 0] });
const blade = strand({
  name: "blade", spine: (m: F) => add(line.spine(m), vec3(-4, -0.3, 0)), up: line.up,
  girth: (m: F) => mul(line.girth(m), add(1, mul(m, 1.5))),
  section: { aspect: (m: F) => add(1, mul(m, -0.85)) },
  tips: ["open", "point"], paint: [0.25, 0.5, 0.45],
});

// B: turned. A profile (radius, height) spun round the axis: a stack of swelling rings.
const rings = lathe((u: F) => vec2(add(0.5, mul(add(sin(mul(u, 18)), 1), 0.25)), mul(u, 4)), ["free", "cap"]);

// C: a chain of copies, each turned a little more: a body made of repetition, not of one surface.
const disc = (k: number) => strand({
  name: `disc${k}`, spine: (m: F) => vec3(add(4, mul(cos(k * 0.7), 0.12 * k)), add(0.4 * k, mul(m, 0.2)), mul(sin(k * 0.7), 0.12 * k)),
  girth: 1.1 - k * 0.08, section: { aspect: 0.25 }, paint: [0.85, 0.5 + k * 0.03, 0.3],
});

export default scene({
  sea: sea({ swell: [] }),
  things: [blade, draw.surface("rings", rings, { paint: () => vec3(0.9, 0.85, 0.7), grid: { u: spacing.even(120), v: spacing.even(32) }, sides: "one", outline: "hull" }), ...Array.from({ length: 10 }, (_, k) => disc(k))],
  style: toon(),
  sun: [-0.4, 0.8, 0.45],
  cameras: { sheet: { kind: "perspective", eye: [0, 4, 15], target: [0, 2, 0], fovY: 0.65 } },
});
```

Ask of every candidate which of these it changed. If none, it is a retune, not a candidate:

- **the spine**: `through` points, a `wave`, a spiral, or a curve whose heading you write as a function of m;
- **the section**: `section.aspect` and `squareness` as functions of m, a `twist` that varies, a flat ribbon;
- **the topology**: a tube, an open sheet (`sheet`, `fan`, `crest`), a closed turned form (`lathe`, `shell`), a folded
  or recursive solid (`draw.field` with `src/lib/fold.ts`), a cloud of copies (`impostors`, `draw.copies`);
- **the construction**: swept, turned, extruded, lofted through curves (`loft`), traced from a distance function,
  grown by repetition (`src/lib/similar.ts`, `src/lib/grow.ts`), relief on a host surface (`draw.relief`).

**Read a component's limits, then go one step down.** `loft api spiralTube` is the model. Its comment states its law
(curl tied to taper), what the law cannot draw (an S-bend, a swelling club, a helix of constant pitch, any section
but round) and the relatives one step down: keep the centre line with `spiralLine` and give `strand` your own girth
or section (candidate A above); write the heading along the line yourself: linear in m is an arc, and with a drift
along `up` a helix; growing as m squared is a clothoid, the fern head and the scorpion's tail (no closed form: sample it
with `tabulate`, or lay it through points with `through`); an Archimedean spiral is the rope coil. Every component is
a short composition over the level below, so leaving it is never a rewrite.

**Do not copy the nearest example's proportions.** Examples are written to show mechanisms. Their numbers (a girth,
three swells, a camera at a certain place) are one author's taste on one day. Read an example for how a part is
attached or how a motion is clocked, then close it and take every proportion from your own direction.

**Know the pull, and the ways out.** The strongest pull on what you write is the example read last and the named
presets (a style's defaults, `LOOKS`, a design's `base`). Say it to yourself: "this is the serpent's spine with new
numbers". Ways out: start from a construction no example uses; start from the written direction, not a file; write
the shape one level below the component you were about to call; make the first candidate the one you would not have.

## 3. Range in style: compose parts, write a part

`--style` swaps a whole look for free (`toon`, `woodblock`, `biolume`, `papercraft`), and `--light` picks one of a
style's lightings. Glance at them, but four presets are four looks, not range. Range in style comes from composing:
paint written in the body's own coordinates; which parts carry ink; the sky and the water as part of the picture; and
a style function of your own where no preset says what your direction says. A `Style` is a plain value, so a new look
is often one function replaced:

```ts run
import { add, dot, fract, max, mix, mul, palette, scene, sea, smoothstep, strand, toon, vec3, type F, type Style } from "../src/index.ts";

const base = toon({ bands: 2, ink: [0.1, 0.05, 0.05] });
// Engraved: light is drawn as lines whose width follows N.L, written on the body's anchor so they ride it.
const engraved: Style = {
  ...base,
  name: "engraved",
  shade: (c) => {
    const lit = max(dot(c.normal, c.sun), 0);
    const line = smoothstep(0.35, 0.65, fract(mul(add(c.anchor.y, mul(c.anchor.x, 0.3)), 9)));
    return mix(mul(c.base, 0.35), c.base, smoothstep(0, 1, add(mul(lit, 1.4), mul(line, -0.6))));
  },
};

const fish = strand({
  name: "fish",
  spine: (m: F) => vec3(add(mul(m, -5), 2.5), 1.2, 0),
  girth: (m: F) => add(0.2, mul(mul(m, add(1, mul(m, -1))), 3)),
  section: { aspect: 1.4 },
  tips: ["round", "point"],
  // Paint in body coordinates: a dark back, a pale belly, and saddle bands that only the back carries.
  paint: (m, v) => palette.bands(m, v, palette.countershade(v, { back: [0.2, 0.3, 0.5], belly: [0.95, 0.9, 0.8] }), { colour: [0.9, 0.4, 0.2], count: 5, width: 0.4, from: 0.15, to: 0.85 }),
});

export default scene({
  sea: sea({ swell: [] }),
  things: [fish],
  style: engraved,
  sun: [-0.4, 0.8, 0.45],
  cameras: { side: { kind: "perspective", eye: [0, 2.5, 9], target: [0, 1.2, 0], fovY: 0.6 } },
});
```

A post pass (`{ label, wgsl }` appended to `post`) changes the whole picture: grain, a palette quantised to a few inks,
an edge from depth. `src/style/biolume.ts` (about 110 lines) is the shortest complete style to read.

## 4. Shapes that read

- **Large forms first.** Three big shapes with clear proportions read; thirty small ones are texture. Build the largest
  forms with flat paint and judge them before adding any detail.
- **Silhouette before detail.** Judge with paint flat from the main camera. Parts merge where one stands in front of
  another: a limb before a face, two eyes that overlap from above, a wall across the thing it frames.
- **Proportion and carriage are identity.** Two subjects with the same parts and different proportions (a head a
  quarter of the body against a head a half) and a different carriage (how high the body rides, where it leans, what
  it holds still) are different characters. Change these before adding parts.
- **Detail in clusters.** Growth, studs and marks in fistfuls at a few places, crowded and large at the base and
  thinning away, read as made. The same detail spread evenly reads as noise: an even hatch, equal stripes winding into
  a pinwheel, dots strewn round a rim.
- **Paint in the body's own coordinates** (`palette.countershade`, `palette.bands`, `palette.spot`, `dorsalness`;
  `tile` with a fold for patterns that repeat). Paint written on m and v stays on the body as it moves and scales with
  it. Space bands by arc length round the section, or they bunch on one side and miss the part every camera sees.
- **Contrast against what is behind.** Dark eyes on a dark body are two dots. Each feature's contrast with its
  surround, at the size it is seen, decides whether it exists.
- **The camera decides.** A face aimed squarely at one camera shows its underside to a lower one. From a camera that
  looks down the light, the shadow side of a ball is 4% of its disc (it is a quarter from the side), so a picture can
  lose its modelling by where it is seen from.
- **Cost is in the paint.** A paint function runs for every pixel and computes every branch: six patterns in one
  paint cost six. Move what does not change per frame into a table, and profile before and after.

## 5. Motion that reads

Each of these is a function (`src/geom/lively.ts`, `src/curve.ts`, `src/geom/`); `docs/animation_principles.md` has the
twelve principles in full.

- **Three beats.** An action is wind-up, action, settle: `beats(since, { anticipate, act, settle })`. The wind-up is
  several times longer than the action; a long wind-up before a short act is what makes a hit read as a hit, and a
  viewer can only read what lasts long enough to be seen.
- **The wrong way first, past the mark after.** `anticipated(u)` dips before it drives; `overshot(u)` runs past and
  comes back; `response(t, { f, zeta, r })` is a spring, with `r < 0` for a flinch.
- **Loose parts lag and swing on.** Read a tail or a banner on `dragged(time, m, lag)`: the same formula, later the
  further along. Add `swing(b.after, amount)` when the body stops.
- **Nothing holds perfectly still; no two parts move in step.** `alive(time, amount, seed)` on every resting angle with
  its own seed; `uneven(side)` so left and right are not twins.

```ts run
import { add, alive, anticipated, beats, dragged, mod, mul, scene, sea, sin, strand, swing, time, toon, vec3, type F } from "../src/index.ts";

const b = beats(mod(time, 4), { anticipate: 0.8, act: 0.2, settle: 1.5 }); // every 4 s
const lean = add(add(mul(anticipated(b.anticipate), -0.3), b.act), add(mul(b.settle, -1), swing(b.after, 0.4)));
const stalk = strand({
  name: "stalk",
  spine: (m: F) => vec3(mul(mul(m, m), add(mul(add(lean, alive(time, 0.05, 1)), 3), mul(sin(dragged(time, m, 0.4)), 0.15))), mul(m, 4), 0),
  girth: (m: F) => add(0.3, mul(m, -0.22)),
  tips: ["open", "round"],
  paint: [0.4, 0.55, 0.85],
});

export default scene({
  sea: sea({ swell: [] }), things: [stalk], style: toon(), sun: [-0.4, 0.8, 0.45], period: 4,
  cameras: { side: { kind: "perspective", eye: [0, 3, 12], target: [0, 2, 0], fovY: 0.6 } },
});
```

- **A pose struck and held.** `struck(clock, { from, snap, hold, release })` snaps into a pose in a few frames, holds it
  exactly, then lets it go. Beside parts that flow, stillness is character. In a `score` of named expressions (`cast`),
  lay the flowing layers first and the struck poses after, or the flow leaks through the hold; `holdsStill` checks it.
- **Planted feet and a body that carries.** A heavy body does not move its feet to idle; it moves over them. `footing`
  fixes each foot and steps only at named events, `carriage` rises, sinks, sways and rolls the body, `limb` solves each
  leg between them, so the knees read for free. `gait` gives foot targets when it travels.
- **Arcs and weight.** `arc(a, b, u, lift)` instead of a straight line; `squash(stretchOf(speed))` in flight and
  `impact(after)` at the stop, volume kept. A thing that changes hands keeps its velocity at every hand-off (grip,
  stroke, flight, water): `thrown`.
- **Temperament is timing.** A nervous creature is still, then somewhere else (`dart`, `tremor`); a slow one reacts
  late and lets what hangs from it move (`pendulum`). Stiffness can be a channel: rage holds a hanging thing rigid,
  grief lets it swing.
- **A group in unison, with contrast.** `routine` is one score for the whole group, evaluated per member, so unison is
  a property of the data, not of performers trying to agree. Keys land on counts and hold; canon is a time shift per
  member, mirror a sign, solos the contrast. `inSync` measures it from each member's own pose.
- **An action with its warning.** `attack` takes a hit shape, an arming time and a warning; `telegraph` draws the
  warning on the water before the motion; `warned`, `landsInside` and `stopsAt` hold it to that. Ask what the world
  feels: a slam the water does not answer (`ring`), or a grab that sets nothing down, reads as unfinished.
- **Sound from the same score.** Cues taken from a routine's keys land on its counts to the sample
  (`examples/bosses/crab_song.ts`; example code, not a library part).
- **Push it.** When it works, `exaggerated(x, rest, 1.3 to 2)` every departure from rest, and look again.

## 6. Predict before rendering

Every expression also runs on the CPU, so the picture can be worked out before it is drawn: where a part is at any
time, how big it is in pixels, what covers what. A prediction written first turns a render into a measurement; a miss
then says the idea of the problem was wrong, which is worth more than a picture that happens to look right.

```ts check
import { compileJs, mul, sin, vec3, variable } from "../src/index.ts";
import { cameraMatrices, project } from "../src/host/index.ts";

// The reed's tip (m = 1) as an expression of time, evaluated on the CPU and projected through the camera.
const t = variable("t", "f32");
const tip = vec3(mul(sin(t), 0.6), 4, 0);
const at = compileJs([tip]);
const camera = cameraMatrices({ kind: "perspective", eye: [6, 4, 8], target: [0, 2, 0], fovY: 0.6 }, 1280 / 800);
for (const s of [0, 0.8, 1.6]) {
  const p = at({ vars: { t: s }, inputs: {}, params: {} })[0] as number[];
  const px = project(camera, [p[0]!, p[1]!, p[2]!], 1280, 800);
  console.log(`t=${s}: tip at x ${px.x.toFixed(0)} px, y ${px.y.toFixed(0)} px`);
}
```

Make a prediction permanent as a check: `minScreenSize` (a part is at least so many pixels), `spansOnScreen`,
`staysInView` (a face is not covered), `expressionsRead` (two expressions are told apart), and an action's checks
above. Export them as `checks` beside the scene; `loft check` runs them and exits 1 on a failure.

## 7. Look at your own render

Look before you report, and look the way the picture will be seen:

- **from the camera it will be seen from**, first, and **small**, at the size it will be seen;
- **as a silhouette**: flat paint, or squint. Does it read in one second, and as what?
- **across the motion**: `frames` at the wind-up, the action and the settle, not only at rest;
- then ask: is any proportion copied from an example? What is the dullest region of the frame? What would a stranger
  call it? Write the answers down before changing anything, and change the largest thing first.

```sh
loft render examples/reed.ts --out out/look.webp --views whole,tip --size 480x300
loft frames examples/reed.ts --out out/across.webp --times 0,0.6,0.9,1.4,2 --view whole --size 320x200
loft frames examples/stormwing.ts --out out/sweep.webp --times 1 --count 6 --sweep strike=0..1
loft variants examples/bosses/reefback_titan.ts --views far
```

A picture is judged from what reads on screen, not from its code. Give every scene at least two cameras: one that shows
the subject whole with room round it, and one on its face or focus, the part that carries the character or the action.
Make motion readable in a handful of frames: a wind-up long enough to be caught, a pose held long enough to be seen, a
loop that closes (`period`). A detail that no camera shows is not in the picture.

## 8. The loop, and its cost

1. Write the direction and its predictions. 2. Make the candidates; render them once, small, on one sheet. 3. Score
against the predictions, then by eye (section 7). 4. Choose, push, render again. 5. Make the predictions checks.

Cost: `loft profile <scene> --view <camera>` prints GPU time per draw and a "normal frame" line; quote that one. The
GPU timer ticks in 0.065 ms, and a GPU can sit in two clock states about three times apart, so compare before and
after alternately within one state. A whole creature costs about 1 to 4 ms at 1280x800; the sea, outlines and post
passes scale with resolution, and paint is usually the expensive part of a body.

```sh
loft check examples/strike.ts
loft profile examples/serpent.ts --view three --frames 30
```
