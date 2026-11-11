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


## Round 4: the headland (2026-11-11)

The owner, on the islander: "the islander is great, but the lighthouse is nice as well. Can we do island back and make
the lighthouse hang off to one side on a point, and some more coastline, and integrate the island back and shell a
little better." So: a new back, `headland` (`examples/bosses/titan/back/headland.ts`), a carapace that carries it
(`carapace/encrusted.ts`), and the `islander` variant picks both. The base back, the lighthouse back and every other
variant are as they were.

What was wrong with the islander's island, looked at from `game`, `far` and `back`:

- it is a drum: a stone wall 4.65 tall standing on the red carapace, capped green. Stone meets shell on one smooth arc;
- its coast is one lumpy circle (lumps of 8%): nothing on it has a name;
- the fort's tower is the one vertical, on the crown, behind the great claw from the game camera.

### What is built, and why there

**One shell.** The island no longer stands on the carapace: its lowest tier *is* shell. Round the sides and the tail the
island's foot is a brick tier with the carapace's own rim line carried round it at the carapace's rim height, so the
shell reads as running on under the whole island (the hermit's shell). Rock grows on that tier's shoulder. Three
boundaries, none a straight seam: moss over stone in hanging tongues with roots below them; stone over brick as a
crust that drips to the rim in places and lets brick windows show through in others; and at the nape (the front, over
the carapace) no cliff at all, a ramp of turf and rock running down onto the carapace at a shallow angle so the
crossing line wanders, with barnacles and crust patches scattered ahead of it on the red. The cliff's strata are
level lines at a fixed spacing in height, and the brick tier and the carapace's flanks carry growth ridges at the same
heights, so a line can be followed from shell into stone.

**The point, on the right flank, in the tail quarter** (115 degrees round from the bow toward the right). Why:

- the left is impossible from the `game` camera: the island already sits at the top of that frame, and a light of any
  height on the far (left) flank stands above the frame's top edge and directly behind the eyestalks (projected: its
  top at y = -20 of 800, x = 765, the left eye at x = 764). On the right the same light is wholly inside the frame;
- the right is the near side to `game` and `far`, so the spike is as large as it can be;
- the left is the small claw's side, where the ship is taken and (another worker, this week) thrown up and out;
- on the right it must miss the great claw and the legs. At rest the great claw stands forward of the island; cocked
  it is thrown back over the right shoulder, over the island's front (x above -1); the slam is ahead-right. The legs'
  feet are at x = -2.6 and forward. So the free sector is behind the last leg. The body yaws up to 0.195 rad (toward
  the Grab) while the feet stay planted, which carries a point 8 out by 1.56 toward that last foot: at 115 degrees the
  tip (x = -5.7) comes to x = -4.2, a clear unit from the foot at -2.6. At 105 degrees it would touch. Further aft
  than 115 the `back` camera loses the light off its bottom edge (at 119 the tower's foot is on the edge).

**The fort goes**: its tower was the island's one vertical and the light replaces it. What stays on the crown is the
ruin's footings: broken wall stubs and rubble under 0.9 tall. One thing the eye goes to on the island.

**Coast.** Named features, all on the right flank where the game camera sees the shore: the point with the light; a
stair down the point's spine from the plateau to the light's door; skerries (three stacks, shrinking) off the tip; a
cove in the corner between the point and the island with a pale beach at the waterline; a reef shelf (a pale ledge at
the waterline) round the point's foot. Round the tail and the left (seen only from `back`): a second, blunt nose and a
notch, and the brick tier's shoulder carrying coral and barnacles.

Inherited: the base atoll (its closed-form profile surface, moss tufts, coral heads, trees, the ruin's stones, the
copies list); the fiddler's lighthouse and gulls (tower, gallery, lantern, the rock after the slam, the scatter); from
the round 2 pictures, the garden crab's growth hanging over its shell's rim and the fiddler's light leaning away behind.
New: the shell tier under the island with the carapace's rim line carried round it, the cape as an inset family of
ellipses joined to the coast by a smooth maximum (every layer of the profile gets a true inset, so cliffs keep their
width on the cape's flanks), strata and ridges at shared heights, the nape ramp, the coast features.

### Predictions (written before the first render of this back)

Crab coordinates (x forward, y up, z right; the water is at y = -1.35). `game` and `far` at 1280 x 800, body at rest.

1. **The point.** Its tip is 4.2 beyond the island's rim along its bearing (8.8 from the island's centre), at
   (-5.7, 8.0): 3.9 beyond the carapace's flank (z = 4.1). Its top falls from 2.9 at the island to 0.9 at the tip. From
   `game` the rim-to-tip run is 155 px (rim (436, 210), tip (300, 290)); from `far`, 150 px.
2. **The light.** Tower 3.8, lantern 0.6: 4.4 tall, leaning 17 degrees out along the point, its top 0.5 beyond the
   tip. From `game`: 132 px from foot to lantern top (foot (325, 274), top (260, 159)), 43 px across at the foot; from
   `far` 130 px. The fiddler's is 7.4 tall. It is the leftmost thing above the water in both frames, and it is the
   island's highest point on screen.
3. **Readable from `game`**, with the smallest dimension of each: the point and its light (43 px); the skerries
   (stacks 1.1, 0.8 and 0.6 across: 38, 28, 21 px, foam round each); the beach (1.8 by 0.7: about 60 by 17 px). I
   expect these three to be named at a glance. The reef shelf (0.35 wide: 9 px) will read as a pale line along the
   point's foot but may not be named as a reef; the stair (0.22 wide: 6 px, 130 px long) will read as a path.
4. **The seam.** The boundary between island inks (stone, moss) and crab inks (brick) will reverse direction at least
   ten times round the coast the `back` camera sees, wandering at least 0.4 either way (25 px in `back`), where the
   base's is one arc. No stone face taller than 2.1 anywhere on the main island (the base's is 4.65).
5. **Clear of the crab.** At no time in the 16 s cycle does the point, the light or a gull's circle intersect the
   great claw, the small claw, a leg or the ship. Nearest approach: the last right leg's foot, 1.0 from the tip at the
   Grab's full yaw. From `game` the light never overlaps the eyestalks (300 px apart on screen at rest).
6. **Checks and pictures.** The islander passes its 7 checks with the eyes' measured sizes unchanged (59.0, 42.5 px:
   nothing of the island is near them). The base variant's three `game` hashes are unchanged.
7. **Cost** (`game`, 1280 x 800, t = 1; before: 90 draws, 106,003 triangles, 2.10 ms GPU in the fast clock state).
   After: 102 draws (the fort's tower leaves, 2; the tower, gallery and lantern arrive, 6; four gulls, 8), about
   131,000 triangles (the island's grid grows from 48 x 96 to about 56 x 160, twice with its outline: +17,400; the
   light and gulls +8,000; about 60 more copies in the reef list, +700; the tower -1,150), and at most 2.40 ms (+0.25:
   the copies +0.08, the strands +0.10, the island's paint and grid +0.05). The carapace's paint is two more line
   patterns on a draw that costs 0.07 ms: unresolved by the timer.
