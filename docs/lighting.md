# Lighting the print: recipes

The woodblock style (`src/style/woodblock.ts`) lights its bodies by a recipe: a record of shading treatments, each on or
off with its own options. With no recipe the style is the print as it was (the rule's own two tones, split at
N.L = 0), and every picture made before recipes is unchanged. Code: `src/style/light.ts` (the treatments, the named
recipes), `src/draw/light.ts` (shadows that fall from one thing on another). Why it is built this way, what was
predicted and what came out: `docs/art_direction/DIRECTION.md`, "Lighting and shading".

## Picking one

```ts
import { LIGHT, light, woodblock } from "loft";

woodblock({ light: LIGHT.threeTone })                 // a named recipe
woodblock({ light: light.recipe({                     // your own: every treatment not given is off
  core: light.core(light.edge.section({ at: 0.3 })),
  rim: light.rim({ width: 0.08 }),
}) })
```

From the command line, on any scene drawn in the woodblock style, without editing it:

```
bun src/cli/main.ts render examples/bosses/crab_lineup.ts --light cel --views lineup --time 2.5 --out out/x.webp
```

`--light` works with `render`, `clip`, `frames`, `profile`, `build` and `variants`. It asks the scene's style for one of
its named lightings (`Style.lights`), so it is not a slot of a design and does not multiply `check --combinations`. An
unknown name fails and lists the names.

## The library (`LIGHT`)

Cost is what the recipe adds to a normal frame of the bruiser from `game` at 1280x800, beside the print as it is in the
same timing rounds (COST_NOTE).

| Name | What it is | For | Cost |
|---|---|---|---|
| `flat` | No shading: every surface in its lit ink, and the key line. | A diagram, a far camera, a base to build on. | COST_flat |
| `twoTone` | The shadow side of every form, and the shadows the parts cast on each other. | The default cel look; reads at game size. | COST_twoTone |
| `threeTone` | `twoTone` and a hard lighter shape on each form's sunward shoulder. | A wet shell; close cameras. | COST_threeTone |
| `bokashi` | Flat inks with a graded wash under every form and up from the water. | The quiet print; a still. The one soft recipe. | COST_bokashi |
| `hatched` | The shadow side drawn in lines across the form, swelling as it turns away. | An engraving; close and middle cameras. | COST_hatched |
| `contact` | Flat inks, and a dark band where a part lies on another or stands in the water. | Separating parts without lighting them. | COST_contact |
| `rim` | The body mostly under a deep indigo block, a cream edge toward the light. | Night, a storm, a lamp behind. | COST_rim |
| `cel` | Two tones, cast shadows, contact bands, sparse highlights. | The nearest match to the painted `shade_best`. | COST_cel |

A recipe that casts shadows or bands overlaps (`twoTone`, `threeTone`, `contact`, `rim`, `cel`) pays for drawing them
after the bodies; the others shade in the bodies' own fragments and add no draw.

## The treatments

Each is a field of `LightRecipe`, a sum type with an `off`, built by `light.<name>(options)`.

