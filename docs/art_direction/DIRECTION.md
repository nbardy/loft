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
little better." So: a new back, `headland` (`examples/bosses/titan/back/headland.ts`), and the `islander` variant
picks it. The base back, the lighthouse back and every other variant are as they were.

What was wrong with the islander's island, looked at from `game`, `far` and `back`:

- it is a drum: a stone wall 4.65 tall standing on the red carapace, capped green. Stone meets shell on one smooth arc;
- its coast is one lumpy circle (lumps of 8%): nothing on it has a name;
- the fort's tower is the one vertical, on the crown, behind the great claw from the game camera.

### Amended before the first render: the owner's reference pictures

After the plan below was first written (commit 929c00d: the point on the right flank, 4.2 long, a ramp instead of a
cliff at the nape) and before anything was drawn, the coordinator passed on reference pictures the owner called
"incredible" (`crab_round4/turnaround.webp`, `game_view.webp`, `stalk_hybrid.webp`, on main) with two instructions: the
light stands on the side away from the great claw, behind the face and never over it; and the island's front rim stays
clear of the head, with room for two long antennae sweeping up and back (another worker's face). The pictures show a
low green dome with round trees, a ring of pale cliff with vertical strata, the shell's red rising into the cliff's
foot in scallops, and a small light on a rocky outcrop at the far edge, lower than the crown, leaning out.

That overturns my first choice of side, and my reason for it was half wrong. I had ruled the left out because a light
there leaves the top of the `game` frame. That is true of the light I had planned (4.4 tall, 4.2 out). It is not true
of a smaller light on a shorter point whose foot is lower than the crown, which is what the pictures show. What I had
right: the frame's top edge is the constraint on that side, and it sets the point's length.

### What is built, and why there

**One shell.** The island no longer stands on the carapace as a drum: its lowest tier *is* shell. Round the sides and
the tail the island's foot is a brick tier, bulging like the carapace's own flank, with the carapace's rim line carried
round it at the carapace's rim height, so the shell reads as running on under the whole island (the hermit's shell).
The cliff stands on that tier's shoulder; over the carapace itself the cliff's foot follows the carapace's dome. Two
boundaries, neither a straight seam: moss over stone in hanging tongues with roots below them; and brick rising into
the stone in scallops of unequal height, inked, with the true crossing of cliff and shell hidden under the brick. The
cap is a low dome, not a flat plateau.

