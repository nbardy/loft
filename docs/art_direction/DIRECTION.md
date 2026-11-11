# Art direction for the five monsters: the woodblock print

Chosen 2026-11-11 from `round1_sheet.webp` (six directions by four subjects, generated images; row A is this one).
The subjects are the game's existing five bosses. The direction is new: the game's current look is a bright toon.

## The look

- **Line first.** Every form is bounded by a dark ink line whose weight varies. Detail inside a form is a pattern of
  lines (scales, planks, wave lines, cloud curls), not shading.
- **Flat colour, few inks.** Indigo sea, cream paper, and one accent per monster. No gradients on a body.
- **Graded wash only at the edges of the world**: the sea toward the horizon, the sky.
- **Foam as cream claws** round whatever breaks the water.
- **Scale.** Each monster dwarfs a ship. Every picture has one small ship with blue sails for scale.
- **Silhouette before detail.** It has to read from a strategy camera, high and three-quarter.

## One accent each

| Monster | Body | Accent |
|---|---|---|
| Maelstrom Wyrm | slate and olive coils | vermilion gills and pearl glands |
| Reefback Titan | brick-red crab | moss-green atoll, cream ruin |
| Thundermaw | cream cloud, indigo shadow | one vermilion eye; white lightning |
| Tithe Fleet | black hulls, bone sails | one red pennant on the flagship |
| Wrecker's Lamp | ink-black spire and figure | lantern yellow, the only warm colour in the picture |

## References

- Row A of the sheet: `round1/01_A_woodblock_whirlpool.webp`, `07_A_woodblock_storm.webp`,
  `13_A_woodblock_ghostfleet.webp`, `23_A_woodblock_island.webp`. The lamp has no woodblock image; its subject is in
  `21_C_inkbook_lure.webp`.
- What each monster is, its parts, phases and attacks: the game repository's
  `notes/design/2026-11-01_fleet_spells_bosses.md`, sections 6.1 to 6.5.
- The old versions, for subject only: `~/git/room-runners-arena-lib/output/boss_variants/baseline/`.

Because style is separate from shape in Loft, the same scenes also render in the other styles. Woodblock is the one
they are designed for.

## Round 2: new directions for the crab (2026-11-11)

`crab_round2/sheet.webp`: six designs by five views (two far step-backs, face, legs, back), generated with the Klein
one-step model from `crab_round2/prompts.json` (`tools/crab_concepts.py`, run with the flux-klein-fast project's
environment). Chosen to build as 3D variants of the Reefback Titan:

- **mask**: the heikegani. The carapace is a scowling war mask; legs are lacquered armour plates; a keep and two pines
  on the back.
- **stilt**: a spider crab. A small thorny body carried high on very long thin legs, a foam ring at each foot, a red
  shrine gate and lanterns on the back.
- **fiddler**: one claw as large as the body, the other tiny; tall periscope eyes; a leaning lighthouse on the back.

Not chosen: fortress (reads as a wall, not a crab; grey), garden (no silhouette from far away), aqueduct (the
amphitheatre reads before the crab; the arched legs are worth stealing).

All three are built as variants of the crab (`examples/bosses/reefback_titan.ts`, options under
`examples/bosses/titan/`): `crab_round2/variants_sheet.webp` shows base, mask, stilt and fiddler at five cameras
(`bun src/cli/main.ts variants examples/bosses/reefback_titan.ts --only base,mask,stilt,fiddler --views game,far,face,legs,back`).
Every variant passes the crab's checks. Open: the fiddler asks for a crush disc of 4.4 (base 3.4); `fiddlerBaseDisc`
is the largest claw that fits the base disc.

## Round 3: a face for the island crab (2026-11-11)

The owner, after seeing base, mask, stilt and fiddler side by side: the stilt crab is not the direction; "I like the
lower to the water non stilt version and the island back, just wanted more face identity"; "like fiddler, fiddler has
great identity and nice face expressions". So: the base crab (carapace, legs, atoll) with the fiddler's one great claw
and a face on tall eyestalks that has character and visibly changes expression. The variant is `islander`
(`examples/bosses/reefback_titan.ts`); its face is `examples/bosses/titan/face/dandy.ts`.

What was wrong with the periscope face on this crab, measured on `claws=fiddler,face=periscope`:

