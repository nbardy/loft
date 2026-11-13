<!--
The written part of `loft context` (src/cli/context.ts): the craft, condensed, then the generated API map where the
marker below stands, then failures and where to read more. docs/craft.md is the long form for people; keep this one
short, since it is read in full before every scene. `bun tools/check_snippets.ts` runs its code and checks that every
name in its inline code is exported.
-->
# Loft

Loft describes animated 3D things as TypeScript expressions compiled to WGSL. A scene file imports from
`../src/index.ts` and default-exports `scene({ ground, things, style, sun, cameras })`; `ground` is `plane`,
`nothing` or `sea`. Expressions (`add`, `sin(time)`, `vec3`) run on the GPU to draw and on the CPU to check. A swept body
has coordinates m (0 at the root, 1 at the tip) and v (round it); paint and attached parts are functions of them.
`time` is an argument, so any frame draws alone. One sun; no reflections, no simulation (everything is a closed form
of time and inputs), no meshes in. `loft` is `bun src/cli/main.ts`.

```ts run
// examples/reed.ts
import { add, mul, scene, plane, sin, strand, time, toon, vec3, type F } from "../src/index.ts";

const reed = strand({
  name: "reed",
  spine: (m: F) => vec3(mul(mul(m, m), mul(sin(time), 0.6)), mul(m, 4), 0), // a point for each m; the tip sways
  girth: (m: F) => add(0.25, mul(m, -0.2)),
  tips: ["open", "point"],
  paint: [0.85, 0.55, 0.2],
});

export default scene({
  ground: plane,
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

# Craft

You set the target, you make the range, and your own renders are your only eye. The long form, with every sample:
docs/craft.md.

## RANGE IN SHAPE: change the construction, not the number

A family of shapes is one law, and a viewer recognises the law, not its numbers: ten spiral tubes with different
curls are one shape. Range comes from changing what a shape is made of.

- **Make three to five far-apart candidates before refining any**, each a different construction, small, side by side
  on one sheet. Write the weakest thing about each; pick one and push it further than feels safe.
- **A candidate changes at least one of these, or it is a retune:**
  - the spine: `through` points, a `wave`, `spiralLine`, or a heading you write as a function of m (`tabulate`);
  - the section: `aspect` and `squareness` as functions of m, a `twist` that varies, a flat ribbon;
  - the topology: a tube (`strand`), an open sheet (`sheet`, `fan`, `sheetOn`), a turned form (`lathe`), a folded
    solid (`draw.field` with `fold3`), a cloud of copies (`impostors`, `draw.copies`);
  - the construction: swept, turned, extruded (`extrude`), lofted through curves (`loft`), traced from a distance
    (`draw.field`), grown by repetition (`grow`, `tree`), relief on a host surface (`draw.relief`).
- **Read a component's limits, then go one level down.** `loft api spiralTube` gives its law, what the law cannot draw
  and its relatives. Every component is a short composition over the level below; keep what it does well and write
  the rest yourself:

```ts check
import { add, mul, spiralLine, strand, type F } from "../src/index.ts";