**The point, on the left flank, in the tail quarter** (120 degrees round from the bow toward the left, the small
claw's side). Why there:

- the owner's pictures and instruction: away from the great claw, behind the face;
- it must miss the left legs (the last foot is at x = -2.6, z = -9; the last root at x = -0.4) and the ship, which is
  taken ahead-left and (another worker, this week) thrown up and out from there. Behind the last left leg both are far:
  the tip is 4 from that leg at rest, and the body's yaw toward the Grab (0.195 rad) carries the point further aft;
- from the `game` camera the left flank is the far side, at the top of the frame. A point at height h and r from the
  island's centre along this bearing projects about 14 px higher per unit of r. The light's top must stay inside the
  frame through the slam (when the shell pitches the tail up by about a unit, 30 px). That fixes it: foot 6.1 out, 0.9
  above the body's origin (lower than the crown by 2.4), 3.5 tall: top at y = 37 of 800;
- the point itself is hidden behind the island from that camera unless the cap comes down to it. So the cap tilts: from
  1.7 out along the bearing the dome falls to the point's 0.9, a green ramp the camera looks down, with the stair on
  it. No tree stands in the sight corridor from the light toward the `game` camera.

**The fort goes**: its tower was the island's one vertical and the light replaces it. What stays on the crown is the
ruin's footings: broken wall stubs and rubble under 0.9 tall. One thing the eye goes to on the island.

**Coast.** The shore is a shell rim all round, and rock breaks over it into the sea in three places: the point (left,
tail quarter) and two short horns on the right in the tail quarter (108 and 144 degrees), behind the last right leg and
facing the `game` camera, with a cove and a pale beach between them. Named features: the point with the light; the
stair down the ramp to the light's door; the cove with its beach; skerries (stacks off each horn and off the point);
a reef shelf (a pale ledge at the waterline) round the feet of the horns and the point; and the scalloped shell rim
itself, carrying coral and barnacles on its shoulder.

**The front rim** is behind the base's (on the centre line the cliff's foot is at x = 2.36 and the base's at 2.99;
I first wrote 1.9 and 2.4 here from the outline's mean radius and forgot its lumps, which put 12.6% on at the bow), no
tree stands forward of x = -0.5 within 2.5 of the middle line, and nothing of the island rises above the dome there:
the head and the antennae have more room than they had. Barnacles sit on the carapace up to 0.9 ahead of the foot.

Inherited: the base atoll (its closed-form profile surface, moss tufts, vertical strata strokes, coral heads, trees,
the ruin's stones, the copies list); the fiddler's lighthouse and gulls (`lightOn`, back/lighthouse.ts); the round 4
reference pictures (the dome, the scallops, the light low on an outcrop at the far edge). New: the shell tier under the
island with the carapace's rim line carried round it; capes as an inset family of half-ellipses about the island's
centre joined to the coast by a smooth maximum (every layer of the profile gets a true inset, so cliffs keep their
width on a cape's flanks); the cap tilted to the point so the game camera sees the light's foot; the coast features.

### Predictions (written before the first render of this back)

Crab coordinates (x forward, y up, z right; the water is at y = -1.35). `game`, `far` and `back` at 1280 x 800, body
at rest. Pixel positions are projections of the planned points through the design's cameras.

1. **The point.** Its tip is 2.3 beyond the island's rim along its bearing (6.9 from the island's centre), at
   (-5.45, -5.98): 1.9 beyond the carapace's flank (z = -4.1). Its top is at 0.9. From `game` it runs away from the
   camera, so rim to tip is only about 22 px ((634, 118) to (654, 126)); from `far` 34 px; from `back` 128 px.
2. **The light.** Tower 3.0, lantern 0.5: 3.5 tall (the fiddler's: 7.4), 1.0 across at the foot, leaning 16 degrees
   out along the point. From `game`: 102 px, foot (647, 139), top (656, 37), wholly visible over the cap, the highest
   thing in the picture but for the raised claw; 130 px above the eyes ((698, 271), (764, 240)) and never over them.
   From `far` 92 px; from `back` 177 px, wholly in frame ((287, 411) to (189, 263)).
3. **Readable from `game`**, with the smallest dimension of each: the light on its point (31 px across at the foot);
   the cove's beach (1.6 by 1.1: about 50 by 25 px, at (436, 295)); skerries (stacks 0.9 and 0.7 across: 29 and 21 px,
   foam round each). I expect these three to be named at a glance. The stair (0.24 wide: 7 px, 110 px long) will read
   as a path to the light; the reef shelf (0.3 wide: 9 px) as a pale line, perhaps not named.
4. **The seam.** Brick meets stone on a line of about 22 scallops round the island, each 0.3 to 0.55 tall (18 to 33 px
   in `back`): nobody can draw it as one line. No stone face taller than 1.9 (the base's is 4.65).
5. **Clear of the crab.** At no time in the 16 s cycle does the point, the light, a horn or a gull's circle intersect
   a claw, a leg or the ship. Nearest approach: horn A (108 degrees) to the last right leg, 1.0 at the Grab's full yaw.
6. **Checks and pictures.** The islander passes its 7 checks with the eyes' measured sizes unchanged (59.0, 42.5 px).
   The base variant's three `game` hashes are unchanged.
7. **Cost** (`game`, 1280 x 800, t = 1; before: 90 draws, 106,003 triangles, 2.10 ms GPU in the fast clock state).
   After: 100 draws (the fort's tower leaves, 2; the tower, gallery and lantern arrive, 6; three gulls, 6), about
   124,600 triangles (the island's grid grows from 48 x 96 to 52 x 144, twice with its outline: +11,500; the light and
   gulls +7,650; about 50 more copies in the reef list, +600; the tower -1,150), and at most 2.35 ms (+0.25: the copies
   +0.08, the strands +0.10, the island's paint and grid +0.07). The carapace's paint: unresolved by the timer.

### Scored (the same day, after the build, on main at 8edf3f2 merged in: the throw, the ring wave, the 22 s loop)

Pictures: `out/headland/before_after_sheet.webp` (the islander with the base back above, with this one below, through
far, back, game, face), `turnaround.webp` (four views round it), `islander_headland_game_cycle.mp4` and
`islander_headland_back_cycle.mp4` (one 22 s cycle each).

| | predicted | measured |
|---|---|---|
| the point's tip beyond the rim | 2.3 (as built: a number) | 2.3; from `game` it is foreshortened to nothing, as predicted (22 px) |
| the light, `game` | 102 px, top at y = 37, wholly visible | about 109 px, top at y = 38 (read off the picture), wholly visible, foot and all |
| the light, `back` | 177 px, wholly in frame | about 174 px, in frame through the cycle |
| the light, `far` | 92 px | its top 65 px show; the rest is behind the resting great claw |
| named at a glance from `game` | the light, the beach, the skerries | the light and the skerries; the beach is a pale wedge of about 55 by 30 px with the last right leg across its foot, and reads as sand only once seen from `back` |
| the stair, the reef shelf from `game` | a path; a pale line | neither is seen from `game`: the stair is 7 px wide beside the light, the shelf is on the far side. Both read from `back` and the turnaround |
| scallops | about 22 round, 0.3 to 0.55 tall | 22 round, 0.13 to 0.93 tall (I sized them in the profile's coordinate and did not convert) |
| tallest stone face | 1.9 | 1.8 on the island's own cliff; but the point's flank at its root is about 3, and a horn's rock runs 2.3 from the cliff's foot to the water |
| clear of the crab | nearest 1.0 (horn to last right leg) | no intersection in 12 frames of each clip, the top camera at six times, or the turnaround; the distance was looked at, not measured |
| the thrown ship | (not predicted: the throw arrived with the merge) | `fliesClear` measures 7 targets for this back (the carapace, the atoll, the tier's rim, three capes, the light); the nearest is still the carapace at 2.22; by arithmetic the point's cylinder is about 8 from the flight |
| checks | 7 of 7, eyes unchanged | 12 of 12 (the merge added five); eyes 58.1 and 42.6 px with either back |
| base `game` hashes | unchanged | unchanged against main's own: `ec8c704f392bb4dd`, `d5a61b284211db4b`, `5d759b54f56d14dc` (t = 1.0, 3.6, 11.3) |
| draws | +10 | +10 (116 to 126) |
| triangles drawn | +18,600 | +13,656 (121,681 to 135,337) |
| GPU frame | +0.25 ms on 2.10 (12%) | +0.52 to +0.59 ms on 4.85 in the slow clock state (11 to 12%), three pairs back to back; in the fast state only unpaired readings (1.51 with this back; 1.18 and 1.90 with the base back, hours apart), so no fast figure |

Where the reasoning was wrong, beyond the side (above):

- **The horns were two more drums.** I gave them the point's whole profile, so each carried the cliff out to its tip, and
  from `back` the tail of the island was three stone walls. They are aprons now: low rock from the cliff's foot out
  over the rim. That made the shore a sum of four kinds (shell, cape, apron, beach) with one dispatcher.
- **The ruin's footings hid the light.** I kept a ring of wall stubs on the knoll for the "cream ruin"; from `game` they
  stood in a row across the light's foot and the stair. Only rubble is left. The same sight line now governs the trees.
- **The paint, not the grid, is the island's cost.** I priced the island's inks at the base atoll's plus a little. The
  first build's draw cost four times the atoll's; with a flat ink it cost less than the atoll. Every pixel computes
  every zone's pattern (there is no branch in a lit paint), and I had six ovals and seven sines of the bearing in it.
  Those went into two static tables; it is now about 2.7 times the atoll's (0.51 against 0.19 ms, slow state).
- **Barnacles read as mushrooms** from the face camera: the dark crater was a third of the cone.
- **The `back` camera.** The cove sits at the frame's bottom edge and half the beach is below it. I chose the horns'
  bearings for the `game` camera and did not project the beach into `back`.

Weak or unfinished:

- From `game` the light stands at the island's far edge with the cap running down to it; that it is on a *point* is
  seen from `back`, `far` only in part, and the turnaround's left views. The frame's top edge leaves 38 px over it.
- From `far` the old fort's tower was a bigger vertical than the light is.
- The cream belly under the tier's rim matches the carapace but from behind reads as a pale plate under the island.
- The left half of the cap has only two small trees (the sight lines to the light cross it).
- The gulls' circles leave the top of the `game` frame.
- `shellTop` (the cliff's foot under the bow) copies the base hull's section by hand; another carapace would not be
  followed. The legs come out through the brick tier, as they came out through the stone.
- The light rocks after the slam about the fiddler's axes, not along the point.


## Round 4: the fiddler's and the spider crab's faces (2026-11-12)

The owner, after the islander: "did we refine other crabs' gestures and faces?" We had not. The islander's face is a
score of six expressions; the fiddler and the spider crab still wore first-pass faces (`face/periscope.ts`,
`face/patient.ts`) with two states each (roused, braced). All four crabs will stand side by side in one video, so each
needs a face and a temperament of its own. This round is those two faces. Their bodies' gestures (how each slams,
throws and taunts) are another worker's.

References, made for this (`crab_round4/others/`): `fiddler_expressions.webp`, `fiddler_turnaround.webp`,
`fiddler_taunt.webp`, `stilt_expressions.webp`, `stilt_turnaround.webp`, `stilt_taunt.webp`. Six expressions each: at
rest, wanting, straining, triumphant, furious, wincing. What I take from them: the fiddler's sheet has no brow and
almost no lid; everything is the stalks (bent together, straight, spread in a V, one recoiled) and the pupils, and a
mouth of two plates. The spider crab's sheet is lids and brows; its turnaround hangs the eyes from drooping stalks
like bells, and that is the picture one remembers.

What was wrong with the two first faces, on the pictures (`game`, 1280 x 800, main 9b3deee):

- the fiddler's eyes are 34 px across and its stalks are dark on a dark shell; nothing about the face changes from
  far away but the lid;
- the spider crab's eyes are about 35 px, stand on stalks like everyone's, and sit 100 px from the frame's top; its
  brows and mouth are 3 px strokes;
- neither has more than two states, and both move on the base face's clocks: the same blink, the same whip.

### What is built, and why

Both faces are new options beside the old ones, and the variants pick them: `fiddler` and `fiddlerBaseDisc` pick
`face=jitter` (`titan/face/jitter.ts`), `stilt` picks `face=mourner` (`titan/face/mourner.ts`). `periscope` and
`patient` stay as options, untouched (parked; they are also the "before" in the cost comparison, in the same build).

**The fiddler: the jitter.** Manic, twitchy, bug-eyed; a show-off who cannot keep still.

- Bone-white eyes, big black pupils, a glint; no iris. Bigger than the periscope's (radius 0.72 and 0.66, were 0.46)
  on stalks 3.3 and 2.95 tall, 1.3 times the islander's, thin (0.2 to 0.13), vermilion ringed black: the great claw's
  ink, light against the oxblood shell and the sea. Lids are vermilion too and all but absent at rest.
- **The stalks act.** A stalk is a bent spine (`src/geom/bend.ts`, new): its pose is three bends (a lean at the root, a
  bow near the root, a crook near the tip), each two angles, and it keeps its length whatever they are. So an
  expression says "bowed over toward the ship, tips craned" or "one thrown back in an S" in numbers, and the pose of
  the stalks is the expression before a lid moves.
- **The eyes swivel separately.** Each has its own restless clock (`src/geom/restless.ts`, new: `dart`): about every
  0.85 s (right) and 1.15 s (left), at uneven intervals, it jumps to something else (the audience, its own great claw,
  the other boat, the ship, nothing), in about a tenth of a second, and overshoots; its stalk flicks after it; it
  blinks on four jumps in ten, one eye at a time. How much it wanders is a channel (`roam`): all the way at rest,
  hardly at all when it wants the ship.
- It trembles in bursts (two fast sines per bend, each stalk at its own rate), harder as `shake` rises; the stalks
  whip after the slam and pop at the throw.
- Two cream teeth high on the pebble's front (the sheet's two-plate mouth): they part, chatter, splay at the top (a
  grin) or at the bottom (a sneer).
- A flinch is not the same on both sides: the eye nearer the slam shuts and its stalk is thrown back; the other ducks
  and peeks at the damage, half-lidded.

**The spider crab: the mourner.** Patient, melancholy, eerie and slow.

- **The eyes hang.** Each stalk leaves a socket on the bare front of the ball going forward and a little up, and
  droops in a crook (the same bent spine: a crook of 2.0 rad hangs plumb); the eye hangs from its end, 0.85 of its
  radius below the tip, like one more of the shrine's lanterns. From the `game` camera they hang clear of the body
  against the sea, lower in the frame than the old eyes.
- **They swing, and nothing is keyed.** Each eye is a pendulum (`src/geom/pendulum.ts`, new) whose period follows from
  its own length and the world's gravity (the number the thrown ship and the ring wave already use, 5): arms of 1.98
  and 2.17 give 3.95 and 4.14 s. The walk sways the body 0.42 each way every 4.2 s, so the pendulums are driven near
  resonance, and the driven oscillator's steady state is the animation: a swing of 25 degrees each way, 0.87 and
  1.00 s late. Each action jolts the sockets and the eyes swing on after it (the oscillator's impulse response): the
  snatch, the throw, the slam. The left stalk is longer, so the two are never quite in step.
- **Heavy lids.** The upper lid lies at the eye's middle and droops outward; the pupil is small and on the water. Lids
  are straw-brown (between the cream and the sea behind) with a black lash line, so a shut eye is a closed bell.
- **Late and slow.** The eyes read the score 0.2 and 0.45 s behind the fight; a mood comes on over a second or more;
  a blink takes most of a second; the wince begins 0.12 s after the blow and lasts 1.3 s.
- `stiff` is a channel: wrath lifts the stalks and holds the eyes out at the disc on stiff arms (they stop swinging);
  grief lets them hang. Brows and a small mouth on the ball (inherited from `patient`), bolder, driven by the score.

| | the fiddler (`jitter`) | the spider crab (`mourner`) |
|---|---|---|
| at rest | eyes wide, each wandering on its own; stalks a little apart, twitching | lids half down, pupils on the water, the eyes hanging plumb and swinging with the walk |
| wanting (from the Grab's warning) | both stalks bowed hard over toward the ship and craned, pupils swollen; at once | the eyes drawn out toward the ship on lifted stalks, lids raised; 0.6 s late, over 1.5 s |
| straining (the hold) | screwed shut, stalks ramrod straight, short and thick, shaking; teeth clenched; bites with each squeeze | shut tight, lids knotted to the middle, stalks drawn in and curled under; a slow shudder; it endures |
| triumphant (from the let-go) | stalks shot up and apart, eyes popped 1.3 times, a grin; it cackles | it raises its eyes to watch the ship fly; the smallest smile, the grief still in the lids |
| furious (Claw Crush's wind-up) | stalks short, spread in a V and thrust at the disc; lids hard down over pinpoints | the stalks rise and stiffen and hold the eyes at the disc; lids hard down to the middle; gathers through the whole warning |
| wincing (the slam) | the near eye shut on a stalk thrown back, the far one ducked, peeking | a moment after: one eye shut, the other left half open; the stalks let go and the eyes swing far forward |

Inherited: the eyeball with a sliding pupil in an audience-facing socket, the lid as a spherical lune, expressions as a
cast and a score over the fight's clocks, the two checks (all the islander's, Round 3); the driven damped oscillator;
the brows, mouth and horns of `patient`; the references. Mine: a stalk whose bending is the pose; eyes on separate
restless clocks that pick real things to look at; the hanging eye as a pendulum derived from the rig's own sway and
the world's gravity, with stiffness as a channel; a flinch that differs by side; measuring an expression's marks on
the pose alone, so a swing or a wander cannot make two expressions look different to the check.

### Which camera is honest for the spider crab

The crab is 13.5 tall and the `game` camera cuts its shrine and its knees. Its eyes are not cut: they hang at about
6.1 above the water, a third of the way down the frame (predicted at (696, 252) and (794, 205)), and the highest any
eye's top comes in any expression is y = 115 of 800 at rest, about 90 with the swing. The check that an eye is big
enough to read asks what a player sees, and the player has the `game` camera, so the sizes are held there. `far` is
the camera that holds the whole crab, and the four-crab video will be nearer to it than to `game`; from it the eyes
are a tenth smaller (41.7 px against 46.7), and the right eye is held there as well. The `face` camera is a portrait
camera, not the game's: it follows a new number, `faceRise` (0 for the base), because very tall stalks leave its top
and hanging eyes its bottom.

### Amended by the theory, before the first render

`tools/faces_fs_theory.ts` is plain arithmetic on the faces' own numbers (the body at rest, each expression at full
weight, no swing, no wandering). Running it changed four things I had planned:

- the fiddler's stalks stood 1.5 apart and its two eyes touched from the `game` camera (52 px between middles, 51 px
  of radii). Now 1.9 apart and leaning apart: 69 px;
- its "wanting" crossed the tips by 0.3 rad and the eyes overlapped. 0.1;
- both faces put the pupil on the eye's rim when they looked far from the audience (the islander's lesson, not
  applied). The fiddler's socket turn and slide went from 0.3 and 0.45 to 0.2 and 0.32, and its triumph looks half at
  the ship and half at the audience; the spider crab's from 0.25 and 0.5 to 0.15 and 0.3, its sockets face 10 degrees
  below the camera (not 25), and the water it watches is further ahead;
- the fiddler's tremble was 0.055 rad a bend: 2 px at the tip at rest. Now 0.12: about 5 px at rest, 10 straining.

### Predictions (written before the first render of either face)

`game`, 1280 x 800; pixel figures from `tools/faces_fs_theory.ts` unless said.

**The fiddler**

1. **Size.** The right eye is 54 px across and the left 48 (the periscope's: 34). `minScreenSize` will measure each
   within 4 px. Held: 48 and 40.
2. **Light shapes with dark pupils.** In the `game` picture at rest the mean luminance of the right eye's cream
   pixels over the mean of the ring from 1.0 to 1.3 radii round it is at least 3 to 1 (cream on sea, far legs and
   shell; the islander measured 3.56). The pupil at rest is 23 px across.
3. **Expressions.** All six (rest and five) reach at least 0.9 of the face in a cycle. The closest pair by the theory
   is strain and wince at 27.6 px (0.51 of the right eye's width; the limit is 0.25), then rest and strain at 32.9.
   On the moving crab `expressionsRead` will find its closest pair between 18 and 40 px. By eye, from stills at the
   game camera, at least five of the six are told apart at a glance; the pair most likely confused is strain and
   wince (each has a shut red eye).
4. **Where it looks.** At rest (not wandering) the pupils are centred (2 px off); wanting, 20 px toward the ship, at
   the eye's edge, as on the sheet; furious, 15 px straight down. Inside the white in each.
5. **It never rests.** In a 22 s loop the right eye jumps 26 times and the left 19; a jump takes about 0.1 s and
   overshoots by an eighth. At rest the right eye's middle is flicked at least 10 px by a jump and trembles by
   about 5.
6. **Cost.** `fiddler`, `game`: now 123 draws and 105,245 triangles. After: 123 draws (two teeth for two plates, two
   cheeks for the gullet's two), about 107,700 triangles (taller stalks, finer balls, two more lids), about 30 more
   per-frame tables, and one timer tick more GPU time (0.07 ms), at most two (0.13), compared back to back with
   `face=periscope` in one clock state.

**The spider crab**

1. **Size.** The right eye is 47 px across and the left 42 from `game` (the old: about 35); from `far`, 42 and 37.
   Held: 42 and 37 from `game`, 36 (right) from `far`. No eye's top comes above y = 90 in a cycle.
2. **Light shapes with dark pupils.** As for the fiddler, at least 3 to 1 against the ring (cream on sea). The pupil
   at rest is 13 px across and sits 14 px below and right of the eye's middle, inside the white.
3. **Expressions.** All six reach at least 0.9. The closest pair by the theory is rest and wince at 25.1 px (0.54 of
   the right eye's width), then rest and triumph at about 30. Measured on the moving crab, the closest pair will be
   between 16 and 36 px. By eye at least five of six at a glance; the pair most likely confused is rest and triumph
   (its triumph is the smallest smile).
4. **The swing.** Pendulum periods 3.95 s (right) and 4.14 s (left). Walking, each eye swings 25 degrees each way,
   0.87 and 1.00 s behind the body's sway: about 0.9 units, 29 px each way on screen (57 px from end to end, more
   than an eye's width), once every 4.2 s. The slam throws them forward 0.4 rad on the first swing and half that on
   the next. Wrath (stiff 0.85) cuts the swing to 15%.
5. **Cost.** `stilt`, `game`: now 343 draws and 160,933 triangles. After: 345 draws (two cheeks), about 164,200
   triangles, about 25 more per-frame tables, one timer tick more GPU time (0.07 ms), at most two, back to back with
   `face=patient`.

**Both**

- Checks: `fiddler` and `fiddlerBaseDisc` pass 12 (9 and the face's 3); `stilt` passes 13 (9 and the face's 4); the
  rest as they were.
- The base, islander and mask pictures do not change. `game` hashes at t = 1.0, 3.6, 11.3, taken on this worktree
  before any change: base `ec8c704f392bb4dd`, `d5a61b284211db4b`, `5d759b54f56d14dc`; islander `59ef0e317ccf25f4`,
  `2d914b1e89233885`, `1c2f407adf50a56b`; mask `67124c3c355495a1`, `c53b63b41302c2ea`, `9acc5e0e00877f96`.