| Field | On | Options (defaults) |
|---|---|---|
| `core` | The shadow block printed flat on the shadow side of a terminator. | `light.core(edge)`; an edge is `light.edge.section({ at: 0.4, key })` or `light.edge.sun({ at, key })` |
| `hatch` | Lines across the form whose weight swells from nothing above the terminator to `cap` below it. | `edge` (section at 0.25), `over` 0.5, `deep` 0.5, `cap` 0.8, `spacing` 0.2, `least` 5.5 px |
| `waterline` | A flat band where a body stands in the water. | `height` 0.22, `amount` 1 |
| `bokashi` | The block wiped to a grade: up from the water, and round every underside. | `foot` 2.2, `under` [-0.15, 0.75], `amount` 1 |
| `rim` | A light edge inside the key line on the key's side, cut out of the block. | `width` 0.1, `ink` cream, `amount` 0.9, `key` the sun |
| `highlight` | A lighter ink where the surface faces half way between key and eye. | `size` 13 degrees (5% of a ball's disc), `ink` cream, `amount` 0.32, `key` |
| `cast` | Shadows of every skeleton's capsules on the other bodies. | `reach` 9, `thin` 0.13 (thinner capsules cast nothing) |
| `overlap` | A band on whatever lies close behind a nearer part. | `width` 0.16, `gap` 1.6, `amount` 1 |
| `block` | The shadow block's ink. | `{ kind: "rule" }`, the style's own, or `light.block({ ink, amount, value })` |

Two ideas the options name:

- **The key** (`light.key.sun()`, `light.key.offLens(45)`). The game camera looks down the sun (23 degrees off it), and
  a form lit from behind the lens has no shadow side: a ball shows 4% of itself in shadow. `offLens(n)` is the sun
  swung away from the view axis, in the plane of the two, until it is `n` degrees off; a camera already further off
  sees the sun itself. It is one direction for the whole picture and turns with the camera, not with the things. Cast
  shadows always fall along the sun itself.
- **The terminator round the section** (`light.edge.section`). A bare threshold N.K < `at` blacks out a limb that
  points along the light and pulses as the limb swings. The `section` edge is N.K < `at` |K across the limb|: the same
  share of every section in shadow, however the limb points. The limb's direction is the gradient of the arc length
  along its spine (`ShadeCtx.anchor.x`); on an end cap, and on a traced solid, it is the plain threshold.

## How the block is printed

A recipe's shadows are one overprinted block, as a printer's are: a tone, 1 where the paper is left and 0 where the
block prints whole. Core, hatching, waterline and wash each give a tone and the darkest wins; the rim cuts its edge out
of it; the highlight goes on what is left. The fixed order is in `lit()` (`src/style/light.ts`).

Without `cast` or `overlap` the block is printed in each body's own fragment. With either, the bodies write the tone to
the picture's alpha (the style's `mark`) and three kinds of draw follow in a new phase, `on-bodies`, before the water
(`src/draw/light.ts`): a box per capsule that lowers the tone where the capsule's shadow falls on the opaque scene
(found from its depth); one full-screen draw for the overlap bands; one that prints the block by blending alone
(`picture * alpha + ink * (1 - alpha)`) and sets alpha back to 1. The first two blend alpha with `min`, so shadows over
each other are one shadow. Nothing reads the picture back.

Under a recipe a `bands` or `keys` print rule prints its lit inks and takes the block over them; the rule's own second
inks are for the style without a recipe.

Host and format: two blend states (`Draw.blend` `alpha-min` and `alpha-pull`, lowered to WebGPU blend descriptors the
loaders already read). No new pass, texture or uniform; `docs/artefact_format.md` and the Rust loader are unchanged.

## The sun

`scene({ sun: movable([0.15, 0.8, 0.6]) })` (`src/sun.ts`) makes the sun the host input `sun`, that direction its
default: `--input sun=x,y,z`, `--sweep sun=a..b` on a clip, or a game writing the input. A bare vector is a fixed sun,
a literal in every shader, as before. The crab and the lineup use a movable sun; their default pictures are the same
to the bit. A cloud's own light tables (`src/soft/`) are made on the CPU from the stated direction and do not follow a
moved sun.

## Limits

- A thing without a skeleton casts nothing: the crab's head, the island's own outline (the reef's capsule stands for
  it), trees, sails. A wide flat form casts as the round chain of its skeleton.
- A cast shadow that crosses a highlight prints over the highlight's lighter ink, so it is a little paler there.
- A smear (`src/spacetime/smear.ts`) shades its own fragments whole: it has the core shadow but nothing cast on it.
- `cast` and `overlap` need hull outlines (the default); with `outline: "screen"` the compile fails by name.
- The `section` edge reads screen derivatives, which are constant over a triangle: on a broad, nearly flat face the
  terminator can show steps of a pixel or two along the grid.

## Looking at them

`bun tools/light_gallery.ts stills|sheets|clips|cost` renders every recipe on the crab, another boss and the lineup,
assembles the labelled sheets under `out/shade/`, encodes a clip per recipe, and measures each recipe's frame beside
the print as it is.
