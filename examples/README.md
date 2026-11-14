# Examples

These are a starting point to move away from, not a target. Most of them are sea monsters because that is what we
were making; the library is not about the sea. Pick the example nearest to your **idea** (a swept body, a field traced
per pixel, a cut list), not to your subject, copy it, and change it until nothing of ours is left.

**Recipes** are the files meant for copying: each says what it makes and from which library pieces, its parameters,
what it cannot do, and what to change first, and imports only the public entry (`src/index.ts`) or the files its
`Uses:` line names (`bun tools/check_recipes.ts` proves it). Everything else is an example: read it, do not import it.

Sizes are lines. Run any scene with `pnpm loft render <file> --out out/x.webp` or `pnpm loft dev <file>`.

## A first scene

| File | Lines | Shows |
|---|---:|---|
| `serpent.ts` | 262 | a body as data: named numbers, a curve, a `strand` swept along it, the head on the neck's chart, closed-form motion |
| `brig.ts` | 44 | one component from its numbers alone: a ship (`ship`) on a swell |

## Shapes

| File | Lines | Shows |
|---|---:|---|
| `shell.ts` | 49 | a surface written directly as a function of (u, v): a circle swept on a logarithmic spiral |
| `shell_relief.ts` | 89 | relief on a surface: ridges and knobs as real geometry, not paint |
| `serpent_scales.ts` | 47 | paint by folding body coordinates into cells |
| `serpent_relief.ts` | 146 | overlapping scales and clusters as relief on a swept body |
| `fleet.ts` | 57 | variety by numbers: three ships from one component |
| `stormwing.ts` | 181 | a flat, broad creature read from above: wing membranes (`fan`) on strands, mirrored, with a lagging wingbeat and an attack input |

## Motion

| File | Lines | Shows |
|---|---:|---|
| `islandback.ts` | 162 | a walk: a legged body with a gait through shallow water, a shell carried on its back |
| `maelstrom.ts` | 94 | a deformation of the sea (`whirlpool`) and things riding it (`afloat`) |

## State (a step, baked)

Closed forms first (a kick is a `jolt`, a trail is the thing at earlier times); these are for what truly steps. Each is
a `recur`: the step runs once on the GPU and the scene reads it as a function of time.

| File | Lines | Shows |
|---|---:|---|
| `skin.ts` | 35 | a grid settled before the scene starts: reaction and diffusion, kept as one still state, read as paint |
| `flock.ts` | 40 | a list with every state kept: 200 motes chasing a lure that is a closed form of time |
| `rope.ts` | 46 | a loop closed by physics: a rope settles into its anchor's swing, and `loft check` measures the seam |
| `creep.ts` | 39 | a grid too large to keep every state: checkpoints, each frame replaying at most 15 steps |

## Actions

| File | Lines | Shows |
|---|---:|---|
| `strike.ts` | 139 | attacks a game can read: a bite along a capsule and a sweep through an arc, with their hit shapes |
| `grab.ts` | 93 | a grab sized to what it grabs: claws close on three boats |

## Styles

No file yet shows one scene in every style (`withStyle`); the nearest are whole scenes in one style each:
`scenes/night_hunt.ts` (biolume), `scenes/island_tide.ts` and `scenes/the_strike.ts` (toon), `scenes/maelstrom.ts` and
every boss (woodblock), and `bosses/crab_lineup.ts`, whose `--look` switches between five looks of one style.

## Light and materials

| File | Lines | Shows |
|---|---:|---|
| `light/lantern_night.ts` | 26 | a lamp inside a paper lantern at night: the flame lights the floor (`lamp`, `scene({ lights })`), the paper glows (`emits`), the sun dimmed to a moon |
| `light/lure.ts` | 36 | a creature lit only by its own lure: one `const` for the flare and the lamp, a check that the head is lit (`litBy`) |
| `light/road.ts` | 30 | a road of patterned lamps as one function, by repetition (a `Light` written by hand) |
| `light/area_lamp.ts` | 33 | a lamp with a size whose light function casts its own shadow (`castRay`): hard in a frame, soft in a still (`--samples 64`) |
| `materials/lacquer.ts` | 28 | wet lacquer: `gloss` and a darker `base`, words every glossy style reads |
| `materials/beetle.ts` | 36 | a thin-film shell: the surface's own answer to each light (`scatter`) |
| `materials/poker.ts` | 37 | a word you invent (`kelvin`) and the style part that reads it |

## Soft things

| File | Lines | Shows |
|---|---:|---|
| `storm.ts` | 104 | a cloud of Gaussian puffs with lightning inside it (`cloud`) |

