# Inventory: what Loft has, what is reusable, what is still crab

Written 2026-11-12 from a read of every file's header, the public entry point (`src/index.ts`) and the style system.
Sizes are lines of TypeScript. This is a map for deciding what to polish and extract; it proposes, it does not decide.

Status words used below:

- **library**: in `src/`, exported, used by more than one example.
- **library, one user**: in `src/`, but only the crab (or one scene) uses it, so its generality is unproven.
- **kit**: reusable as it stands but lives under `examples/`, so a user cannot import it.
- **crab**: specific to the Reefback Titan; reusable only as a worked example.
- **tool**: a script under `tools/` that encodes a process step.

## The size of it

| Area | Lines | What |
|---|---|---|
| `src/` | 26,500 | the library: 380 exported values from one flat entry point |
| `examples/bosses/titan/` + `crab_lineup` + `lineup_shots` | 6,500 | the crab and its group dance |
| `examples/bosses/lib/` | 800 | parts shared by bosses (faces, eyes, growth, impact) |
| other bosses (wyrm, thundermaw, tithe fleet, wrecker's lamp) | 2,400 | four more monsters |
| other examples (20 files) | 2,600 | serpent, shell, ships, coral, storm, showcase scenes |
| `tools/` | 3,800 | 42 scripts: theory, measurement, deliverables |
| `tests/` | 5,900 | 62 files |

About four fifths of what was written for the crab already sits in `src/`. What remains crab-only is mostly content
(its shapes and faces) and one 700-line rig that mixes generic and specific.

## 1. Core: the language, the compiler, the host

All **library**, all domain-free. Nothing here knows about crabs or the sea except where noted.

| Part | Where | Lines | Notes |
|---|---|---|---|
| Expressions (symbolic maths, CPU and GPU from one graph) | `src/expr.ts` | 1,170 | includes `namespaced` for several subjects in one scene |
| Symbolic differentiation | `src/grad.ts` | 690 | |
| Curves from keys, ramps, waves, responses | `src/curve.ts` | 200 | |
| Map algebra: charts, warps, sweeps, surfaces, tables, distance bricks, surface nets, transported frames | `src/map/` | 1,690 | |
| Compiler: one pass to WGSL and tables; rates (what changes per build, per frame, per pixel) | `src/compile/` | 1,430 | |
| The artefact contract; the artefact as files | `src/artefact.ts`, `src/host/files.ts`, `manifest.ts` | 640 | drawn without the compiler; a Rust loader exists (`rust/loft-loader`) |
| WebGPU host (browser and headless), level of detail, GPU profiling | `src/host/` | 2,330 | headless needs bun |
| Mesh export to glTF | `src/bake/` | 290 | |
| Scene | `src/scene.ts` | 76 | **a scene must have a sea** (see "What blocks an outsider") |
| Sun as a fixed vector or a host input | `src/sun.ts` | 36 | |
| CLI: render, frames, clip, dev (live viewer), profile, check, variants, gallery, build, bake | `src/cli/` | 1,800 | `gallery` renders every scene in every style to one page |

## 2. Drawables: how a shape reaches the screen

All **library**.

| Part | Where | Used by |
|---|---|---|
| Surface (a grid on any map), with level of detail | `src/draw/surface.ts`, `grid_detail.ts` | everything |
| Field (a distance function, sphere-traced per pixel) | `src/draw/field.ts`, `trace.ts`, `field_shadow.ts` | coral, nest, bosses |
| Relief (shell-traced on a surface) | `src/draw/relief.ts`, `src/relief/` | serpent_relief, shell_relief |
| Impostors (a solid per lattice cell) and copies (a list of solids) | `src/draw/impostors.ts`, `copies.ts` | pearls, barnacles, trees |
| Density (a medium integrated in closed form), glows, lightning, rain, clouds | `src/soft/` | storm, thundermaw, night hunt |
| Sky, cast shadows on water, ink outline from depth, the printed shadow block | `src/draw/sky.ts`, `shadows.ts`, `ink.ts`, `light.ts` | every scene |

## 3. Shapes

| Part | Where | Status |
|---|---|---|
| Strand (a swept body with tips and paint), sheetOn (was crest), fan | `src/geom/strand.ts`, `crest.ts`, `fan.ts` | library |
| Chain (was limb: two or three segments by closed-form inverse kinematics) | `src/geom/limb.ts` | library |
| Coil (was shell), spiral tube (was horn), whorl (one family on a logarithmic spiral) | `src/geom/spiral.ts` | library |
| Symmetry: mirror, array, reflect | `src/geom/symmetry.ts` | library |
| Hull from a lines plan; a sailing ship from named numbers | `examples/recipes/hull.ts`, `ship.ts` | recipe |
| Ropes; the signed distance of a closed skin (an attack's `skin` target) | `src/geom/lines.ts`, `skin_distance.ts` | library |
| Sea with swells; whirlpool; a body afloat | `src/geom/sea.ts`, `whirlpool.ts`, `afloat.ts` | library |
| Distance functions (2D, 3D), folds, similarity folds, iterated folds, noise | `src/lib/` | library |
| Height layers, sculpt tables, displacement | `src/relief/` | library |
| Raup coil with a mouth, a lip, bands, a coil paint, and a CPU twin for distances | `examples/bosses/titan/back/spiral_shell.ts` | **crab**; overlaps `src/geom/spiral.ts` |
| Head, brows, plate mouth, cheeks | `examples/bosses/lib/face_parts.ts` | **kit** |
| An eye on a stalk that bends and looks | `examples/bosses/lib/stalk_eyes.ts` | **kit** |
| Sea growth: barnacles, mussels, two corals, kelp; a cluster at a place, a run along a curve | `examples/bosses/lib/growth.ts` | **kit** |
| Water where an attack strikes | `examples/bosses/lib/impact.ts` | **kit** |
| A serpent at any size; ships at game scale; the ghost fleet's parts | `examples/scenes/lib/`, `examples/bosses/tithe/kit.ts` | **kit** |
| Lighthouse, beacon, keep, shrine, headland, terraces; carapaces; claws; legs; eight faces | `examples/bosses/titan/` | **crab** (content) |

## 4. Motion

All in `src/geom/`, closed form in time. Most came out of the crab work and are already extracted.

| Part | Where | Status |
|---|---|---|
| The twelve principles as formulas (anticipation, overshoot, squash, arcs, drag, beats) | `lively.ts` | library |
| Look-at, breathe, beat, strike | `motion.ts` | library |
| Gait: foot targets for many legs from one cycle; `minStep` | `gait.ts` | library |
| Stance: planted feet, a carried body, idle, brace, dance, walk, replanting | `stance.ts` | library, one user |
| A stalk that bends by a pose; a pendulum; restless twitching | `bend.ts`, `pendulum.ts`, `restless.ts` | library, one user |
| A pose struck and held | `struck.ts` | library, one user |
| Expressions as values: a cast of named faces, layered and scored against beats | `expression.ts` | library, one user |
| Placing: a thing placed by Loft's stand-in or by the host | `placing.ts` | library, one user |

## 5. Actions, and what a game reads

| Part | Where | Status |
|---|---|---|
| Attack: one definition, with its warning, beats, hit shape, aftermath | `src/spacetime/attack.ts`, `hit.ts` | library (serpent, crab, bosses) |
| Telegraph, smear, wake, rear-and-slam | `src/spacetime/` | library |
| Grab: reach, close by size, carry, squeeze, set down or throw | `src/geom/grab.ts` | library (`examples/grab.ts`, crab) |
| Ring wave from a slam, and a boat riding it | `src/geom/ring.ts` | library, one user |
| Emote: an action with no hit | `src/spacetime/emote.ts` | library, one user |
| Routine: a group dance as one score of key poses on counts; unison, canon, mirror, solos | `src/spacetime/routine.ts` | library, one user |
| Several subjects in one scene | `src/lineup.ts` | library, one user |
| Design: numbers, slots with named options, named variants | `src/design.ts`, `docs/variants.md` | library, one user |
| The crab's rig: walk, two attacks, hand-off readouts, the dance applied to a body | `examples/bosses/titan/rig.ts` (700) | **crab**, with a generic core inside |

## 6. Checks: arithmetic on the CPU from the same description the GPU draws

`src/check.ts` (870 lines), all **library**: `landsInside`, `clearance` (a measure), `gripReaches`, `releasesAt`,
`throwLands`, `fliesClear`, `letsGoClean`, `shoves`, `frontFirst`, `warned`, `minScreenSize`, `expressionsRead`,
`spansOnScreen`, `staysInView`, `limbsReach`, `budget`; the routine's `inSync`.

Still **tools**, each a check waiting to be one: limbs against a solid over a cycle (`tools/spiral_clear.ts`), a subject
as boxes through a shot's camera with face windows (`tools/lineup_framing.ts`), what shows of an eye in a picture
(`tools/eye_pixels.ts`).

## 7. Styles: the rendering engines

A **style** is one value of type `Style` (`src/style.ts`): seven functions written in the same expression language as
the shapes (`shade`, `ink`, `mark`, `water`, `shadow`, `sky`, plus full-screen `post` passes), an optional printed
shadow `block`, and optional named `lights`. A scene is drawn in another style without being edited
(`withStyle`, `--style`), and in another lighting of its style (`withLight`, `--light`).

| Layer | What exists | Where |
|---|---|---|
| Named styles | `toon`, `woodblock`, `biolume` (a night sea), `papercraft` (folded paper) | `src/style.ts`, `src/style/` |
| Named looks of one style | woodblock only: flat, twoTone, threeTone, bokashi, hatched, contact, rim, cel, sunset, referencePalette, engraving, engravingSunset | `LOOKS` in `woodblock.ts`, `docs/lighting.md` |
| A lighting recipe: ten treatments, each a small sum type that is on or off (key, terminator, core, highlight, cast, waterline, overlap, bokashi, hatch, rim, block) | written as a function of an ink and the shading context, so it is style-free in shape; **only woodblock calls it** | `src/style/light.ts` |
| Paint treatments (a hotter red, mottling, a swelling key line), sea palettes | inside woodblock | `woodblock.ts` |
| White water: rolling foam, curled heads, a crown; seven contact-foam patterns | generic functions, **named and wired for woodblock** | `src/style/whitewater.ts`, `woodblock.ts` |
| Sky: a graded wash or colour stops, a sun disc, cloud bands | **inside woodblock** | `WoodblockSky` |
| Shared pieces: an antialiased edge, colour helpers, post-pass WGSL | exported, small | `src/style/shared.ts` |
| A sheet comparing styles; a gallery of every scene in every style | | `src/style/sheet.ts`, `loft gallery`, `tools/light_gallery.ts` |

### What I would do with styles

Three layers, each built only from the one below it, so the defaults are the proof that the parts suffice:

1. **Parts**: functions a style is composed from, importable on their own. The lighting recipe, the hatch, the rim,
   the wash, a palette quantiser, the sky's grade, sun and clouds, sea lines, contact foam and white water, the
   post passes (ink from depth, paper grain). Most exist; several are trapped inside `woodblock.ts` (860 lines).
2. **Styles**: the named ones, each a short file that composes parts. Ship four to six as defaults and as worked
   examples.
3. **Looks**: named presets of a style, chosen without editing the scene.

The rule that keeps this honest: **a built-in style may use nothing a user cannot import.** The test of the
extraction: write two new styles, say `sketch` (pencil hatching on paper) and `neon` (dark, emissive rims and glow),
each in under about 100 lines, from public parts only, and give `toon` the hatched lighting by passing it the recipe.
If that works, a custom style is fast to roll and no registry or plugin mechanism is needed: a style is a value handed
to `scene({ style })`.

Done 2026-11-12 (docs/styles.md): the parts are out of woodblock (`skyOf`, `swellLines`, `contactFoam`/`marksFoam`
with `printLayers`, paint treatments, `paperGrain`/`bloom`/`reflection`, `litBodies`); every style is composed from
them, toon takes a recipe, and `sketch` and `neon` are written from public parts only (`tools/style_imports.ts`).

## 8. Process: how the pictures got good

| Part | Where | Status |
|---|---|---|
| Theory before render: predict pixels, sizes and clearances from the closed form | `tools/*_theory.ts` (six), `tools/spiral_views.ts` | tools, one per feature; the shared idea is not a library function |
| Shots cut on beats; clips in several formats from one load; per-count strips; a seam proof | `examples/bosses/lineup_shots.ts`, `tools/lineup_deliver.ts` | **crab**, though nothing in it is about crabs |
| Framing on paper before rendering | `tools/lineup_framing.ts` | tool |
| Contact sheets, labelled sheets, picture differences, face shots | `src/cli/image.ts`, `label.ts`, `tools/picdiff.ts`, `face_shots.ts` | library and tools |
| Benches and timing | `loft profile`, `tools/*_bench.ts`, `gpu_time.ts`, `pose_time.ts` | library and tools |
| Reference pictures from an image model, kept with their prompts | `AGENTS.md`, `docs/art_direction/` (nine rounds) | process |
| Skills for agents: an art-direction round, frame time, briefing a worker, lively motion | `.claude/skills/` | process |
| Design notes | `DESIGN.md` (1,160), `docs/` (12 files, 5,350) | written for us, as a record |

## 9a. Extract or not: the rule (added after the owner asked whether extraction bakes our patterns in)

Moving code into `src/` is a promise to keep its shape. Three different things were being called "extraction", and
only the first belongs in the core:

- **Removing a limit** (the core assumes something of ours): a scene must have a sea; the sky, sea lines and white
  water belong to woodblock; names from our history in public types. Fixing these makes the library *less* opinionated.
  Do them.
- **Removing a duplicate**: two shells. Do it when next touched.
- **Promoting a pattern** (how we happened to build a creature, a rig, a film): the creature parts, a legged-body rig,
  the shots system. These are taste, proven on one subject. Do **not** put them in the core. Keep them as recipes: one
  self-contained file each under `examples/`, importing only the public entry point, written to be copied and edited.
  A user gets all of the code and none of the commitment.

A pattern earns a place in the core only when a second, unrelated subject uses it unchanged and it is something a user
could not write from the primitives in an afternoon. By that rule several modules already in `src/` with the crab as
their only user (stance, routine, expression casts, struck poses, emotes, placing, the ring wave, the lineup) are
unproven: they should not be presented as settled API until a non-crab example uses each one. Where that example has
to fight a module, the module is a baked pattern and should be cut back into smaller functions.

### Closed components: an audit (2026-11-12)

A component is **closed** when its shape slots take only numbers: its family of shapes is fixed, and anything outside
it means abandoning the component. It is **open** when its shape slots take functions, or when its mechanism is
exported in the form the level below accepts, so leaving it is one step down.

| Component | Shape slots | State |
|---|---|---|
| `strand` | spine, girth, twist and section aspect are functions | open; four-part comment |
| `limb` (now `chain`), `fan`, `gripper`, `bent` | every slot an expression or a function | open; four-part comments; `limb` renamed `chain` |
| `horn` | curl, taper, length: numbers (a tapered log-spiral tube and nothing else); 13 callers, so every claw finger, thorn, moustache and rostrum we have drawn is that one curve | **opened** and renamed for its shape: `spiralLine` returns `{ spine, girth, up }` as `strand` takes them, and `spiralTube` is `strand({ name, ...spiralLine(o), tips, paint })` |
| `shell` (now `coil`) | the law's five numbers; the section an ellipse or any curve `(u, phi) => [across, along]` | **opened**: `coilAt(law)` places any section point at any whorl position (u may run back: a throat); `coilPlan` is the law in plain numbers (covered arc, which whorls hold a point), the general part of `titan/back/spiral_shell.ts` |
| `hull` | 16 numbers | closed in its numbers (`Lines` is the numbers; the laws are methods and cannot be given directly); its step down is the map algebra's `loft` through stations (`Hull.stations`, `Hull.section`) of your own |
| `ship` | 14 numbers (masts, tiers, jibs): one kind of ship | closed; commented as a composition to copy. Recommended: move to examples as a recipe (its spars, sails and pennant are private and are the ship's taste) |
| `lines` (now `ropes`) | end points and sag expressions | open; renamed (`lines` collided with the hull's `Lines`) |
| `crest` (now `sheetOn`) | height, v, rake, flutter take functions; sag and rays numbers | **opened**: `sheetOnEdges` returns `{ root, edge }` as `sheet` takes them |
| `cloud` | numbers; `profile` keys or `(h, bearing) => share` | **opened**: `billows(places, motion)` boils and drifts any puffs |
| `pendulum` | length, gravity, damping numbers; drives a sum type of expressions | open by its drives (a keyed pivot path is a list of jolts); four-part comment |
| `whirlpool` | numbers | closed (not in this pass: owned by the scene-without-sea worker) |
| `sea` | a list of swells; deformations | closed to anything that is not a swell or a listed deformation |

The pattern for opening one, as done for `horn` (now `spiralTube`): split it into its **mechanism** (a function that returns the spine,
section, lines or field, named as the level below takes them) and a **composition** a few lines long. Nothing about the
component's own call changes; the picture is byte-identical (the base crab's three `game` hashes, checked).

#### Plan for opening the rest (written before building, 2026-11-12)

Each row: the law (what is tied to what), the mechanism one step down, the new name, the relatives its comment names.
Old names stayed as one-line aliases until the callers were renamed; they are gone now (`bun tools/rename_exports.ts
<paths>` renames a branch's callers).

| Today | Law | Mechanism (signature) | New name | Relatives the comment names |
|---|---|---|---|---|
| `shell` | a section carried round an axis while every length grows by W a whorl: point = W^(turns (u - 1)) R(2 pi turns u) [(c + x) out + (-T c + y) axis], c = a (1 + D) / (1 - D) | `coilAt(law): (u, across, along) => P3` places any point of a section at whorl position u (u may run backward: a throat); `coil({ section })` takes an ellipse `{ aspect, ridges }` or a curve `(u, phi) => [across, along]`; `coilPlan(law)` in plain numbers: the centre's offset, the arc of a section the next whorl out covers (the suture), and which whorls hold a point (the CPU twin's inside test, from `titan/back/spiral_shell.ts`) | `coil` (`ShellSpec` to `CoilSpec`; `coil: "right"` becomes `hand`) | a section that changes along u (a keel that grows, a mouth that flares); spire and growth as functions (Raup's T varying: a turban that flattens); W = 1 with a pitch (a spring, a helix: not this law); an Archimedean coil (fixed step per turn) |
| `crest` | a ruled sheet between a root on the host's line v (rho 0.85) and an edge raised along the section's outward direction plus rake along the spine, scalloped by sag sin^2(pi rays x) | `sheetOnEdges(spec): { root, edge }` as `sheet(root, edge)` takes them; `v` and `rake` also take functions of x | `sheetOn` | a root that winds round the body (v a function: a spiral frill); a curled or doubled edge; a curved sheet (`surface` on the host's frame, not ruled); `fan` between rays |
| `fan` | already open: rays are curves, sag and scallop expressions, catenary across each gap | (is the mechanism) | keep `fan` | rays from `bent` stalks; a fan of fans; a different gap profile than the catenary |
| `limb` | two or three straight segments, the middle joint by the law of cosines on the `bend` side | already open: `joints`, `knots`, `spine`; `polyline` for more points | `chain` (`LimbSpec`, `Limb` to `ChainSpec`, `Chain`) | four or more segments (FABRIK, or chains of chains through `polyline`); curved segments (`bent` per segment); joint limits |
| `hull` | stations from a lines plan (half-breadth, keel, sheer, superellipse bilge with tumblehome), lofted by Catmull-Rom along the length | the stations exist (`Hull.stations`, `Hull.section`); the general mechanism is the map algebra's `loft(curves)` | keep `hull` (a lines plan is a hull's) | any lofted body through your own stations: a pod, a canoe, a fish; `lathe` for round ones |
| `ship` | 14 numbers pick one of three mast plans and decide everything else | not opened in code: its parts (`spar`, sails, pennant) are private | keep; **recommend moving to examples as a recipe** (a subject, not a shape) | |
| `lines` | ropes between moving end points, parabolic sag, one stroke | already open (end points are expressions); `sag` becomes an expression | `ropes` (the type is already `Rope`; `lines` collides with the hull's `Lines`) | catenary sag; ropes along a curve (a net) |
| `cloud` | puffs in a shape of revolution: three tiers by hash, flattened base, each puff boiling in its own noise and drifting | `billows(places, motion): Cloud` boils and drifts any list of puffs; `profile` also takes `(h, bearing) => share` | keep `cloud` (the shape's word; it is smoke, steam and dust as well) | a smoke ring, a trail along a curve, a column bent by wind: places from any curve or field |
| `pendulum` | damped driven oscillator, steady state plus impulse response, linear in drives | already open: drives are a sum type summed by `answer` | keep | a piecewise-linear pivot path is exactly a sum of jolts at its keys; a double pendulum is not linear (not this) |
| `strand`, `bent`, `gait`, `gripper` | already open | | keep | four-part comments only |

What changed from the plan when built: `hull` got no new code, because the general mechanism (`loft`) already existed
in the map algebra and the hull's skin is that loft with u and v swapped; `ship` was commented, not opened, and is
recommended for the examples. `coilPlan` takes an `aspect` (the plain-number twin knows only elliptical sections).
`thunderhead` is a preset of `cloud` and by the naming rule belongs in the examples; moved to `examples/recipes/`. The one-step-down
uses are in `examples/open.ts`.

### Names: geometry in the library, subjects in the examples (2026-11-12, the owner's rule)

A name is a pull as well. A function called `horn` gets used for horns; called `spiralTube` it gets used for whatever
is that shape. So a library name says what the geometry is, and a subject's name (`handlebar`, `tusk`, `rostrum`)
appears only in an example, where the reader sees it is a shape plus numbers.

Proposed, not yet done (each is a rename across callers, to be made when no branch is in flight):

| Today | Shape | Proposed |
|---|---|---|
| `horn`, `hornLine` | a tapered tube on an equiangular spiral | `spiralTube`, `spiralLine` (done) |
| `shell` | a section swept on a growing spiral round an axis | `coil` (done; alias removed) |
| `crest` | a sheet standing on a curve of a surface | `sheetOn` (done; alias removed) |
| `limb` | a chain of straight segments reaching a target | `chain` (done; alias removed) |
| `lines` | ropes between moving end points, one stroke | `ropes` (done; alias removed) |
| `tusk` (example side) | a flat plate between two points | `plate` |
| `dome` (example side) | a superellipsoid with relief on its front | `reliefDome` |
| `strand`, `fan`, `whorl`, `gripper`, `bent`, `gait`, `pendulum`, `cloud` | already say a shape or a mechanism | keep |
| `hull`, `ship` | these are their subject (a lines plan is a hull's) | done: both are recipes (`examples/recipes/hull.ts`, `ship.ts`); the library keeps only a `skin` attack target, the closed solid any lofted surface bounds (`src/geom/skin_distance.ts`); `thunderhead` likewise (`examples/recipes/thunderhead.ts`) |

### What a component's comment must say (2026-11-12, the owner's rule)

Four parts, in this order, so that a reader (or a model) is pushed outward as well as shown the way in. `spiralTube`
in `src/geom/spiral.ts` is the worked example.

1. **The shape and its law**, in a sentence or two of mathematics: what is tied to what.
2. **Parameters**: which take expressions (and so can move), which are fixed.
3. **What it cannot do**, as a list, each item a consequence of the law. A limit that is not written down is read as
   the whole space.
4. **Going past it**: the one-step-down call that keeps most of it, and the relatives: other shapes from similar
   mathematics, each with the line of reasoning that derives it, so a variation is an invitation and not a research
   project.

The table below is kept as first written; rows 2, 4 and 5 are patterns by this rule (recipes, not core), rows 1, 3
and 6 are limits or duplicates (do them), and rows 7 and 8 wait for a second use.

## 9. What can be extracted next, most valuable first

| # | Extract | From | Into | Why it is worth a name |
|---|---|---|---|---|
| 1 | A scene without a sea: the ground as a choice (sea, a plane, nothing) | `src/scene.ts`, every style's `water` | core | without it every example is a sea picture, and an outsider's first scene is blocked |
| 2 | Shots and deliverables: a cut list on beats, camera moves, formats, one load to clips and strips, the seam proof | `lineup_shots.ts`, `tools/lineup_deliver.ts` | `src/film/` | anyone who wants a shareable clip needs it; today it is hard-wired to the lineup |
| 3 | Style parts: sky, sea lines, contact foam and white water, post passes, out of `woodblock.ts`; the recipe usable by every style | `src/style/` | `src/style/parts/` | makes a custom style a short file (section 7) |
| 4 | A legged body: stance, gait, carriage, limbs, a display limb, attacks and a routine applied to it | the generic core of `titan/rig.ts` | `src/geom/` or a kit | the next creature should not copy 700 lines |
| 5 | Creature kit: head and brows, stalk eyes, growth, impact water | `examples/bosses/lib/` | an importable kit, apart from the core | reusable today, but not importable |
| 6 | One shell: the mouth, lip, bands, coil paint and CPU twin joined to the library's shell | `titan/back/spiral_shell.ts`, `src/geom/spiral.ts` | `src/geom/spiral.ts` | two shells in one repo |
| 7 | Checks from tools: stays outside a solid; a face is unobstructed in a shot; a group fills the frame | `tools/spiral_clear.ts`, `lineup_framing.ts` | `src/check.ts` | they caught real faults three times |
| 8 | Predict before render: one helper to project a point, a box or a part through a camera at a time on the CPU | the six theory tools | `src/check.ts` | the habit that made one-shot builds possible |
| 9 | Music from a score (in progress) | the song worker's branch | example side first | unproven until heard |

Things that should stay examples: the crab's shapes, faces and backs; the other bosses; the numbers records. They are
content. Their value to others is as worked examples of slots and variants.

## 10. What blocks an outsider today

1. ~~Every scene needs a sea.~~ Done 2026-11-12: `scene({ ground })` takes a sea, a plane or nothing (`DESIGN.md` 13).
2. **One flat entry point with 380 names.** No layering tells a newcomer which twenty matter. A split by layer would:
   core (expressions, curves, maps, draw, scene, compile), shapes, motion, actions and checks, styles, kit, film.
3. **No way in for a reader.** The README is 39 lines for someone who already knows the design; `DESIGN.md` is our
   record, not a guide. There is no "first scene in ten lines", no ladder of examples.
4. **The examples are all sea monsters**, and the large ones (the crab is 6,500 lines) are not where to start.
5. **The package is private** (`0.0.1`, no export map, no build for npm); headless rendering needs bun.
6. **Load time grows with content**: five crabs are 22 MB of WGSL and one to three minutes to load. A shader module
   also fails to build now and then under load; that is uninvestigated.
7. **Names from our own history** in public types (`WoodblockLook`, `Staged`, `Standing`, `STAND_IN`): each is a
   question a stranger must ask.

## 11. Examples worth having, by part of the system

Existing examples already cover: a swept body (`serpent`), a function of (u, v) (`shell`), ships from numbers (`brig`,
`fleet`), folds and tracing (`coral`, `nest`, `pearls`), soft things (`storm`), an attack (`strike`), a grab (`grab`),
a whirlpool (`maelstrom`), relief (`serpent_relief`, `shell_relief`), five showcase scenes, five bosses, the group dance.

Missing, and each should be one short file that shows one idea:

| Example | Shows | Subject that is not a crab |
|---|---|---|
| first scene | expressions, a curve, one strand, a camera | a paper lantern or a teapot on a table (needs item 1) |
| one scene, every style | `withStyle`, looks | the same lantern in toon, woodblock, papercraft, biolume |
| a custom style | composing parts | `sketch` or `neon` (needs item 3) |
| a creature with variants | `design`, slots, `loft variants` | a lantern fish with three heads and two tails |
| a face that acts | `cast`, `score`, `struck` | an owl, or a mask on a wall |
| a walker | `gait`, `stance`, `limb` | a heron or a walking house on four legs |
| an attack a game can read | `attack`, `telegraph`, checks | a lighthouse beam sweeping the water |
| a duet | `routine`, `inSync` | two buoys or two cranes dancing eight counts |
| a clip to share | shots on beats, formats, the seam | any of the above (needs item 2) |
| a scene as files | `build`, the loader, the Rust loader | the first scene |

## 12. The work, in waves (2026-11-12)

The owner: "do all the library work; reflect and map it all out; use sub agents and get it all done." What "all" is,
what depends on what, and who holds which files. A wave starts when the files it needs are free.

### Wave 1: running now (five workers; five is one over the usual cap, taken because the fifth barely uses the GPU)

| Work | Branch | Owns | Proof it is done |
|---|---|---|---|
| A scene without a sea: the ground as a choice (sea, plain ground, nothing) | `lib/ground` | `src/scene.ts`, `src/compile/`, `src/draw/`, `src/geom/sea.ts`, `src/check.ts`, `src/host/`, `examples/first/` | existing pictures byte-identical; three non-sea examples in four styles |
| Style parts out of woodblock; the lighting recipe for any style; `sketch` and `neon` built from public parts only | `lib/style-parts` | `src/style/`, `src/style.ts`, `src/draw/light.ts`, `docs/styles.md` | four styles byte-identical; two new styles in about 100 lines each |
| Closed components opened, named for their geometry, commented in four parts | `lib/open-components` | `src/geom/` except the sea's files, `src/soft/cloud.ts`, `examples/open.ts` | pictures byte-identical; one use of each a step down |
| `loft api` (the reference, generated), the craft guide (`docs/craft.md`), `loft context` (both), `docs/guide.md`, `README.md` | `docs/guide-context` | `src/cli/`, `README.md`, `docs/guide.md`, `docs/craft.md`, `docs/api.md` | every snippet run by a script; every export listed |
| Crab body plans and a taller hermit shell (art, not library) | `art/body-plans` | `examples/bosses/titan/` | silhouettes tell the five apart; checks pass |

### Wave 2: when wave 1 has merged (the callers are free and the ground exists)

| Work | Needs | Proof |
|---|---|---|
| Renames finished: every caller on the shape names, every alias deleted, in one commit | open-components and body-plans merged | no alias left; pictures byte-identical |
| The second shell removed: the hermit crab's shell as a thin use of the library's coil | open-components and body-plans merged | the islander's pictures byte-identical or better by the owner's eye |
| **A second subject for every one-user module** (stance, gait with planted feet, expression casts, struck poses, emotes, the routine, placing, the ring wave, the lineup, the design's slots): short non-crab examples, each one idea: a heron that walks, an owl's face that acts, two buoys in a duet, a lighthouse beam as an attack a game can read, a lantern fish with variants | ground merged | each example under about 100 lines; where one has to fight a module, the module is cut back into smaller functions and the fight is written down |
| Recipes: the boss parts (`examples/bosses/lib/`), the shots-on-beats cut and the clip delivery, as self-contained files that import only the public entry and say in their header "copy and edit" | open-components merged | each runs alone; none imports another example's private file |
| Guide and craft text revised for the ground, the style parts and the new names; the snippet check re-run | all of wave 1 merged | every snippet passes on main |

### Wave 3: last, because everything else moves what it touches

| Work | Proof |
|---|---|
| The entry point by layer: core (expressions, curves, maps, drawables, scene, compile), shapes, motion, actions and checks, styles; the one flat list of about 380 names goes, and the modules still unproven after wave 2 are marked so | `loft api` shows the layers; every example still imports from a public entry |
| Robustness an outsider meets: the shader module that fails to build now and then under load (find the cause); load time against content (five crabs: one to three minutes), with what can be cached or built ahead | the failure reproduced and fixed or explained; a measured load-time table |
| Package: an export map, a version, what runs without bun; nothing is published without the owner saying so | `pnpm pack` produces a package whose examples run |
| The whole test suite once on a quiet machine, and every failure chased | all green, or each failure named |
| The basin test: a fresh agent, the library and `loft context`, three unrelated prompts with no reference; do the results share silhouettes, proportions or palette with our examples? | a sheet and a verdict; whatever pulls is fixed in the docs or the examples |

Not planned: a judging packet or any training material (the team that trains has its own; Loft supplies its type
checker and a render that succeeds or fails); promoting the boss parts or a legged rig into the core (9a).