// spiralTube's centre line with a section of your own: round at the root, a flat blade at the tip.
const line = spiralLine({ length: 5, curl: 1.6, taper: 0.4, girth: 0.35, out: [0, 1, 0], toward: [1, 0, 0] });
export const blade = strand({
  name: "blade", spine: line.spine, up: line.up, girth: (m: F) => mul(line.girth(m), add(1, mul(m, 1.5))),
  section: { aspect: (m: F) => add(1, mul(m, -0.85)) }, tips: ["open", "point"], paint: [0.25, 0.5, 0.45],
});
```

- **Examples and presets pull.** An example's numbers are one author's taste on one day: read it for how a part is
  attached or a motion clocked, then take every proportion from your target. Start from a construction no example
  uses, and make first the candidate you would not have made.

## Range in style: compose parts, write a part

The presets (`toon`, `woodblock`, `biolume`, `papercraft`; `--style`, `--light`) are four looks, not range. Compose:
paint in the body's coordinates, choose which parts carry ink, treat sky and ground as part of the picture, and write a
function where no preset says what your target says. A `Style` is a plain value: spread one and replace its `shade`
(given the point's normal, the sun, its base colour and its anchor). A `post` pass (`label`, `wgsl`) changes the whole
frame: grain, a few inks, an edge from depth. The shortest style to read is src/style/biolume.ts.

## Set the target in words first

Without a written target the first render becomes the target, and it is the nearest example with new numbers. There
are no reference pictures. In the file's top comment write: the silhouette in one sentence (main camera, paint off);
the three largest forms and their proportions; what makes it unlike the nearest example; the palette (two or three
inks, one accent and where it goes); the two or three beats of its motion, wind-up against action; and predictions a
render can be scored against (a part's size in pixels from a named camera, what is in front of what, a beat's length).

## Shapes that read

- Large forms first: three big forms in clear proportion read, thirty small ones are texture. Judge them in flat paint.
- Silhouette before detail, from the main camera: parts merge where one stands in front of another.
- Proportion and carriage are identity: change a head's share of the body, how high it rides, where it leans, before
  adding parts.
- Detail in clusters, crowded and large at a few places and thinning away; spread evenly it reads as noise.
- Paint in the body's coordinates (`palette.countershade`, `palette.bands`, `palette.spot`, `dorsalness`, `tile`): it
  stays on the body as it moves. Space bands by arc length round the section.
- Contrast with what is behind decides whether a feature exists at the size it is seen.
- Every scene has two cameras: the whole with room round it, and the focus. A detail no camera shows is not there.

## Motion that reads

- Three beats: `beats(since, { anticipate, act, settle })`, the wind-up several times longer than the act.
- The wrong way first, past the mark after: `anticipated`, `overshot`, `response` (a spring; negative `r` flinches).
- Loose parts lag and swing on: `dragged(time, m, lag)`, then `swing` when the body stops.
- Nothing holds still, no two parts move in step: `alive(time, amount, seed)` on each resting angle, `uneven`.
- A pose struck and held: `struck(clock, { from, snap, hold, release })`; in a `score`, flowing layers first, struck
  poses after (`holdsStill` checks it).
- Weight: `arc` not lines; `squash(stretchOf(speed))` in flight, `impact` at the stop; feet fixed by `footing`, the
  body moved by `carriage`, legs solved by `chain`; `gait` to travel. A thrown thing keeps its velocity (`thrown`).
- Temperament is timing: nervous is still, then elsewhere (`dart`, `tremor`); slow reacts late (`pendulum`).
- A group is one `routine` evaluated per member: unison is in the data, canon a time shift, a solo the contrast.
- An action with its warning: `attack`, `telegraph`, checked by `warned`, `landsInside`, `stopsAt`.
- When it works, push it: `exaggerated(x, rest, by)` with by 1.3 to 2, and look again.

```ts check
import { add, anticipated, beats, mod, mul, swing, time } from "../src/index.ts";

// Every 4 s: 0.8 s leaning the wrong way, a 0.2 s act, a 1.5 s settle that swings past rest.
const b = beats(mod(time, 4), { anticipate: 0.8, act: 0.2, settle: 1.5 });
export const lean = add(add(mul(anticipated(b.anticipate), -0.3), b.act), add(mul(b.settle, -1), swing(b.after, 0.4)));
```

## Predict, then look

- Predict before rendering: expressions run on the CPU (`compileJs`, `evaluate`), so where a part is and how many
  pixels it covers can be worked out first, and the render becomes a measurement. Keep a prediction as an exported
  `checks` list (`minScreenSize`, `spansOnScreen`, `staysInView`, `expressionsRead`); `loft check` runs it.
- Look at your own render from the camera it will be seen from, small; as a silhouette; across the motion (`loft
  frames` at wind-up, act, settle). Ask: is a proportion copied from an example, what is the dullest region, what would
  a stranger call it? Write the answers, then change the largest thing first.
- Cost: `loft profile <scene> --view <camera>`, quote its "normal frame" line; a whole creature is 1 to 4 ms at
  1280x800. Paint runs every branch for every pixel: move what does not change into a table.

<!-- the API map is printed here -->

# When a scene fails

`render`, `frames`, `clip` and `check` end with `loft: <kind>: <message>` and an exit code:
0 drew, or every check passed; 1 internal (a bug in Loft; report it); 2 usage (fix the command, not the scene);
3 load (syntax, an unknown import, a throw at top level); 4 scene (a constructor refused it); 5 check (the named
checks failed); 6 environment (GPU, `cwebp`, `ffmpeg`: not the scene's fault).
A refusal reads `given ...; expected ... Fix: ...` and ends `(at file:line)`. The likeliest in a first scene:

- a shape put in `things` undrawn: draw it, `draw.surface("name", shape, { paint, grid, sides, outline })`;
- a surface or paint that returns a number: return `vec3(...)`;
- arithmetic on vectors of two sizes (`add` of a vec3 and a vec2): make them one size;
- `fovY` in degrees: it is radians;
- two things with one name: names key shaders, so rename one.

Every refusal, with its fix: docs/errors.md.

# More

- `loft api`: the whole reference, every export with its comment.
- `loft api spiralTube` (any name): one entry whole, with what it cannot do and its relatives.
- docs/craft.md: the craft in full, with its samples. docs/guide.md: the tutorial.