## Tracing (a field drawn per pixel)

| File | Lines | Shows |
|---|---:|---|
| `coral.ts` | 85 | a branching form from a kaleidoscopic fold, sphere-traced (`draw.field`) |
| `coral_list.ts` | 65 | the same form listed as copies instead of asked per pixel |
| `nest.ts` | 86 | one shape repeated at shrinking scales through a fold (`repeatScale`) |
| `pearls.ts` | 53 | a lattice of shapes, ray-cast by walking the cells a ray crosses |

## Your own geometry and rays

| File | Lines | Shows |
|---|---:|---|
| `shatter.ts` | 75 | triangles from your own algorithm (a Voronoi shatter) moved by an expression of time (`draw.mesh`) |
| `bead.ts` | 38 | a solid you intersect yourself, here by intervals, drawn per pixel (`draw.traced`) |
| `mirror.ts` | 29 | a reflected ray cast from a paint at another thing's field (`march`) and at everything's stand-in (`castRay`) |
## Pictures drawn off screen (`pass`) and stills

| File | Lines | Shows |
|---|---:|---|
| `reeds_shadow.ts` | 71 | shadows from every shape the sun sees: a depth picture from the sun read by a style part; a CPU check that every caster lands inside it |
| `pool.ts` | 51 | a pool that mirrors what stands round it: the view's camera reflected, half size, read by the ground |

A still needs no new name in a scene: `loft render <scene> --samples 64 --shutter 0.05 --aperture 0.08` averages 64
samples of one instant (motion blur over the shutter, depth of field focused on the camera's target).

## Whole scenes

| File | Lines | Shows |
|---|---:|---|
| `scenes/the_strike.ts` | 116 | one gameplay beat on a loop, read from the game's camera |
| `scenes/island_tide.ts` | 167 | a walker on a reef flat in daylight |
| `scenes/stormfront.ts` | 102 | a flier at the edge of its own storm |
| `scenes/maelstrom.ts` | 133 | a whirlpool with ships and wreckage, woodblock |
| `scenes/night_hunt.ts` | 173 | a lit chase at night: glowing serpent, lanterns as the only light |
| `bosses/maelstrom_wyrm.ts` | 561 | a boss: a serpent coiled round its whirlpool, attacks and warnings |
| `bosses/thundermaw.ts` | 391 | a boss partly made of cloud |
| `bosses/tithe_fleet.ts` + `bosses/tithe/` (1 file) | 593 + 176 | a boss that is a group of ships |
| `bosses/wreckers_lamp.ts` | 696 | a boss that stands still: a spire with a lit eye |
| `bosses/reefback_titan.ts` + `bosses/titan/` (36 files) | 326 + 6,247 | the largest example: one creature with slots and variants (`design`), a rig, faces that act, a routine. Not a place to start |
| `bosses/crab_lineup.ts` | 227 | five variants of the titan dancing one routine in sync (`lineup`, `routine`) |
| `bosses/lineup_shots.ts` | 122 | the lineup's cut list: a use of the film recipe on a tempo, two formats |
| `bosses/crab_song.ts` | 323 | music as a function of time, written to the lineup's counts |

## Recipes (copy and edit)

| File | Lines | Makes |
|---|---:|---|
| `recipes/film.ts` | 106 | a film: a cut list (stills and eased moves) per format, on a tempo or not; `bun tools/film.ts` delivers it (sheets, a seam proof, clips, sound) |
| `serpent_clip.ts` | 23 | a use of `recipes/film.ts`: seven seconds of the serpent, four shots, wide and square |
| `bosses/lib/face_parts.ts` | 272 | a head mass with relief, eyes, lids, brows, a mandible, a moustache |
| `bosses/lib/stalk_eyes.ts` | 170 | an eye on a bending stalk, its look, its lids, and the marks a check reads |
| `bosses/lib/growth.ts` | 253 | seeded clusters of cones (barnacles, mussels, coral, kelp) in one copies list |
| `bosses/lib/impact.ts` | 140 | foam where something strikes the water, one draw for any number of splashes |
| `bosses/lib/warning.ts` | 20 | a telegraph's colours chosen so every alpha stays in the palette |
| `bosses/lib/scale_ship.ts` | 36 | the small ship that gives a picture its scale |
| `scenes/lib/fleet.ts` | 38 | a component scaled to a game's size, keeping its proportions |
| `scenes/lib/wyrm.ts` | 184 | a serpent builder at any size, named so two share a scene |
