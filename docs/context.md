<!--
The written part of `loft context` (src/cli/context.ts): conventions and the smallest scene, WGSL text (the names in
scope are generated where its marker stands), the craft, condensed, then the generated API map where its marker
stands, then failures and where to read more. docs/craft.md is the long form for people; keep this one short, since it
is read in full before every scene: tests/context.test.ts holds the whole print to its word budget. `bun tools/check_snippets.ts` runs its code and checks that every
name in its inline code is exported.
-->
# Loft

A scene imports from `../src/index.ts` and default-exports `scene({ ground, things, style, sun, cameras })`; `ground`
is `plane` (y = 0), `nothing` or `sea`. y is up. Write the maths of a function (a spine, a girth, paint, a field) as
WGSL in `wgsl` text and the structure (which things, where, cameras, checks) in TypeScript: both build one expression
graph that draws on the GPU and evaluates on the CPU, so checks read what is drawn. `time` is an argument, in seconds,
so any frame draws alone; `period` says when every motion repeats. A swept body's m runs 0 (root) to 1 (tip) and v 0
to 1 round it; a surface's u and v run 0 to 1; paint is a function of them. Give every scene two cameras: the whole
with room round it, and the focus. `strand`, `spiralTube`, `sheetOn`, `ropes` and `impostors` return things: put them
in `things`. `coil`, `fan`, `sheet`, `lathe`, `loft` and `extrude` return a bare surface: draw it,
`draw.surface(name, surface, { paint, grid })`.
Light is the sun (`sun: { toward, amount }` dims or colours it; amount 0 is a moonless night) and up to seven `lights`,
each a function of a world point returning `{ toward, amount, distance?, size? }`, such as `lamp(at, { colour, reach })`;
a lamp casts a shadow given shadow: true, a glow lights nothing by itself. Paint may return a struct: `base` (the colour)
and words of your own, of which the styles know gloss (0 to 1), metal and emits (linear light), and `pbr` also rough
and normal (a bump: `gradient` of a height); a style notes the words it does not read. Its `scatter` answers light
from a direction, its `shade` changes the finished colour.
Your own triangles draw with `draw.mesh`, a solid you can intersect with `draw.traced`. `pass` is another camera's
picture (a mirror) for an expression to read. `--samples 64 --shutter 0.05 --aperture 0.1` makes a still with motion
blur and depth of field. Meshes come out (the bake command writes glTF), none go in. `loft` is `bun src/cli/main.ts`.

```ts run
// examples/reed.ts
import { plane, scene, strand, toon, wgsl } from "../src/index.ts";

const reed = wgsl`
fn spine(m: f32) -> vec3f { return vec3(m * m * sin(time) * 0.6, m * 4.0, 0.0); }  // a point per m; the tip sways
fn girth(m: f32) -> f32 { return 0.25 - 0.2 * m; }`;

export default scene({
  ground: plane,
  things: [strand({ name: "reed", spine: reed.spine, girth: reed.girth, tips: ["open", "point"], paint: [0.85, 0.55, 0.2] })],
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
loft frames examples/reed.ts --out out/reed_t.webp --times 0,0.8,1.6 --view whole --size 480x300
```

# WGSL text

`wgsl` returns a function per fn, taking expressions or numbers; it fills any slot a TypeScript function fills. A slot
may pass more arguments than a fn declares (paint passes u, v, at); a returned struct fills a record slot by field
name. In scope: the module's fns, structs and consts, `time`, the names below, and holes: `${x}` brings in a `param`,
an `input`, a solved curve, a TypeScript function (called in the text) or a number. The subset that checks, `solve`
and `gradient` read: f32, vec2f..vec4f, bool, mat2x2f, mat3x3f, constant arrays, structs; let, var, assignment,
swizzle writes, if/else, for over a counter with break and continue, early return. As in WGSL, round ties to even, %
truncates (mod floors) and 1 is an integer (write 1.0). A fn using anything else (integers, bit operations, textures,
derivatives, while) is raw: it renders, and checks refuse it by name and line. Lower down, a module draws
(`draw.shader`: vs and fs entries) or computes (`compute`), reading camera_ray(px), frame and holes.
`loft show examples/reed.ts --thing reed` prints the WGSL the GPU runs, your lines marked. Call form (`add`, `mul`,
`vec3`) is for what TypeScript does: mapping over parts, helpers.
<!-- the names in scope in WGSL text are printed here -->