- its eyes are 34 px across from the `game` camera at 1280 x 800 (radius 0.46);
- it has one lid, raised out of sight, so nothing about the eyes changes but shut or open;
- when it looks at the ship or the disc the pupil is on the far side of the eye from the game camera. The camera is
  40 degrees above the horizon, in front and to the right; the ship is 112 degrees round from it as the eye sees them
  and the disc 78. So in every beat of either attack the camera sees a cream ball and no pupil.

### The face

Temperament: vain, a show-off, a bully (kept from the periscope). What is new is that the temperament does the
staging: **the eyes face the audience and never quite leave it**. The socket turns a quarter of the way to what it
looks at and the iris slides the rest across the white, so it watches the ship from the side of its eye, and the game
camera always sees white, iris and pupil. Two lids an eye (a brow above, a cheek below, painted coral), a moss-green
iris (the island's own ink), everything bigger on the great claw's side (the right eye is larger, higher, wears its
lid higher and leads by 0.14 s), and a handlebar moustache of two curled antennae.

Kept from before: the eyeball on a stalk, the lid as a spherical lune and its ink, the stalks' whip after the slam,
the door-plate mouth (all `faceKit`); watching the audience at rest and the stalks trailing the walk (the periscope).
New: the audience-facing socket with a sliding iris, the lower lid, the iris, the size and the lopsidedness, the
moustache, and expressions as values (`src/geom/expression.ts`): six named expressions over one rest, laid over the
fight by its own clocks.

| expression | when (the rig's clocks) | what is drawn |
|---|---|---|
| rest (posing) | otherwise | lids half down and uneven, the right eye cocked higher, looking at you |
| covet | from the Grab's warning until it looks away | eyes flung wide, pupils swollen, stalks craned up together toward the ship |
| strain | the hold; bites with each squeeze | slits under a scowl, stalks drawn down and trembling, mouth clamped |
| glee | from the let-go, 1.5 s on | cheeks push the eyes into arches, stalks flung apart and back, laughing |
| glare | Claw Crush's wind-up | brows hard down to the middle, pinpoint pupils, stalks low and thrust at the disc |
| wince | the slam, 0.5 s | shut tight on a lash line, stalks jolted down, the lopsidedness thrown over |

Expression references were to be generated first (`tools/crab_faces.py`: six moods, each close and at game distance).
A process from `~/git/flux-klein-fast` was running when this started, so the shapes came from the round 2 fiddler
pictures and from looking at renders. The sheet was generated after the build, once no such process was running:
`crab_round3/sheet.webp` (12 pictures, `crab_round3/prompts.json`). It did not shape this face; it is a second opinion
on it. The model would not put eyes on the stalks: in all twelve the eyes sit on the carapace's front under heavy
brows, and most of each expression is an open mouth (the greedy, gleeful and furious ones especially). This face has
almost no mouth from the game camera. A mouth on the carapace's front that the game camera can see is the next thing
to try.

### Predictions (written before the first render of this face)

From `tools/face_theory.ts`: plain arithmetic on the face's own numbers, the body at rest, each expression at full
weight, the `game` camera at 1280 x 800.

1. **Size.** The right eye is 60 px across and the left 45 (the periscope's: 34). At rest the iris is 30 and 22 px
   across and the pupil 18 and 13. `minScreenSize` will measure each eyeball within 4 px of that.
2. **Contrast.** The white (cream) is drawn inside its own dark surround (dark lids, a dark back to the eyeball, the
   ink outline), on the brick body: by the inks, 4.9 to 1 against brick and about 9 to 1 against the lid. In the
   `game` picture at rest, the mean luminance of the cream pixels inside the right eye over the mean of the ring from
   1.0 to 1.3 radii round it will be at least 3.5 to 1 (lower than the inks alone: the ring takes in the stalk and the
   shadow print).
3. **Expressions.** All seven (rest and six) reach at least 0.9 of the face in a cycle. For every pair, some drawn
   vector (stalk root to eye, eye to a lid's middle, eye to pupil, eye to its corner) differs by at least a quarter
   of the right eye's width (15 px). The closest pair is rest and glee at 33 px (the right stalk), then strain and
   glare at 33 px (the lower lid) and rest and strain at 34; the farthest is covet and wince at 101. Measured on the
   moving crab (`expressionsRead`), the closest pair will be between 23 and 43 px. By eye, from stills at the game
   camera, at least six of the seven are told apart at a glance; the pair most likely to be confused is strain and
   glare (both scowl with low stalks).
4. **Where it looks.** Looking at the ship the pupils sit 22 px right and 9 px down of the eyes' middles (right eye);
   at the disc, 20 px straight down; at rest, centred. Each is inside the white, so all three read.
5. **Cost.** On `claws=fiddler,face=periscope` the `game` frame is 2.62 ms of GPU time (the fast clock state; pose
   stages 0.39), 84 draws, 102,371 triangles drawn, 122 map tables. The islander: at most 2.75 ms (expected one timer
   tick more, 0.07), 90 draws, about 105,300 triangles, about 156 map tables.

### Scored (the same day, after the build)

Pictures: `out/face/expression_sheet.webp` (each expression from `face` and from `game`, with the game window at its
true size), `out/face/variants/sheet.webp` (base, fiddler, islander through far, face, game),
`out/face/islander_game_cycle.mp4` and `islander_face_cycle.mp4` (one 16 s cycle each). `loft check` on the islander
passes 7 of 7: the crab's four and the face's three.

| | predicted | measured |
|---|---|---|
| right eye, `game`, 1280 x 800 | 60 px | 59.0 px (`minScreenSize`) |
| left eye | 45 px | 42.5 px (moved 0.5 further from the camera after looking; the theory on the final numbers says 44.2) |
| white against its surround (luminance) | at least 3.5 to 1 | 3.56 to 1 (white 0.743, ring 0.209; `game`, t = 6.3) |
| every expression reaches 0.9 of the face | 7 of 7 | 7 of 7, each 1.00 |
| closest pair | rest and glee, 33 px, in 23 to 43 | rest and glee, 26.5 px (0.44 of the eye; the limit is 0.25) |
| told apart by eye at the game camera | at least 6 of 7 | 7 of 7 in the sheet's true-size insets, my own judgement |
| GPU frame | at most 2.75 ms (2.62 before) | 2.49 ms in the fast clock state; back to back in the slow one, 4.46 before and 4.52 after: one timer tick |
| draws, triangles, map tables | 90, about 105,300, about 156 | 90, 106,003, 160 (and 2 scan tables, the moustache's, not predicted) |

Prediction 4 (where the pupils sit) was not measured as a number. In the pictures the three looks are distinct: centred
at rest, at the right edge of the eye toward the ship, low toward the disc.

The numbers held; the first pictures did not. What was wrong, and each was a fault in the reasoning, not a slip:

- **Shut eyes were two dark balls on a brick body**, the complaint this round started from. I had kept the base
  face's lid ink (the body's dark brick) and argued that a dark surround makes the white read. It does, while there
  is white. Wince and strain have almost none, and then the lid is the whole eye. The lids are now coral: lighter than
  the body, darker than the white. The contrast figure passed with either ink and did not see this; a check of the
  shut eye against the body would have.
- **The two eyes overlapped from the game camera.** The theory measured each eye alone. They are 2.55 apart across
  the body now (were 1.8).
- **The first glee hid its pupils**: it looked down at the ship, behind its own raised cheeks. Glee now looks half at
  the ship and half at the audience, which is also more like it.
- **The first wince's lash line faced the water.** With the brow pulled right down the lids met below the eye's
  middle, and the camera above saw only the brow's top. They meet on the middle line now.
- **Facing the game camera squarely was wrong for every other camera**: from `face` and `far` (lower) the eyes showed
  their undersides. The sockets face 10 degrees lower than the game camera; the pupils still look at it.
- The socket's turn and the iris's slide were cut from 0.3 and 0.45 to 0.25 and 0.4: at the first numbers the pupil sat
  on the eye's edge from the game camera when it looked at the ship.
- The stalks were cut from 3.0 to 2.5 tall so the right eye stays inside the `face` camera's frame; it still touches
  the top in glee. I had not looked at that camera in the theory at all.
- The pair I expected to be confused, strain and glare, is not: the glare's slant reads. The weak one is glee, which
  in a still reads as wide-eyed and pleased more than as laughing (the laugh is in the motion: the stalks bob and the
  mouth chatters five times a second).

Weak or unfinished:

- From the game camera in covet the pupil fills the iris and sits at the eye's edge: the look reads (white eyes, black
  pupils turned at the ship), the green does not.
- At the slam the right eye drops onto the carapace and its stalk is hidden behind it from above.
- The moustache reads as a smile from the front. That suits posing and glee; in the glare it lifts where a snarl
  would turn down.
- The mouth plates are driven by the score but are hidden from the game camera by the claws most of the cycle.
- The audience is a fixed point (the `game` camera's eye, written in `face/dandy.ts`). If the game's camera orbits, the
  point must become a host input.
- The contrast was measured on one frame by a throwaway script, not by a check.


## Contact foam (2026-11-11)

The owner, looking at the islander: "the little white web-like patterns where his legs touch the water are strange; do
a little set of 4-5 of those to explore better patterns." This section is the theory and the predictions, written
before any pattern was built; the score is appended below it.

### What is there now, and why it reads as a web

The woodblock's contact foam (`src/style/woodblock.ts`) is a function of two things only: the sea's contact band
(`Contact.near.band`: 1 at a body, 0 at the sea's foam width, negative beyond) and the water's world xz. Today's
pattern, now named `ridge`, draws a collar where the band is over 0.6 and "fingers" where `abs(noise2(xz * 3.4))` is
under 0.3 of the band. The zero set of a smooth noise is a set of closed wandering curves about one lattice cell
(0.29 world units) across, so a thin neighbourhood of it is a net, not fingers: nothing in it points away from the
body. Checked against the picture (`game` camera, t = 3.6, feet enlarged three times): filaments 1 to 2 px wide
enclosing cells about 10 px across, with 1 px specks round them. The diagnosis holds.

### The instrument: what the band is, measured before designing on it

One debug render (the band printed as grey, not kept): from the `game` camera the band's positive part round a foot
is a blob 60 to 90 px wide and about 40 px tall; it is smooth where it is over about 0.3 and grainy below that
(radial hatching: the band is a minimum over 16 taps on a ring that turns from pixel to pixel). From the `legs`
camera, at water level, the same region is a sliver about 100 px wide and 4 to 8 px tall. Numbers that follow from
the cameras: the crab's sea has foam width 0.85; a near-side foot is 36.7 from the `game` eye, so 35 px per world
unit, the band is 30 px wide across the screen and 22 px along the view (the eye is 47 degrees up); the far feet 28 px
per unit. From `legs` 80 px per unit, so the ring is at its 40 px cap and the band is 3.8 px deep.

Three consequences every pattern below is designed on:

1. **Everything happens inside 30 px of the body at game size.** A feature must be 2 px or more to survive, so a
   pattern has room for about three things across the band (three rings, a 12 px collar and a 12 px tooth).
2. **No direction from the band's gradient.** The brief's first idea takes the fingers' direction from the band's
   gradient. A screen derivative of the band differences two pixels whose rings are turned differently; below 0.3 the
   grain is larger than the slope (1/30 per pixel), and fingers are exactly what lives there. Rejected before
   building. Direction has to come from somewhere smooth: the band's level sets (a threshold that dips makes a tooth
   that must point down the band), a fixed world direction, or the sea's own printed lines.
3. **Hard edges belong above band 0.2**; below it only things that tolerate grain (dots, a line thinning to nothing).

A theorem used twice below (DERIVATION, mine). Let the foam be the set band > T(xz). If T varies along one world
direction only, T = g(n . xz), then along any line parallel to the level lines of g the set is {band > constant}, which
contains the body's edge whenever it is not empty: every piece of foam is joined to the collar along that line. No
islands and no closed loops, whatever the amplitude, so teeth may be as long as the band is wide. A T that varies in
two directions has no such guarantee: its teeth pinch off once its slope exceeds the band's (amplitude over period
greater than 1 over the foam width). A union of such sets keeps the property.

### The six patterns (today's and five)

All sizes are at the `game` camera, 1280 x 800, at a near-side foot (35 px per unit). P is a period in world units.

| pattern | what it is | at a foot, predicted | smallest feature | inherited / mine |
|---|---|---|---|---|
| `ridge` (today) | collar, filaments on the zero lines of a noise, specks | a net of 1-2 px filaments, 10 px meshes | 1 px specks: do not survive as shapes | the existing code |
| `claws` | a cream collar whose edge is thorns: three families of parallel teeth 60 degrees apart (each T = c + a tri(n . xz / P), unioned), concave-sided, a key line round the whole | an irregular star, 12 to 18 tips, tips 6 to 10 px long, 10 px apart; collar 10 px | the tips (sub-pixel needles that fade); teeth 3 px wide half way | the claw from Hokusai and from the crash foam (`examples/bosses/lib/impact.ts`); the three-comb union and its no-island argument are mine |
| `rings` | three thin cream arcs on the band's level lines, broken, moving outward and thinning to nothing, no fill | arcs 9 px apart (7 up the screen), 3 px wide at the foot thinning to 1; the outermost ragged from the grain | the outer arc, 1 px | Hiroshige's ripple lines round posts and boats; the brief |
| `apron` | one flat cream shape with a scalloped edge and one key line: T rises with the distance to a square lattice of points (period 0.36), so the edge is round lobes meeting in cusps | 45 px wide, 33 tall, about 11 lobes 12 px wide and 5 deep, key line 1.5 px; slope 0.71 of the band's, so no islands | the key line, 1.5 px | the scalloped foam edge of the prints (seigaiha scales); the lattice-distance threshold is the plain way to get it |
| `stipple` | printed dots only, two lattices (periods 0.17 and 0.105), each dot's radius growing with the band past its own hashed threshold | 40 to 60 dots within 20 px of the foot, 2 to 4 px across at the body, 1 px at the edge | the outer dots, 1 px: they fade, they do not flicker | the dotted spray of the Great Wave; today's `specks` |
| `breakers` | a narrow collar, and wherever one of the sea's own printed swell lines comes within the band it breaks: the line swells into a cream blade along itself, tapering away from the body (T = c - a (1 - d / dw), d the distance to the line the sea already computes) | collar 9 px; a spindle through the foot along the swell line, 11 px wide at the foot and reaching 27 px out each side, present about 2 s of every 2.9 s as lines pass (lines 2.6 apart moving 0.9 a second) | the blade tips | mine: the foam's direction comes from the print's own wave lines, so it moves with the sea instead of on its own |

From the `legs` camera every pattern is a strip 4 to 8 px tall: `apron` and `claws` a solid cream strip with a line
under it, `breakers` the same but thinner with a blade lying along it now and then, `rings` and `stipple` a pale
broken strip (their features are under a pixel tall there and average out; they must do so without shimmer). Along the
island's waterline (a long edge) `apron`, `stipple` and `breakers` should be at their best and `rings` at its weakest
(parallel lines beside a long wall read as stripes). Round the ship (a 3-unit hull) all five have room.

In motion: every pattern but `rings` and `breakers` is fixed in the world, so round a planted foot it stands still.
`rings` drift outward 3.7 px a second; `breakers` slide with the swell lines at 0.9 units (31 px) a second. A foot
that lifts takes its foam with it in every pattern, today's included (the band is the 3D distance to the body, so it
falls as the foot rises: the foam shrinks in over a few frames, it does not pop). Expected failure: the edges that
sit below band 0.2 crawl (the ring's turn is fixed to the screen, the body moves under it).

The fifth idea in the brief, foam that follows the walk, is not built: the style sees only the band, which has no
clock and no body. A footfall belongs to the sea's `marks` (a deformation the rig drives), which is where the slam's
ring wave is being put.

### Predicted cost

Today's foam is priced in `docs/gpu_native.md` ("The sea"): the style's contact foam was 0.16 ms over the whole sea
beside 0.33 for the band, and near quads now cost 0.20 ms a frame in total, so the pattern is about 0.065 ms of the
frame (one timer tick), of which the gradient noise is about 0.04. No new pattern evaluates a gradient noise; `stipple`
has two cell hashes (today: one), the others none. Predicted difference from `ridge`, fast clock state:

| pattern | at 1280 x 800 | at 2560 x 1600 (four times the pixels, to rise above the tick) |
|---|---|---|
| `claws` | 0 or -1 tick | -0.16 ms |
| `rings` | 0 or -1 tick | -0.20 ms |
| `apron` | -1 tick | -0.20 ms |
| `stipple` | 0 or -1 tick | -0.13 ms |
| `breakers` | -1 tick | -0.22 ms |

Wrong if any pattern is slower than `ridge` at 2560 x 1600, or if the five differ from each other by more than
0.13 ms there. None needs a wider band or any read the band does not already make.

Predicted ranking, before looking: `apron`, `breakers`, `rings`, `claws`, `stipple`.

### Scored (the same day, after building; merged with main 8edf3f2)

Pictures (git-ignored, under `out/foam/`): `sheet.webp` (a row a pattern: the feet from `game` at true size and doubled,
the `legs` camera, the second boat's hull, the island's waterline; the round 4 reference under `curls`),
`hard_cases.webp` (the thrown hull landing from `arena` at t = 9.4, the second boat beside the slam's ring at 12.5),
one six-second `game` clip a pattern. Islander, t = 1.0, 1280 x 800.

**Most of the theory above was wrong, and the instrument was why.** In order:

1. **The band is not a distance.** I read one debug picture as "smooth above 0.3, grainy below". It is two plateaus
   (about 0.45 where the inner ring of taps hits the body, about 0 where only the outer does, 1 only where the sunk
   body lies right behind the pixel) with hashed grain at every step between them, and holes round a thin leg (a ring
   of 8 taps 23 px apart misses a leg 10 px wide about one pixel in three). Every pattern as first built on it failed
   at a glance: the apron was a dot at the foot, the rings a dotted smear, the breakers a stub. A lobe collar cut from
   the band alone had a furred edge. So nothing here except `ridge` reads the band.
2. **What the patterns needed, and what it costs.** A closeness without grain or plateaus. The sea now gives one
   beside the band, `Contact.near.level` (`loft_contact_level`, `src/geom/sea.ts`): the four pixels of a 2 x 2 quad
   turn the ring by four fixed quarter steps and share the least gap (two fine derivatives; which pixel is which is
   read from the derivative of the pixel's own parity), four more taps run along the direction of the nearest hit, the
   gap is measured across the water with height counted double beyond a quarter of the width (a tap that lands a
   little way up a leg reads the distance to where the leg stands, and a belly over the water is far), and the quad's
   value is tilted toward the hit so it varies from pixel to pixel. 21 depth reads near a body against the band's 17,
   9 elsewhere (the inner ring is skipped wherever the clearance map says it cannot see anything: exact, the hashes
   did not move). No read is farther from the pixel than the band's, the `clear` case is untouched, and a style that
   does not read `level` does not pay for it. It is mine; the quad-shared sample pattern is the old interleaved
   sampling idea, the rest follows from what the taps are.
3. **The level's own lines are still rough by a pixel or two** (its radial steps are a sixteenth of the ring). A
   threshold on it (the first apron, the rings) has a ragged edge; a shape drawn in the world and only *sized* by the
   level is clean. So the apron and the collars became round lobes on a lattice whose radius grows with the level.
4. **The no-island theorem was false as stated.** A line of constant n . xz that passes beside the body without
   meeting it still crosses the band, and the set on it is a chord, joined to nothing. Three combs of such chords
   printed a lattice of six-pointed stars round each foot: a web again. `claws` is not in the library.
5. **The band's gradient** was rightly refused; the level's is no better (it is one value a quad).

| pattern | predicted at a foot (`game`) | seen | verdict |
|---|---|---|---|
| `ridge` (today) | a net of 1-2 px filaments | that | the web the owner named; hull and island the same |
| `curls` | (not predicted: added after the round 4 pictures) | a cream collar about 50 px wide and 40 tall, 6 to 8 lobes of 10 to 13 px each with its key line, no curls drawn at this size (by design, under 18 px a cell), a faint arc beyond; close up (the island from `back`, the hull) a band of lobes lying over each other, about half with a curled head cut out of the cream | reads as rolling foam at a glance and holds from every camera; but the lobes are near equal circles, so close up it is pebbles in a row where the reference is an irregular mass with hooked heads |
| `apron` | 45 x 33 px, 11 lobes 12 px wide, key line 1.5 px, built as a threshold | 50 x 40 px, a flat cloud-edged shape; built from the lobes, not the threshold; the key line is there (2 px) and nearly invisible, black on indigo | the strongest read at game size and the plainest; a little lumpy at the hull |
| `rings` | three arcs 9 px apart, 3 px thinning to 1, the outer ragged | two or three broken arcs, 1.5 to 2.5 px, all a little ragged | reads as ripples from `game`; rough close up; nothing at the foot itself, so the leg does not look wet |
| `stipple` | 40 to 60 dots, 2 to 4 px | about that, 1 to 3 px | glitter: too fine to read as foam from `game`, the weakest |
| `breakers` | a 9 px collar and a spindle 27 px long through the foot | a low collar of small lobes and now and then a spike 10 to 15 px long | the blades cannot be longer than the foam is wide (0.85), so they are stubs; a quiet splash, not the idea |

From `legs` every pattern is the strip predicted, 4 to 8 px tall: solid for `curls` and `apron` (with the key line under
it), broken for the rest. Beside the slam's ring and the landing splash (both cream with a key line) `curls` and
`apron` sit as the same ink and the same kind of edge; no pattern fights them.

In motion (the clips; consecutive frames laid side by side for `curls` and `apron`): the lobes are fixed in the world
and grow and shrink with the level as a foot moves, so the collar follows the foot without sliding its lobes; nothing
pops between frames. Not measured as a number: a count of cream pixels changing per frame was dominated by the legs'
own cream bands and is not reported. The rings drift as designed; the faint arcs outside the `curls` collar are the
part most likely to shimmer.

Cost, `profile` (normal frame, least of 40 frames), islander, `game`. The machine was heavily loaded all session
(load average over 20, another process on the GPU); `tools/sea_bench.ts` could not resolve anything (rounds from 3.5 to
14 ms), so these are medians of four `profile` runs at 2560 x 1600 and of the slow-state runs at 1280 x 800.

| pattern | predicted, 2560 x 1600 | measured, 2560 x 1600 (ridge 5.24 ms) | 1280 x 800, slow state (ridge 4.85 ms; sea draw 1.56) |
|---|---|---|---|
| `curls` | - | +0.40 ms | +0.39 (sea 1.90) |
| `apron` | -0.20 ms | +0.27 | +0.23 (sea 1.74) |
| `rings` | -0.20 | +0.20 | +0.10 (sea 1.64) |
| `stipple` | -0.13 | +0.20 | +0.17 (sea 1.70) |
| `breakers` | -0.22 | +0.27 | +0.20 (sea 1.76) |

Every cost prediction was wrong in sign: I priced the patterns' arithmetic and predicted all five cheaper than `ridge`;
all are dearer, by 0.05 to 0.10 ms of a fast-state 1280 x 800 frame (4% to 8% of the frame in either state), because
they read the level and the level reads more than the band (which skips its inner ring unless the outer one found
something; the level may not, see the comment at the code). `curls` adds four cell hashes and an arctangent to that.
Not measured: a fast-state base frame after the merge (the fast readings that day were 1.9 to 2.4 ms for the new
patterns and none for `ridge`).

**Ranking: `curls`, `apron`, `rings`, `breakers`, `stipple`; `ridge` last.** (Predicted: apron, breakers, rings, claws,
stipple.) I would ship `curls`: it is the one that looks like the pictures the owner chose, at game size and close,
and it is the same cream and key line as the slam's ring. If its 0.1 ms is too much, `apron` is the same collar
without the lines for 0.07.

Weak or unfinished:

- `curls` against the reference: equal round lobes, where the reference has a few large hooked heads on a mass. A
  wider range of sizes was tried and made loose bubbles (the lobes stopped overlapping). What it wants is a second,
  coarser layer of lobes in front and a head that is a hook, not a circle with a spiral cut in it.
- The key line round a foam shape is black on indigo and hardly shows; inside `curls` (black on cream) it does.
- The level is a property of the sea, used by one style. The toon and papercraft foams read the grainy band still.
- The patterns' sizes are world lengths suited to a foam width near 0.85 (the crab's). The other boss scenes use the
  default 0.3 and were not rendered with any new pattern.
- The crab's `foam` slot multiplies `loft check --combinations` by six for no gain (the checks do not read the style).
  It is there to compare patterns from the command line; once one is chosen it should become the scene's own option.
- GPU timing under this load is good to about a tick; the order of the five costs is not established, only that all
  are above `ridge`.