# Craft (long form: docs/craft.md)

**Range in shape: change the construction, not the number.** A viewer recognises a shape's law, not its numbers: ten
spiral tubes with different curls are one shape. Make three to five far-apart candidates before refining any, small on
one sheet; write the weakest thing about each, pick one, push it further than feels safe. Each must change at least one
of: the spine (`through`, `wave`, `spiralLine`, a heading written in m with `tabulate`); the section (`aspect`,
`squareness`, `twist` as functions of m); the topology (tube `strand`, sheet `sheet` or `fan`, turned `lathe`, folded
`draw.field` with `fold3`, copies `impostors`); the construction (`extrude`, `loft`, `grow`, `tree`, relief
`draw.relief`). Otherwise it is a retune. Read a component's limits (`loft api spiralTube`: its law, what it cannot
draw, its relatives), then go one level down and write the rest. Examples and presets pull: take how a part attaches
from them, never a proportion; make first the candidate you would not have made.

**Set the target in words first;** there are no reference pictures, and without a target the first render becomes it.
In the file's top comment: the silhouette in one sentence, the three largest forms and their proportions, what makes
it unlike the nearest example, two or three inks and one accent, the beats of its motion, and predictions a render can
be scored against (a part's pixels from a named camera, what is in front of what, a beat's length).

**Range in style: compose parts, write a part.** Presets (`toon`, `woodblock`, `biolume`, `papercraft`) are four looks,
not range. Paint in body coordinates (`palette.countershade`, `palette.bands`, `palette.spot`, `tile`), choose which
parts carry ink, make sky and ground part of the picture. A `Style` is a plain value: spread one and replace `shade`;
a `post` pass changes the whole frame. Say what a thing is with a word in its paint or a `scatter` before you wrap the
style; a `shade` that ignores the style's colour drops out of every restyle. Light a night by what is in it: a dim sun, and a `lamp`
at the same point as the glow that shows it; check the subject is lit (`litBy`).

**Shapes that read.** Large forms first, judged in flat paint; silhouette before detail; proportion and carriage are
identity; detail in clusters, not spread evenly; contrast with what is behind decides whether a feature exists.

**Motion that reads.** Three beats, wind-up several times the act (`beats`); the wrong way first and past the mark
after (`anticipated`, `overshot`, `response`); loose parts lag (`dragged`, `swing`); nothing still or in step (`alive`,
`uneven`); a pose struck and held (`struck`); arcs and weight (`arc`, `squash`, `impact`, `footing`, `carriage`,
`gait`); temperament is timing (`dart`, `pendulum`); a warning before an action (`attack`, `telegraph`); then push it
(`exaggerated`). State only for what truly steps: a kick is a `jolt`, a trail is the thing at earlier times; `recur`
a grid, a flock, contact.

**Predict, then look.** Expressions run on the CPU (`compileJs`), so predict sizes and positions first and keep them as
exported `checks` (`minScreenSize`, `staysInView`). Look at your render small, from its camera, as a silhouette and
across the motion (frames, above); change the largest thing first.

<!-- the API map is printed here -->

# Wrong without an error

A one-sided surface wound inside out shows its far wall or nothing (try `sides: "two"`); a camera aimed past the
subject shows sky; a subject far too small or large for its camera; a `period` that is not every motion's own jumps at the loop.
`minScreenSize` and `staysInView` catch two. Exit 6 is the environment, not the scene.

# More

`loft api` (everything); `loft api spiralTube` (one entry whole, with its limits and relatives); docs/craft.md;
docs/guide.md.
