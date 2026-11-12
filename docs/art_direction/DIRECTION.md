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
all are dearer, by 0.05 to 0.10 ms of a fast-state 1280 x 800 frame (2% to 8% of the frame in either state), because
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

## Round 4: a face with a head (2026-11-11)

The owner, of the twelve generated pictures in `crab_round3/`: "look at how expressive this is. Can we try modeling his
face more like these, they're incredible, besides the weird diffusion artifacts; they have a more robust head shape and
more expression." Of the dandy: "the moustache is great", "the islander is great". So the islander keeps everything
but its face. The dandy is two eyes on poles; this round's face is a head: brows, eyes set in it, a mouth that opens.
References made for the round: `crab_round4/` (a turnaround, six expressions, a game view, a short-stalk hybrid).

The variant is `bruiser` (the islander's picks with `face: "brow"`); the face is `examples/bosses/titan/face/brow.ts`,
built from parts in `examples/bosses/lib/face_parts.ts` that any creature can use. `dandy` and `islander` are untouched,
so the two faces can be set side by side. `--variant bruiser,face=browStalk` is the same face with its eyes raised 1.1
radii on short thick stalks (the hybrid picture): one number, `stalk`, 0 in the head.

### The face

- **A head.** A boxy superellipsoid (exponent 3) on the front of the carapace, placed and sized from the carapace's
  numbers: 0.61 of its half-width across (5.0 wide on the base shell), 1.68 half-heights up and down (4.2), its middle
  0.56 half-heights behind the shell's front end. Its face is tilted 0.7 rad up. Why 0.7: the game camera stands 40
  degrees up and 49 degrees round to the crab's right; a face's normal tilted up by t meets it at
  0.477 cos t + 0.68 sin t, which is flat from 0.6 to 1.0 rad (0.80 to 0.83). The tilt is not what costs the picture:
  the 49 degrees to the side is (across the face a unit is 29 px, up it 34). I did not turn the head to the camera:
  the head is the front of the shell and the game's camera may move.
- **Relief and paint in the face's own coordinates**: a dish under each eye, a shelf the brows stand on, a muzzle, two
  cheeks that rise and swell with the `cheek` channel, a mouth hollow that deepens with `jaw` while the head's lower
  half drops 0.4. Paint: dark sockets (the white's surround), the maroon mouth with a tongue and a lip line, nostrils,
  furrows that come with a scowl and folds that come with the cheeks.
- **Eyes** of radius 0.85 and 0.72, sunk 0.3 of a radius, facing 15 degrees to the crab's right of the face's normal.
  The iris is truly under the look while the look is within 0.45 of the eye's facing and is held at 0.45 past that, so
  the eye follows the thrown ship behind its ear and still shows iris and pupil. Coral lids, as the dandy's.
- **Brows**: one thick dark bar an eye, riding a sphere round the eye from straight over it (raised) to in front of
  its middle (clamped), each end on its own angle (the slant), always facing the way the eye does.
- **Mouth**: two cream plates like buck teeth at rest; they part to the mouth's corners as pointed tusks. The dandy's
  moustache on the muzzle. Two antennae from the crown, loose.
- **The score** (the loop is 22 s since the throw merged): greedy from the Grab's warning (0 to 3.0), straining
  through the hold (3.0 to 6.1), gleeful from the let-go through the ship's whole flight (6.1 to 9.3) and fading as it
  looks away after the landing, furious through Claw Crush's wind-up (10.55 to 12.0), wincing at the slam (0.5 s),
  then **gloating** at the second boat its wave shoved (12.9 to 16.9), then posing.

Inherited: the references; the dandy's moustache, score, coral lids, sliding iris and audience; the eyeball and the
lid lune from `faceKit`. Mine: the head with its relief, brows as bars on the eyes' spheres, the gaze held at the rim,
the mandible mouth with a jaw that drops, the antennae, and two checks (`spansOnScreen`, `staysInView`).

### Predictions (written before the first render of this face)

From `tools/brow_theory.ts`: plain arithmetic on the face's own numbers, the body at rest, each expression at full
weight, the `game` camera at 1280 x 800.

1. **Size.** The head is 143 px across and 142 px from crown to chin. The right eyeball is 58 px across and the left
   47 (`minScreenSize` within 4 px of each). What shows of each eye when open, white and iris, is about 0.9 of the
   ball across (the socket takes the rest): 52 and 42 px, both above the dandy's smaller eye (38). Irises 26 and 21 px.
2. **Brows.** Each is 19 px thick and about 50 px long, a dark bar: by the inks, cream is 5 times as light as dark
   brick. In the `game` picture at rest the mean luminance of the right eye's white over the mean of its brow will be
   at least 3 to 1.
3. **Mouth.** In the roar (0.3 s before the slam) the mandibles' hinges are 52 px apart (the limit is 40) and the
   hollow behind them is 61 px wide and 37 px tall.
4. **Expressions.** All seven (posing and six) reach at least 0.9 of the face in a loop. For every pair some drawn
   vector (an eye's middle to each end of its brow, to each lid's middle, to its pupil; the mouth's top to each
   mandible's tip) differs by at least a quarter of the right eye's width (14.5 px). The closest pair is greedy and
   gloating at 20.6 px (the left upper lid), then greedy and gleeful at 21.5 (the lower lid), posing and gloating at
   23.4 (the pupil); the farthest gleeful and wincing at 40. Measured on the moving crab (`expressionsRead`) the
   closest pair will be between 14.5 and 27 px. By eye, from stills at the game camera, at least six of the seven are
   told apart at a glance; the pair most likely to be confused is posing and gloating (the same raised brow; they
   differ in where they look and in the cheeks).
5. **Where it looks.** The iris's middle, in shares of the eye's radius, across and up: posing (0.32, 0.18) (at the
   audience, truly); the ship afloat (-0.38, -0.23) and in flight (-0.45, 0.05), both held at the rim; the disc
   (0.26, -0.37). Each is at most 0.45 from the eye's middle and the iris is 0.45, so iris and pupil are always on the
   white.
6. **The claws.** Neither eye's middle nor the mouth is behind either claw at rest (the great claw's nearest edge
   passes about 1 unit, 30 px, to the camera's side of the right eye's line of sight). Through the loop no point is
   hidden for more than 0.45 s at a stretch (the great claw crosses the lines of sight inside its 0.3 s strike; the
   cutter works on the far side of the face from the game camera and never does), under 1 s in all of the 22.
7. **In the head or on short stalks.** In the head will read better from the game camera: raised 1.1 radii the eyes
   are 3% nearer the camera and no larger, their brows leave the head and the head keeps two empty dark dishes. The
   stalked one will have the stronger outline from `far` and from the side.
8. **Cost.** The islander on this tree: 116 draws, 121,681 triangles drawn from `game`, 192 map and 38 scan tables.
   The dandy's face is 22 of those draws and 8,144 triangles. This face is 26 draws and 33,440 triangles (the head
   alone 23,040: a grid of 80 by 72 drawn twice, once as ink hull), so the bruiser: 120 draws, about 147,000 triangles
   drawn, about 208 map tables, 38 to 42 scan tables. GPU frame: one to three timer ticks over the islander in the
   same clock state (0.07 to 0.2 ms): the head's vertex stage (5,900 vertices, each a five-point normal of a relief of
   eight bumps, twice) is the new work; its paint covers about 140 by 140 px.

### Scored (the same day, after the build)

"The face" and the predictions above stand as written before the first render. What changed after looking is under
"What was wrong" below; the numbers in the code are the built ones.

Pictures (`out/brow/`, with a page, `index.html`): `expression_sheet.webp` (each expression from `face` and from `game`
with the head's window at its true size), `against_references.webp` (each render beside its reference picture),
`variants_islander_bruiser.webp` (far, face, game), `in_head_vs_stalks.webp`, `bruiser_game_cycle.mp4` and
`bruiser_face_cycle.mp4` (one 22 s loop each). `loft check` on the bruiser passes 14 of 14: the crab's nine and the
face's five. After main's merge the bruiser carries the headland back, as the islander does.

| | predicted | measured |
|---|---|---|
| right eyeball, `game`, 1280 x 800 | 58 px | 58.2 px (`minScreenSize`) |
| left eyeball | 47 px | 47.8 px |
| white and iris showing, eyes wide (greedy) | 52 and 42 px | 57 and 42 px (`tools/eye_pixels.ts` on the `game` frame); posing 52 (right); furious 54 and 30 |
| eye white over its brow, luminance | at least 3 to 1 | 5.0 to 1 (white 0.861, brow 0.173; posing) |
| the roar's mouth, hinge to hinge | 52 px | 49.0 px as first built; 59.1 px after the gape was widened (`spansOnScreen`; the limit is 40) |
| every expression reaches 0.9 of the face | 7 of 7 | 7 of 7, each 1.00 |
| closest pair | greedy and gloating, 20.6 px, in 14.5 to 27 | greedy and gleeful, 19.8 px as first built and 19.9 px now (the right lower lid; the limit is 14.6). In range; the pair was my second closest (21.5), not my closest |
| told apart by eye at the game camera | at least 6 of 7, posing and gloating the weak pair | 6 of 7 in the sheet's true-size insets, my own judgement; posing and gloating are the pair I would mistake |
| a claw over the face, longest stretch | at most 0.45 s, inside the strike | **wrong**: with the islander's claws the right eye's middle is behind the great claw's palm for 2.05 s at a stretch (2.45 s in all), in the Grab's reach, not the strike. With `fiddlerAside`: 0.05 s (`staysInView`) |
| in the head or on stalks | in the head | in the head (below) |
| draws, triangles drawn, tables | 120, about 147,000, about 208 map and 38 to 42 scan | 120, 147,601, 209 map and 39 scan, on the tree the prediction was made on. With the headland back merged the islander is 126 draws, 135,337 triangles, 202 map and 43 scan tables, and the bruiser 130, 161,625, 219 and 44: 4 draws, 26,288 triangles, 17 map tables more |
| GPU frame | one to three timer ticks over the islander (0.07 to 0.2 ms) | 0.21 ms (three ticks) in the slow clock state: islander 5.44, 5.51, 5.37, 5.44, 5.44, 5.44, bruiser 5.70, 5.64, 5.64, 5.57, 5.70, 5.64, alternated. Not resolved in the fast state (islander 2.23, 2.88, 3.21; bruiser 2.16, 2.49). Every reading was taken with three other workers on the GPU |

Prediction 5 (where the iris sits) was not measured as a number. In the pictures: centred on the game camera when
posing, at the rim toward the ship (afloat, held and thrown 37 units off), low toward the disc; iris and pupil show in
every one, though of the thrown ship's look only about half the iris shows from the game camera, behind the laugh's
raised cheek lid.

**In the head, or on short stalks** (`in_head_vs_stalks.webp`). In the head is shipped. On stalks the eyes are larger
on screen (62 and 52 px of white and iris against 57 and 42) and stand above the head's outline, which I had not
expected; I had said "no larger". But from the game camera the stalks themselves are behind the eyes and not seen, the
right eye stands in front of the left in four of the seven expressions, and the brows leave the head, so the furious V
is two bars in the air and the thing stops reading as a head. From `far` the stalked one does have the stronger outline.

What was wrong, each a fault in the reasoning:

- **The great claw hid the face** (the miss above). I reasoned about the claw's rest pose and its strike. I left out
  that the body turns 0.14 rad toward the Grab, which turns the face away behind a claw that stands on the camera's
  side, and that the claw waves then. At rest the banner stood 15 px from the right eye, which the arithmetic did say
  (about 0.45 of a unit) and I called clear. The fiddler's rest pose is now a parameter (`claws/fiddler.ts`,
  `fiddlerWith`; `fiddler` itself draws the same pictures), and the bruiser picks `fiddlerAside`: the wrist 0.8 further
  back and 1.1 further out, the hand leaning out. Cost: the banner stands less tall beside the head than the islander's.
- **Lids that were not half down.** I wrote a lid's angle as a share of a quarter turn and read the share as cover.
  Seen from in front a lid `a` radians down hides (1 - cos a) / 2: posing's 0.64 was 23%, an open eye. Posing is 1.0
  now (38% and 56%).
- **A raised brow showed its edge.** The first brows lay on the eye's own slope, so raised they faced up the head:
  11 px where the theory, once it measured the bar and not the arc it rides, said 19 facing front. They face front now.
- **The moustache crossed the right eye.** I placed it from the dandy's numbers, on a face whose eyes are 1.7 across
  and sit 0.5 above it. It hangs lower now (droop 0.5, shorter, thicker) and its lift is half what the dandy's is. From
  the `face` camera its near half still crosses the right eye's lower rim in the laugh.
- **The layout was too small for its parts.** Eyes of 1.7 and a mouth 1.2 tall on a head 4.2 tall left nothing for a
  muzzle. The head is 0.68 of the carapace's half-width now (5.6 across), the eyes sit 0.2 higher and the mouth 0.15 lower.
- **Posing was measured mid-blink.** `expressionsRead` takes rest in the middle of its longest stretch; my blink was
  there, so the closest pair came out as posing and wincing at 19.4 px. The blink is a second and a half later now.
  The check does not know about `only` layers.

What the reference still has that this lacks (from `against_references.webp`):

- its eyes are smaller in a heavier head and sit under the brows; these are frog's eyes, 0.4 of the head's width each,
  which is why they read at game size and why the head reads as round, not robust;
- its brows are long ridges of the head itself that sweep down the outside of the eyes and crease the head when they
  move; these are short bars, and the head between them does not move (two furrow lines come and go);
- its cheeks and jowls are separate bulging forms; these are relief on one smooth ball, which flat ink does not show,
  and a fold line;
- its gape takes the whole lower half of the face; this one's hollow is 2.6 wide of 5.6, with narrower tusks;
- its shut eyes are creased lids in the body's red with crow's feet; these are coral domes with a lash line;
- it is mottled all over; this face is kept plain so its features read.

Weak or unfinished:

- Posing and gloating are close: the same raised brow and half lids, told apart by the look and the cheek lid.
- The clench (`grit`) barely reads; straining reads by its brows and slit eyes.
- In the wince the right brow still lies partly on the shut lid.
- The antennae follow their formulas (trail, flick, whip); I looked at them in the clips, not in a frames sheet across the slam.
- The audience is the dandy's fixed point (the `game` camera's eye). If the game's camera orbits it must become an input.
- The islander's own claws (`fiddler`) wave every 8 s, which was twice in the 16 s loop; in the 22 s loop the wave does
  not meet itself at the seam (0.72 before it, 0.27 after). `fiddlerAside` waves twice a loop of any length. Not changed
  for the islander: its pictures are held.
- The headland's barnacles on the nape end where the head's back begins (about x = 2.6 at the middle); none is under it.
- The blink is not checked, and where the iris sits is not checked.


## Round 5: the terraced back (2026-11-12)

The owner, on the headland: "The back looks ugly though. Check those renderings, pass renderings to gpt image and ask
it to make more interesting and seamless ones." The renders went to the image model; its paint-overs of our own
cameras are the design (`crab_round5/back_paintover_*.webp`, `back_model_sheet.webp`; the growth and the light's base
in `crab_round4/growth/`). The owner then chose the spiral shell for the stalk-eyed islander (another worker), so this
back, `terraces` (`examples/bosses/titan/back/terraces.ts`), is the **bruiser's**: the head-and-brows face, the great
claw carried aside. The headland, the base atoll and the fiddler's light are as they were.

What is ugly in the headland, checked against its renders (`out/headland/turnaround.webp`, and the bruiser through
far, back, game, face): it is five rings stacked on one axis (green cap, grey cliff, red tier, cream belly, water),
each closed by a level edge, so every boundary can be drawn as a ring; its cliff strokes are vertical and its tier's
blots have no direction, so no line leads anywhere; the coral and barnacles sit one every 20 degrees; the light is the
smallest thing on it. Nothing is large.

### The form: one screw

What the three paint-overs share, though they disagree on where anything is: from every side, red blades rise from
lower left to upper right and end in terraces. Only one solid looks like that from every side: a screw. So the island
is one closed-form surface whose second coordinate is not the bearing but the **thread**: grid columns are the screw's
lines, and every edge of the form (a plate's lip, the foot of a cliff, a terrace's rim) is a column. Ribs, strata and
turf lines are lines of the same coordinate, so they run on through shell, stone and turf because they cannot do
anything else.

- Hull: a mound, not a mesa (`G` from the crown, 0, to the rim, 1): radius `sin^0.8`, height `cos^1.25` of a quarter
  turn; the crown 1.2 over the atoll's `top` (4.1 in crab y; the headland's cap: 3.35), the rim at the carapace's own
  rim line (-0.19), egg-shaped in plan (0.90 of the atoll's radii at the bow, 1.06 at the tail). Taller because the
  front half of the island stands on the carapace: of a 3.4 flank only what is above y = 1.1 shows toward the game
  camera.
- The screw: thread phase `psi(G) = 1.9 (1 - (1 - G)^2.25)` groups from crown to rim, three groups round. Its lines
  leave the rim straight up and lean over as they climb (about 45 degrees at mid flank), then run half a turn round
  the cap. A group is three plates of unequal width (0.18, 0.20 and 0.36 of the group, each followed by a recessed pale
  band 0.06, 0.06, 0.14): two minor blades and one **broad plate**. So: three broad plates, six blades between them.
- Low on the flank every plate is a shingle: a convex red lobe swelling to a lip (cream edge, key line) that overhangs
  a recessed pale band. Above G = 0.6 the minor blades die into the hull; the broad plate's lip goes on as the rim of a
  **terrace** (level tread, pale riser: the same band, now a cliff), winding in to the crown, where the three meet in
  a pinwheel. Level treads are exact: a tread's height is the hull's at the start of its cell (DERIVATION in the file).
- Red tapers to a point along each lip (to G = 0.42); turf hangs over the blades' upper edges in tongues on the rib
  lines.

Inherited: the paint-overs (blades, bands, terraces, the coast's parts, growth at low seams, barnacle beads at blade
tips); from the headland's code, the profile-with-shores idea, `cape`, `join`, `stations`, the trees, the reef list's
paint dispatch, `keepClear`, the CPU twin for placing things; the fiddler's gallery and gulls. Mine: the screw
coordinate as the surface's own (form and ink from one phase, creases on columns); unequal plates in groups, the broad
one becoming the terrace; the mound hull; where the coast goes and why (below); the growth kit; the light's structure.

### The coast, and where it must go

The point is on the left of the centre line at the tail: bearing 200 degrees (20 degrees left of dead astern), tip 8.4
from the island's centre (3.6 beyond the rim; the headland's: 2.3), top 0.8 above the water at the light. Not at the
headland's 240 degrees: from `game` that is behind the crown's right shoulder and beside the bruiser's antennae; at
200 the light stands against open sea at the island's upper left with 9 units to the nearest foot and 15 to the thrown
ship. Not on the right, where the pictures' game view (which turned the island) puts it: the light stays off the great
claw's side. Round from it toward the right flank: a crescent beach under the arch of the plates (152 to 192 degrees,
sand to 1.2 beyond the rim), then a reef shelf with three stacks (2.3, 1.5 and 0.9 tall at 126, 121 and 117 degrees,
shrinking outward). Two groves: one left of the crown, one forward-right, both behind x = -0.5; open turf between,
and nothing on the sight line from the light to the game camera (it crosses the cap 2.5 from the centre at bearing 130).

The light: tower 3.4 (the headland's 3.0), girth 0.78 to 0.5 (0.5 to 0.34), an intact lantern room and roof 0.9, on
a mound 0.8 over the water; lean 0.34 (19 degrees) out along the point. Its lower 0.36 is crust. Structure for a later
attack, numbers with defaults: `lean` 0.34, `rise` 0 (a segmented neck out of the mound, not drawn at 0), `shutters` 0.

### Predictions (written before the first render; `bun tools/terraces_theory.ts`, bruiser at rest, 1280 x 800)

1. **The light, `game`.** Finial at about (500, 70), mound top at (521, 192): 120 px from mound to finial (the
   headland's whole light: 109), 41 px across at the foot (25); from the water 138 px. Wholly visible, the tallest
   thing but the trees; at the slam the tail pitches up by 1.5 to 1.9 (it is 9 behind the body's origin) and the finial
   stays 15 px or more inside the frame. `far`: 110 px, its lower third behind the aside claw's finger. `back`:
   170 px, in frame; the point's tip 25 px from the bottom edge.
2. **That it stands on a point, `game`.** The point's outer 2 units and the water on both sides of it show beyond the
   island; its root (inside 6.5) is behind the tail's flank. I expect "a lighthouse on a rock off the island's far
   corner" at a glance, the join to the island inferred.
3. **Plates, `game`.** Toward the camera the flank shows from the carapace (y = 1.1) to the shoulder: red blades
   about 50 px tall, 45 to 110 px wide; two broad plates wholly visible, the third in part; above them three terrace
   rims on the cap, each 120 px or more long with a riser 12 to 35 px tall. From `back`: three broad plates, 150 px or
   more along the lip each. Nobody can draw a level ring on the island from any camera: every edge climbs.
4. **The beach.** `game` at rest: mostly behind the aside claw; the point end shows as a pale sliver 40 px long over
   the claw's finger. This is a miss of the brief that I can state before building: the bruiser's claw covers the only
   water the game camera sees beside the tail. It shows whole (82 px by about 20) while the claw is raised for Claw
   Crush. `back`: 206 px, at the frame's bottom edge (the stacks' feet are below it: the `back` camera was framed for
   the atoll). `far`: 76 px.
5. **Growth.** Five clusters and rests, 230 to 280 cones in the reef list, largest at the light's foot; three barnacle
   beads at each broad plate's tip. From `game` a cluster reads as one pale-and-pink patch 25 to 40 px across.
6. **Clear.** The front rim on the centre line at x = 2.15 or less (the headland's 2.36; the head's back is at 2.6);
   no tree forward of x = -0.5. Thrown ship: nearest of my targets 15 away (the point and the light). Last right foot
   to the nearest stack 3.6 at rest; about 1.5 at the Grab's full yaw. Checks 14 of 14 on the bruiser.
7. **Cost against the headland** (`game`, t = 1, on the bruiser): the same 15 draws (island 2, list 1, tower, gallery,
   lantern and shutters 8, two gulls 4; the headland's: island 2, list 1, light 6, three gulls 6); triangles drawn
   within 1,500 of the headland's (grid 46 x 144 against 52 x 144; 120 more copies at 12 each); GPU frame within one
   timer tick of the headland's either way in the slow clock state. Where it goes: the island's paint reads the thread
   and `G` directly (one static table for the shore), about 0.3 ms against the headland's 0.51; the list about 0.55
   against 0.41 (nearly twice the copies, but small ones).
8. **Unchanged.** Base `game` hashes `ec8c704f392bb4dd`, `d5a61b284211db4b`, `5d759b54f56d14dc` (confirmed on this
   branch before starting); fiddler `0cdc48fb9d08d13c` (t = 1), `f306729c859bd6c3` (t = 12.4); the headland itself
   (`bruiser,back=headland`, to be recorded before the refactor that shares its helpers).

### Fifth pass: the screw was the wrong form (a second engineer, after merging main ada7ee2)

The first four passes were not scored in writing. Judged now against the paint-overs, the fourth pass from `back`
(`bruiser`, t = 1): the island reads as a **cinnamon roll**. Nine red stripes of nearly one width wind evenly into a
pinwheel at the crown; every edge is a spiral, so the eye circles and finds no big shape; the ribs are 22 to a group,
close and even, so each stripe is a uniform hatch; the light is small and lies on a white blob of sand; the growth is
white dots spread round the rim. From `game` the island is a green disc behind the claw. What the paint-overs have
that this lacks: (1) a **level turf cap** with two clumps of trees, broad and flat, the biggest single shape; (2) under it
**pale stone cliffs with sparse level strata**; (3) **a few broad plates laid on the cliffs like leaves**, each convex,
wide low and **pointed at its tip** under the turf, unequal in width, with stone between them; (4) a tall light
standing up off a rock at one corner; (5) growth in fistfuls at the plates' feet, none higher up.

The screw's insight was right (every plate leans the same way, so the blades rise lower left to upper right from every
side) and its execution made it a thread; a thread is a uniform pattern, not three shapes. Replaced:

- **Hull** (mine): the surface's v is the bearing again. A level cap to G = 0.42 (falls 0.1 of the island's height,
  reaching half the rim's radius; its edge wanders 0.05 in G), then a cliff falling as `1 - t^1.5`: a rounded shoulder,
  steepest at the water. Crown 1.2 over the atoll's top, as before.
- **Plates** (mine, from the paint-overs): a list of five, each a root bearing at the rim, a sweep (its middle turns
  back 30 to 36 degrees as it climbs: the screw's handedness, kept), a half-width, a tip's G and a thickness. Across a
  plate `x` runs -1 to 1; it stands out of the hull as `sqrt(1 - x^2)`, a little more on its leading edge (a shingle's
  lip), its width a leaf's: widest a quarter of the way up, drawn to a point at the tip. Three broad (half-widths 24 to
  30 degrees) all on the half of the island the game camera sees (bearings 332, 42, 108); two narrow blades on the
  coast's side (166 over the beach, 250 by the point). Unequal gaps: 70, 66 and 224 degrees, the last holding the coast.
- **One form through the materials** (the paint-overs' note): a plate's ribs (7 across it, sparse) run on past its
  red tip as stone flutes, up to the cap; the turf hangs down over the tip in a tongue. Stone elsewhere has level
  strata, two or three to the cliff.
- **Ledges** (the "terraces"): two turf treads cut into the cliff as arcs (not rings), at G 0.6 round bearing 140 and
  0.68 round 330, which the plates cross.
- **The cove** (the paint-overs' "recessed crescent beach"): the rim is pulled in by 0.11 of the radius over the beach
  (176 +- 22) and the sand runs out 1.9 beyond the original line: a crescent set into the island, the rim arched over it.
- **The light**: taller and leaning more (height 3.6, girth 0.78 to 0.54, lantern 1.0 of the fiddler's, lean 0.36); its
  crusted foot is the point's top, with the kit's crust clusters round it. Point, shelf, stacks, groves, growth kit,
  shore code and `keepClear`: inherited from the first build.

#### Predictions for the fifth pass (`bun tools/terraces_theory.ts`; bruiser at rest, 1280 x 800)

1. **Plates, `game`.** Three broad red shapes, rim to tip 138, 146 and 138 px, 57 to 85 px across at their middles,
   leaning one way. The aside claw covers everything below about y = 190 between x = 400 and 670, so the rims of the
   middle and right plates are hidden: I expect each broad plate to show 60 to 100 px of its upper part above the claw,
   tips at y 198 to 246. The first broad plate (root 332, rim at x = 710) shows whole.
2. **The light, `game`.** Mound at (542, 191), finial at (522, 40): 152 px mound to finial (the headland's 109), 42 px
   across its foot; the finial 40 px inside the top edge at rest, 20 or more at the slam's pitch. **`back`**: 242 px, finial
   at (80, 544), in frame by 80 px; the point's tip at (210, 744).
3. **The beach.** `back`: a pale crescent 235 px long and up to 137 px deep from the cove's back to the sand's edge, at the
   frame's bottom. `game`: 106 px end to end, mostly behind the claw (as before: a stated miss of the brief that the
   bruiser's claw causes).
4. **The cap, `back`.** 255 px across between bearings 40 and 220; with the two groves, the largest single shape.
5. **Evenness.** No two neighbouring plates of the same width or gap; no rib spacing under 6 px from `back`.
6. **Clear.** Front rim on the centre line at x = 1.96 (head's back at 2.6); thrown ship 15.2 from the nearest of the
   point, light and left-tail rim; last right foot to stack 1: 5.0. `bruiser` 16 of 16.
7. **Cost.** The same draws and triangles as the fourth pass (grid 46 x 144, the same list within 10%): against the
   headland, within 1,500 triangles; frame within 0.07 ms (one tick) of it in the slow state.
8. **Unchanged.** Base `game` hashes as main's (`0c1b1e4368820ff8`, `0db3c25b4d11121b`, `2315edeb83c975e9`); islander and
   fiddler as taken after the merge, unless the spiral's barnacles are swapped for the kit.

#### Amended while building (before scoring), and why

- The broad plates first sat all on the game camera's half (332, 42, 108). The first render showed the aside claw covers
  that half's lower flank from `game` (x 400 to 670 below y = 190), and every other camera saw bare stone: moved to
  three corners (34, 158 arching over the beach, 272) with blades at 96, 212, 330; leaf widest a fifth of the way up,
  full to near the tip ((1 - s)^0.45 (1 + 0.6 s)); the cliff a straight cone (`cliff` 1.0) so a high camera sees the
  plates' faces. The first leaf (widest a quarter up, then tapering) left the upper cliff bare.
- The point from 206 to 192 and the beach to 164 +- 15: the render put the light's foot behind the cap from `game`
  (the body at t = 1 stands about 0.6 higher than the rest pose the theory used; the finial measured 17 px above its
  prediction). A silhouette test (the theory's hull projected, raised 0.6) gave 190 or less as clear of the cap.
- Crown 1.2 to 0.7 and cream stone with strata 6.5 to the island's height: the paint-overs' island is a low wedge with
  pale, finely lined cliffs; ours read as a bald drum under the turf.
- Light 3.6 tall, girth 0.64 to 0.46, lean 0.24 (0.36 laid it over toward the high cameras).

#### Scored (2026-11-12, at the fifth pass's last commit; bruiser, t = 1, 1280 x 800)

1. **Plates, `game`: missed.** One broad plate shows (the bow-right one, about 90 px, its lower half behind the
   antennae) and one blade; the other two broad plates are on the far side. Wrong in the reasoning: I placed plates for
   the cameras before checking what the bruiser's own claw and antennae cover; there is no bearing where three broad
   plates show from `game` with the claw aside. From `back`: the beach plate 265 px rim to tip, about 330 across; a
   blade 150 px. From the paint-overs' four cameras: two or three broad plates each.
2. **The light, `game`.** Finial at (457, 45) (point at 192; predicted for 206: (522, 49)); mound to finial 125 px
   (predicted 143), 38 px across its foot (42). Its crusted foot shows in white water beside the cap, left of it: it
   reads as a light on rock off the island's corner; the point's rock itself is mostly behind the claw's tip. `back`:
   205 px or more, the foot cut by the frame's bottom edge (predicted 220, in frame by 80: missed, the point moved
   toward that camera).
3. **The beach.** `back`: a pale crescent under the beach plate's arch, about 270 px visible before the frame's edge
   (predicted 235 by 137). `game`: hidden by the claw (as predicted).
4. **The cap, `back`.** 300 px across (predicted 255 with the higher crown).
5. **Evenness.** Plate half-widths 36, 38, 33 and blades 15, 12, 17; gaps between roots 62, 62, 54, 60, 58, 64 degrees:
   **nearly even** in bearing, unequal only in width. A remaining fault; a bunched order (34, 80, 150, 200, 268, 316:
   gaps 46 to 78) is the next thing to try.
6. **Clear.** `bruiser` 16 of 16; the thrown ship's nearest target is the body (2.13), not the island; every variant's
   count as main's; the lineup with the bruiser in the islander's slot 44 of 44.
7. **Cost.** Draws: 131, the same as the headland's (the back's own: island 2, list 1, light 12 with outlines).
   Triangles over the base atoll: +14,372 (headland +13,656; predicted within 1,500: held). The list: 342 copies (the
   first build's 692 halved: 6 seam clusters, lighter recipes). Time, under load (the lighting worker profiled at the
   same time; frames 2.1 to 2.8 ms, between the two clock states): in the one round where both builds sat in the same
   state, the back's own draws 0.354 ms against the headland's 0.314 (+0.04, under one tick: island 0.131 against
   0.157, list 0.223 against 0.157). Other rounds moved ours wholesale to 0.69 to 0.73 (a state change, not like for
   like). Not a clean slow-state reading; the list is about one tick dearer than the headland's.
8. **Unchanged.** Base `0c1b1e4368820ff8`, `0db3c25b4d11121b`, `2315edeb83c975e9` (held); fiddler `247e38d376bb6f99`
   (t = 1), `bbb893662a1c567f` (t = 11.3) (held). Islander changed by the barnacle swap: `175282b767d91ee8` to
   `6bab240b7cfd410a` (t = 1), `9146c0ee531360d8` to `590f10d1a79b8352` (t = 11.3).

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

## White water (2026-11-12)

The owner, on the contact-foam sheet: "this is sick, this style for the white water interaction", of the round 4
reference row (rolling cream foam, curled spiral heads, bold key lines). So that is the style for all white water:
the collar where a body stands in the sea, a slam's burst, the ring wave's crest, a landing. This section is the
design and its predictions, written before building; the score follows it. `curls` (above) is the starting point and
its named gap is the brief: equal circles in a row, where the reference has a few large hooked heads on a rolling mass.

### What white water is made of

One painter, `rollingFoam` (to be `src/style/whitewater.ts`), draws white water from three things an event gives it: a
**mass** (0 where there is none to 1 in the thick of it: the contact level for a collar, a splash's own shape for a
burst, the sea's marks for a wave's crest), a **lean** (a direction on the water the heads curl toward, or none), and a
**head** size (the world radius of the largest curled head). From them, back to front:

1. **the roll**: round lobes on a jittered lattice a third of a head apart, sized by the mass: large and overlapping
   into one cream mass where the mass is high, shrinking outward into separate **droplets**; one key line round the
   mass, and inner arcs only on about half the lobes (every lobe outlined is what read as pebbles);
2. **the heads**: one to a cell of a coarser lattice (2.4 head radii apart), present in about two cells in three, of
   unequal size (half to full). A head is a **hook**, not a circle: its edge is one turn of a spiral, r = R (1 - 0.4 a)
   for a the share of a turn from the beak, so it has a pointed beak overhanging a notch 0.4 R deep; the same spiral
   goes on inward for a turn and a half as a channel of water between two key lines, closing to a line. The beak
   points along the lean (all heads curl the same way); with no lean, by the cell's hash;
3. events with a centre add a **crown**: pointed fingers round it (the angle is known there, so teeth can point
   outward, which the collar's cannot).

The spiral is a level set of u = (1 - r / R) / 0.4 - a: outside the head u < 0, the edge and every inner turn are the
whole numbers, and u steps by exactly 1 across the beak's line, so "on a whole number" is continuous there. Mine; the
hooked wave head itself is Hokusai's and the references'.

It is a set of shared pieces that each drawer calls, not a sum type of events inside the style. An event's drawer
already owns what the painter needs and the style cannot see: the collar is the sea's contact case, the crest is the
sea's marks, a burst is its own patch on the water with its own clock (`examples/bosses/lib/impact.ts`). A dispatcher
in the style would have to be handed all of that through one door. What is a sum type is the choice a scene makes:
the collar's pattern (`ContactFoam` gains `roll`) and how the sea's marks print (`flat`, as now, or `heads`).

What the sea must hand the style that it does not now, both added readouts: the sea's foam width beside the contact
level (so a pattern's sizes follow the scene's sea: the other bosses' is 0.3, the crab's 0.85), the direction the
largest swell travels (the collar's lean), and a lean beside a deformation's marks (the ring wave's is outward).

### Predictions

At the `game` camera, 1280 x 800, a near foot (35 px a unit), the crab's sea (foam width 0.85, so head R = 0.29,
10 px):

| what | predicted |
|---|---|
| collar at a foot, `game` | a cream mass 45 to 55 px wide with **2 or 3 heads** 10 to 20 px across (4 or 5 coarse cells touch the collar, two in three hold a head, sizes half to full), each showing its notch (4 px on the largest) and no inner spiral (drawn only where R is over 9 px); 3 to 6 droplets of 2 to 4 px beyond; one arc |
| collar close (`back`, the island; about 55 px a unit) | heads 16 to 32 px across with a turn and a half of channel 2 to 3 px wide, beaks all toward the swell's travel; lobes behind them with few inner lines |
| along the hull (3 units) | 4 coarse cells: 2 or 3 heads of unequal size over about 10 lobes; big, small, small, big by the hash, not by design |
| slam, first 0.25 of its life | a crown of about 14 pointed fingers round the disc, each 0.3 of the reach long |
| slam, after | a ring of heads (R = 0.5: 35 px across at `game`, spirals visible), beaks outward, thinning to droplets as it fades |
| the ring wave's crest | a line of heads (R = 0.35) one deep along the front, beaks outward, breaking into separate heads and then droplets by three quarters of its reach |
| landing | the slam's crown at the splash's smaller reach (heads R = 0.3) |
| swims or pops | no: every lobe and head is fixed to the water; sizes follow the mass |

Not predicted to work: a bow wave for a moving hull needs the hull's velocity, which neither the style nor the sea has;
it would be a drawer of its own fed by the rig. Built last if at all.

Where the reasoning is most likely wrong: a head is sized by the mass at each pixel, not at its centre, so a large head
across a steep mass is squeezed on its far side and its spiral with it; and the heads sit wherever the lattice puts
them, not on the mass's edge, where the reference has them.

### Budget

- The collar: no more than `curls` costs, +0.10 ms a frame over `ridge` at 1280 x 800 (measured before). By count it is
  `curls` plus one cell hash (the heads' lattice is searched one cell deep; `curls` has four hashes and an arctangent,
  this five and one): predicted equal within what can be measured.
- A burst (`impactFoam`): today one gradient noise and three edges over its patch while live; the painter is five
  hashes and an arctangent there: predicted up to twice the patch's cost for the 1.4 s it lives, under 0.05 ms.
- The crest: today one threshold on a number the sea already has. Anything drawn costs more than that; the budget
  cannot be "no dearer". To keep it small the crest gets the heads only (one hash, one arctangent), no roll:
  predicted under 0.05 ms while the pulse is on the screen, nothing otherwise.
- How it will be measured, with other workers on the GPU: `profile` of old and new built in turn (A, B, A, B, six of
  each, 2560 x 1600 and 1280 x 800), the median and the whole range reported; a difference smaller than the range
  between runs of one build is reported as not resolved, with no number.

### Scored (2026-11-12, after building; merged with main 9f7d0e2)

Pictures (git-ignored, `out/water/`): `sheet_white_water.webp` (ours beside each reference: the collar at a foot, close,
the hull and the shoreline, the slam at four instants, the ring at the second boat, the landing), `sheet_styles.webp`
(every selectable collar on the crab), `sheet_bosses.webp` (the other four bosses before and after),
`lineup_t2.5.webp` (four crabs, the slam cascade), and clips of one 22 s cycle from `game` and `arena`.

What was built is the design above with four changes, each forced by a picture:

1. **A shape has to be sized by the mass at its own centre.** Sized at each pixel, a lobe as wide as the level's whole
   range was cut along the level's own (rough) lines: a torn edge. The sea now returns, with the contact level, the unit
   direction across the water to the nearest body point (`Contact.near.toward`: the hit the level already found,
   carried through the same quad sharing); the level rises that way by 1 / width a unit, so the level at a lobe's or a
   head's centre is known to first order and the shape is one whole circle or hook. For a mass that is a formula (a
   splash, a wave's mark) the slope is taken from screen derivatives (`slopeOf`). This is the one thing the first
   round lacked, and it is what the prediction named as most likely wrong.
2. **The roll has no inner lines at all.** Half the lobes outlined still read as bubbles. The roll is one cream shape
   with a scalloped edge; the lines inside the white water are the heads' spirals only.
3. **The droplets are their own layer** (sparse dots with a thin key line in the outer fringe), not the roll's small
   lobes: those made a froth.
4. **The crown stands on the foam's outer edge and behind it**, with curved fingers. On the front itself it was hidden;
   in front, it was a flat star that covered the curls.

| what | predicted | seen |
|---|---|---|
| collar at a foot, `game` | a mass 45 to 55 px wide, 2 or 3 heads of 10 to 20 px with a notch, 3 to 6 droplets, one arc | a scalloped mass about 50 px wide with 1 or 2 hooked heads (15 to 25 px), 2 to 5 droplets; the arc is faint |
| collar close | heads 16 to 32 px with a turn and a half of channel | heads 20 to 40 px, one turn of channel between two key lines (the spiral was made bolder: it winds in by half the head a turn, not 0.4) |
| along the hull | 2 or 3 heads of unequal size over about 10 lobes | 3 heads of unequal size on one mass; the rhythm is the hash's, as predicted, and reads as irregular, not as big-small-small-big |
| slam, first quarter | a crown of about 14 pointed fingers | 13 curved fingers behind a boil of rolling foam with curled heads |
| slam, after | a ring of heads, beaks outward, thinning to droplets | that; the ring is thick and full of heads (a burst holds a head in 0.45 of its cells, a collar in 0.75) |
| the crest | a line of heads one deep, breaking up by three quarters of its reach | a line of small hooked heads on the face that carries the boat, thinning to separate heads and droplets |
| landing | the slam's crown, smaller | a small crown, a boil, a ring round the hull |
| swims or pops | no | no popping seen in the stills of consecutive instants; the 22 s clips were rendered and not reviewed frame by frame |

Against the references, what ours still lacks:

- **Height.** The references' splashes stand up (a crown seen from the side, a wave front with a face). Ours is paint on
  the water's surface: the crown is a flat star from above, and from the low `lineup` camera a burst is a thin band.
- **The big sweep.** A reference head is larger than the mass it rises from and trails a tapering tail; ours are hooks
  set into a scalloped mass, never larger than the collar is wide.
- **Line weight.** The references' key line is heavier than ours (a tenth of a head, never under 1.5 px).
- **The long ripple lines** outside the collar (smooth, like the sea's own lines); ours are the level's arcs, short and
  a little rough.
- **A bow wave and wake** for a moving hull: not built. It needs the hull's velocity, which neither the style nor the
  sea has; it would be a drawer of its own fed by the rig.
- **Lean relative to the body's edge.** The collar's heads curl the way the swell travels, all one way in the world;
  the bursts' and the crest's curl outward. A lean that follows the body's edge needs a direction that is constant over
  a head, and the level's is per pixel.

Which bosses use it: the crab (collar, crest and bursts, by default: its `foam` slot's base); the wyrm, thundermaw and
the lamp take the roll collar only. Left as they were, and why: the whirlpool's arms (as heads they broke into a
dotted band; the flat spiral lines are better), the fleet (from its camera the hulls' foam is a few pixels and no
different), and the other bosses' bursts (not looked at at their own instants, so not switched).

Cost, `profile` (normal frame, least of 40), islander, `game`, 2560 x 1600, main and this branch in turn. The machine's
load average was 60 to 90 throughout (seven workers); `tools/sea_bench.ts` and the 1280 x 800 runs were dropped.

| instant | main (ridge), four or three runs | this branch (roll) | difference |
|---|---|---|---|
| t = 1.0, the collar only | 5.51, 5.51, 5.57, 8.45 | 6.09, 6.09, 6.09, 7.08 | +0.55 at the medians (+0.14 ms a 1280 x 800 frame) |
| t = 12.4, the slam's burst and the crest too | 5.70, 5.77, 5.77 | 6.82, 6.88, 6.88 | +1.1 (+0.28 a frame) |

Against the budget: the collar was to cost no more than `curls` (+0.10 over `ridge`); it reads +0.14. Three of four
pairs agree to a timer tick and one run of each build is an outlier a third higher, so the sign is certain and the
amount is good to about 0.03: it is probably over the budget by a little, from the droplets' hash, the direction's
derivatives and the arctangent. The burst and the crest together add about 0.14 a frame while they are on screen,
against the 0.10 predicted for the two. Neither was measured at 1280 x 800 or in a quiet machine.

New base hashes (`game`, 1280 x 800, the base crab): t = 1.0 `00623051d0eb03ef`, t = 3.6 `9ca615e4fe5b5b4c`,
t = 11.3 `322b7f7a5d7aa8c0` (main 9f7d0e2: `4d1bff1565296878`, `b6bceaf3bf6754fa`, `c1fe2acaa48c6e88`; the first was
re-measured here, the other two are as given).

Decisions the owner might reverse: the collar's heads curling with the swell (not with the body); the crown as flat
claws on the water; the roll with no inner lines; a burst holding fewer heads than a collar; the whirlpool's arms left
flat; the crab's `foam` slot kept as a look (the other collars one pick away) instead of removed.


## Round 5: the spiral shell (2026-11-12)

The owner, at `crab_round5/back_spiral_pick.webp`: "This feels like the best back design for when it's a stick-eye crab,
as that is a hermit crab. Make it happen." A new back, `spiral` (`examples/bosses/titan/back/spiral.ts`, its closed
form in `spiral_shell.ts`), and the `islander` picks it. `bruiser` keeps its own explicit `back: "headland"` (another
worker is giving it a terraced back); every other variant is as it was.

### What the picked picture settles, read off it

- **Hand.** Tracing the cream ridge outward from the curl it runs clockwise on screen, so going back from the mouth
  the tube turns counterclockwise seen from the face. The first draft of the closed form (169a52e) turned the other way;
  it is mirrored (the coil's angle is `mouthAt + 2 pi back`, and the surface's v runs against phi to keep it facing out).
- **Where the mouth is.** The outermost ridge reaches the lip at the front, under the forward grove, running *down*:
  the tube comes down into the mouth, so the mouth is at the front of the coil facing down, a hood over the carapace's
  back half (`mouthAt` 0.15: its front lip lifted a little, so it faces down and slightly forward). The first draft
  had the mouth below the axis facing forward at the tail: the body barely entered it and the lip was a ring behind
  the crab. In this pose the curl sits low and behind the body, the last whorl's crown over the back, as in the picture.
- **Which flank.** The brief said the face looks to the crab's *left*. In crab coordinates (z right) the `game` camera
  (23, 27, 21) and the `back` camera (-15, 19, 9) both look at the *right* flank (Round 4: "from the `game` camera the left
  flank is the far side"), and in the picked picture the crab heads up and right with its right legs toward us: the
  viewer is behind and to its right. So the face looks right (+z), raised 0.3 rad: the picture and both cameras agree.
  The throw leaves from the left flank, which is the shell's base side, not the curl's.

### The shell's numbers, and what each does to the silhouette (inherited: Raup 1966; the rest is ours)

Raup's coil `tip + W^-back ((c + r cos phi) out(back) + (-t + a sin phi) axis)`:

| number | value | what it does |
|---|---|---|
| `growth` W | 2.0 | each turn twice the last: the face's rings stand 2 apart (126, 57, 31, 15 px across from `game`); the last whorl is half the face's radius, so it is most of the mass |
| `whorls` | 3.25 | turns from the curl to the mouth: three rings and the curl show on the face |
| `mouth` | 2.2 across the coil, 3.2 along the axis | the section: 1.45 : 1 along the axis, a fat globose tube; the shell is 7.8 long, 7.2 wide, 6.2 above the water |
| D (off the axis) | 0 | the tube touches the axis: no navel, the curl closes tight |
| `spire` | 0.15 | each turn's crown steps 0.15 of its radius toward the face: a low cone, the curl stands a little proud |
| `tilt`, `rake` | 0.3, 0 | the face looks right and up: `game` sees it at cos 0.60, `back` 0.48, the picked camera 0.84 |
| `mouthAt` | 0.15 | the mouth ahead of the axis: a hood facing down and a little forward |
| `seat` | in 1.07, up 1.26, side 0.3, lip 1.9 | the hood's plane 1.26 above the body's origin 1.07 ahead of the tail; outer lip at x = 1.61 (the carapace's middle is 1.7); the curl's centre 2.5 behind that, at x = -3.0, y = 0.9 |
| `band` | cream 0.3 on the suture | cream ridges straddle the line where turns meet; brick is 70% of each ring; ridges proud by 5% |
| `lip` | flare 0.12, thick 0.2 | a thick cream roll, the join with the body |

### Predictions (written before the first render)

Crab at rest unless said; `game` at 1280 x 800; from `tools/spiral_theory.ts` (projections of the closed form) and
`tools/spiral_clear.ts` (the islander's own limbs through one 22 s cycle, measured against the shell's twin).

1. **From `game`.** The shell 275 x 205 px (x 435..710, y 117..322), curl centre (472, 250): the face's three outer
   rings 126 x 160, 57 x 78, 31 x 39 px and the curl 15 x 19 px, so two full turns of band and the curl read. Cream ridge
   on the last turn's crown about 21 px wide. The light 76 px tall, foot (493, 135), top 62 px below the frame's top
   (the slam lifts the tail about 30 px), against open sea: a spike at the silhouette's back. Eyes untouched: the check
   still reads about 58 and 43 px (>= 50, 38). Groves at (538, 161) on the crown and (570, 243) on the forward shoulder.
2. **The body in the shell.** The carapace ahead of the outer lip is 0.34 from the shell at its nearest (the head and
   face clear); under the hood about 16% of the carapace's top is inside the shell (the tail in the inner turn, hidden),
   and the lip rests on the back.
3. **Clear of the crab** (measured before building, on the twin): through the cycle the nearest limbs are legR3 0.45,
   legL3 0.47, the great claw 0.69 (at the slam's wind-up), legL2 0.87; the eyes more than 2.4. With the taunt: the
   great claw waved overhead, predicted no nearer than 0.6.
4. **The throw.** The ship's flight passes no nearer than 9.3 to the shell or the light (the limit is 2.4); both lie
   inside the atoll's cylinder that `fliesClear` already states, and the back's `keepClear` adds a disc over each.
5. **Checks.** islander 13/13, bruiser 15/15, the others 10/10; the lineup 40/40.
6. **Pictures unchanged.** base, bruiser, fiddler `game` hashes (t = 1.0, 3.6, 11.3) as before.
7. **Cost** (`game`, t = 1, against the headland). Draws: shell, growth list, tower, gallery, lantern: 5 drawables,
   7 draws with the shell's hull outline and the list's silhouette pass; the headland's 10, so 3 fewer. Triangles: the
   shell's grid 74 x 32 (spaced by the tube's size, not 144 x 36) = 4,736, twice with its hull = 9,472; the light about
   5,600 with outlines (the headland's light and gulls were 7,650, and there are no gulls); about 90 copies at about 12
   each, 1,100: the back about 16,200 against the headland's island, reef and light, so roughly 3,000 fewer triangles
   drawn in total. GPU: the shell's paint is a log, a floor and two turf patches per pixel (the headland's island took
   0.51 ms in the slow state): the islander's frame 0.15 to 0.25 ms under the headland's in the slow clock state.

### Finishing the shell (2026-11-12, a second engineer): why ours read as a red ball, and the turban

Today's render (`out/spiral/now.webp`, before any change) against the pick: a red ball with a spiral roundel on one
flank. The cause is the band law, not the paint: `bandAt` spaced the bands in log(1 + cos phi) so that seen along the
axis the rings stood W apart. That is exact on the face, but `spiral_theory.ts` printed it: the last whorl's cream bands
lay at 54..72, 95..105 and 119..125 degrees round the section, and the 107 degrees over the outer side (the dome every
camera but `pick` sees) were brick. No choice of numbers in that law puts bands on the dome.

**The bands (ours).** Still lines of constant phi, so each runs the tube's whole length (a spiral on the face, a ring
round the dome), but spaced evenly in *arc length* round the section: s = phi + e sin(2 phi) / 2 with
e = (a - r) / (a + r), first order in e and exact in the ratio of the ellipse's speeds (a : r). `count` 6 bands a
section, cream a quarter of each period (a third as wide as the brick between). Every whorl carries the same six,
shrunk by W a turn, so the face still reads as a spiral. Key lines on both edges of every band and on both sides of each
suture (inner whorl at phi = `shows[0]`, outer at `shows[1]`).

**The form (inherited: Raup's coil; the pose and its search are ours).** I searched poses with a new tool,
`tools/spiral_views.ts` (the closed form splatted with a depth buffer into `game`, the lineup's low front camera, a low
rear and `top`, painted by the shader's own `bandAt`, plus the rest-pose legs of `spiral_theory.ts`):
- A forward-opening turban (axis near vertical, mouth facing +x) is what a hermit's shell does, but the mouth plane
  contains the axis, so the aperture is 2a tall above a body 2.5 thick: from the low front a black hole over the face.
  And the dextral hand puts the coil to the crab's right, onto the right legs: every one of 24 poses went 1.1 to 1.8
  inside a thigh.
- The legs leave free only the volume above y = 2.2 over the hip ring and everything behind x = -2. The original pose
  (the mouth down over the carapace, `mouthAt` small) uses it; grown and stood up it becomes a turban: `tilt` 1.0
  (the tip up and to the right), `spire` 1.0 (each turn's crown steps its own radius toward the tip, so the tip is
  0.76 below the shell's top), section 3.0 by 3.0, `mouthAt` 0.3 (0.45 brings the last whorl onto legR3), seat up 1.6.
  The last whorl runs back to x = -9.2 and down into the sea behind (the woodblock's contact foam rings it).
- The light stands on the tip on a plinth, the last 0.9 turn printed cream (the collar) with a ring and a crust of
  barnacles, leaning out along the spire and half as much back. Groves on the shoulders of the whorls at 0.3 and 1.15
  turns (found, not given: where the surface faces most nearly up and shows). Barnacle knots run along one edge of
  each band from the mouth toward the collar, the growth kit's `run` (crowded and large low, thinning upward).

### Predictions (written before the first render of the turban; `spiral_views.ts`, `spiral_theory.ts`, at rest)

1. **Bands.** Cream bands crossed by the best line through the shell: `game` 7, low front 4, `top` 8, low rear at
   least 4 (the splat sketch is noisy from below; it counts 22). So at least three in every view.
2. **Size.** Silhouettes at `game`: shell 71,400 px against the body's 36,100: 1.98 times (ask: 1.5). Shell 340 x 258
   px (was 276 x 217), 7.3 above the water, 11.1 long, 9.4 wide. Low front: 2.1 times the body.
3. **Mouth.** At `game`, 8,000 px of lip and 900 px of throat show (the body stand-in in front); the carapace ahead of
   the lip is 0.60 clear; 13% of its top is under the shell.
4. **Light.** Foot (-4.3, 5.0, 4.3), top 9.25 above the water; from `game` 101 px tall, its top 66 px below the
   frame's top (the slam's lift leaves it in frame). Groves at `game` (475, 65) and (505, 104).
5. **Clear.** Legs at rest 0.68 (legR3) and 0.96 (legL3). Through the cycle (`spiral_clear.ts`) I expect the walk to
   take 0.3 to 0.5 of that: least about 0.2, nothing inside. The throw passes 9.7 from the shell.
6. **Checks** islander 14, others unchanged; base hashes unchanged.
7. **Cost.** The shell's grid is unchanged (74 x 32); the growth list grows from about 90 copies to about 250 (six runs
   of eleven knots of three, less those hidden or under water, plus 26 at the collar): within 0.1 ms. Frame time at
   `game` within 0.1 ms of today's.

### Scored (the same day; the numbers as committed: `tilt` 1.35, `mouthAt` 0.6, seat in 0.0 up 1.6 side -0.2, cream 0.3)

The first render missed twice, and the numbers moved after the predictions:
- **Cream 0.25 printed as thin lines.** The law was right (a half-cream test showed three broad bands on the dome),
  but each band's two key lines and the proud hump's hull creases ate it from far off. Cream 0.3 (a little under half
  the brick's width, broader than the brief's third), no hump.
- **From the low front the light stood behind the great claw** (tip at z = 4.5; the claw is at z 5 to 6). Standing the
  axis up (`tilt` 1.35) brings the tip to z = 2.5: the light over the middle, above the eyes.
- **The mouth was behind the great claw from `game` at every time.** Seating the shell 1.07 further back and turning
  the mouth forward (`mouthAt` 0.6; legs clearer, not tighter) did not fix it: from `game` the fiddler's great claw
  stands exactly between the camera and the shell's front-right. Only a sliver of lip and throat shows beside it. A miss.

Measured (`spiral_views.ts --measure out/spiral/v4.webp`, t = 1, by ink along the best of four lines; barnacles are
cream too, so the counts were checked by eye: at least three broad bands in each view):

| | predicted (first form) | after the moves (theory) | measured |
|---|---|---|---|
| bands crossed: game, low front, low rear, top | 7, 4, >= 4, 8 | 5, 5, 13 (noisy), 8 | 5, 6, 5, 7 |
| shell / body silhouette at `game` | 1.98 | 1.89 | (theory only: the body and shell share an ink) |
| least limb gap through a 22 s cycle | about 0.2 | | 0.78 (legR3 at t = 5.2); great claw 1.42 |
| checks | islander 14, others as they were | | islander 14; base 11, stilt 16, fiddler 15, fiddlerBaseDisc 15, mask 18, bruiser 16 |
| base `game` hashes | unchanged | | 0c1b1e4368820ff8, 0db3c25b4d11121b, 2315edeb83c975e9 |
| frame at `game`, slow clock state | within 0.1 ms | | 5.31 -> 5.44 ms (+2 ticks; three alternating rounds, another worker on the GPU) |

Still worse than the pick: the pick shows the spiral on the flank, bold and nearly as much cream as brick; ours turns
its face to the sky (the price of the light on top from the front and the legs' clearance), so from `game` the spiral is
seen from above and from the low rear the shell is a large red dome with three bands. The mouth does not read from
`game`. The waterline foam is thin: the shell's back meets the sea, but the contact foam there is a line, not the
pick's white ring.

## Round 4: the samurai's face (2026-11-11)

The owner, looking at the four crabs: "did we refine other crabs' gestures and faces?" We had not. The islander has a
face that changes through the fight; the samurai (`mask`) had a first pass, a scowling human face painted on a disc
whose eye slits are 6 px tall from the `game` camera. All four crabs will stand side by side in one video, so each
needs a temperament of its own. This is the samurai's. Gestures of the body are another worker's.

**Temperament: a stern, proud warrior. Its expressions are kabuki: few, large, held.** A mie is a pose struck and
frozen at a peak of emotion, often with the nirami, the cross-eyed glare. Where the other crabs flow, this one snaps
and holds; beside their motion its stillness is its character.

References: `crab_round4/others/mask_expressions.webp` (six expressions), `mask_turnaround.webp`, `mask_taunt.webp`.
The face is reworked in place (`examples/bosses/titan/face/mask.ts`); the first pass is in git at 9b3deee.

### The face

What is special about this problem is the camera and the temperament, and both point the same way. The `game` camera
is 44 degrees above the horizon and 43 degrees to the crab's right: a ball eye's pupil goes round its far side (round
3 found this and turned the islander's sockets to the audience). A mask does not have that problem if its eyes are
**flat**: a pupil in the middle of a flat white looks at whoever is looking, from every camera, and a pupil slid to the
nose is seen to be crossed from every camera. So the eyes are big, nearly flat discs with small black pupils and no
iris, and a mie faces the audience without the face knowing where the audience is.

| part | what it is | what it does |
|---|---|---|
| face | the red sphere patch in front of the helmet, relief and flat inks | nods and turns on its sphere as a whole (chin up, chin down, toward the ship) |
| eyes | two flattened cream balls, black-rimmed, 1.6 across, lids painted on them in the face's red | lids close to slits or shut one eye; pupils slide to the ship, or cross |
| brow plates | two black lacquer bars standing 0.42 proud of the face | tip into a deep V, lift apart, cock one up |
| mouth | thick lips in a downturned bow on the face's surface, cream teeth, upper fangs in the gape | the jaw drops everything under the mouth's line; lips part to bare the teeth; a corner drags aside |
| tusks | two cream fangs on the lower jaw | drop with it |

| expression | when (the rig's clocks) | what is drawn |
|---|---|---|
| rest | otherwise | brows low in a shallow V, lids a third down, pupils square on you, teeth just showing |
| wanting | 0.2 s after the Grab's warning, until the claw has the ship | the face turns to the ship, eyes slide all the way to it (`rig.shipNow`), the near brow cocks, the jaw loosens |
| straining | the hold | eyes screwed to slits under crushed brows, every tooth bared, chin tucked; it trembles with each squeeze |
| triumphant (a mie) | the let-go, held 1.8 s | chin up, brows flung high and apart, eyes wide round small pupils, lips sealed in a proud bow |
| furious (a mie, the nirami) | Claw Crush's warning, held until the claw lands | brows in a deep V, eyes wide and crossed, the jaw dropped in a roar |
| wincing | the slam, 0.55 s | the near eye shut under its jammed brow, the mouth dragged to that side, the head jolted back |

**Timing is one library piece** (`src/geom/struck.ts`). `struck(clock, { from, snap, hold, release })` is one weight
for a score's `show` layer: from the cue it first goes the wrong way (the brows lift before they slam down), arrives at
speed and stops in one frame; through the hold it is exactly 1; the release eases. Stillness needs no switch: a `show`
at weight exactly 1 replaces every channel with the expression's own number, so what flows (a blink, the strain's
tremble) is laid first and a held pose covers it. `holdsStill` checks that: through each hold every channel is the
cast's own number. The whole face reads the score on one clock (no eye lags): in a mie every part lands on one beat.

Inherited: the sphere patch, its rim, the lacquer guards and brim, the jaw that drops everything under the mouth's
line (this face's first pass); the cast, the score and the marks a check compares (round 3); anticipation as a dip
before a drive (`anticipated`, the twelve principles); the mie and the nirami (kabuki). Mine: the flat eye as the answer
to a high camera; a struck pose as one weight whose plateau is exact, with stillness by the order of layers and a check
that holds it; anticipation of a pose as a negative weight (the face leaves the pose it is about to strike, in every
channel at once, for nothing); the nod and turn of the whole face as channels.

### Predictions (written before the first render of this face)

From `tools/mask_theory.ts`: plain arithmetic on the face's own numbers, the body at rest, each expression at full
weight, the `game` camera at 1280 x 800.

1. **Size.** The face's middle is turned 48 degrees from the camera. The right eye (the camera's side) is a disc 54 px
   across by 51 tall, the left 36 by 49. `minScreenSize` measures a ball of the eye's radius, not the turned disc: 56
   and 54 px, held at 48. Pupils 15 and 10 px at rest, 11 and 7.5 in the glare.
2. **The glare crosses.** The pupils are 71 px apart at rest and 49 crossed: each moves about its own width (13 and
   10 px) toward the nose. I expect it to be seen at true size on the right eye and to be marginal on the left.
3. **Light against dark.** In the `game` picture at rest, the mean luminance of the cream pixels inside the right eye
   over the mean of the ring from 1.0 to 1.3 radii round it (black liner, lacquer brow, red face) is at least 4 to 1.
4. **The brow V and the roar.** A brow plate is 87 px long (right) and 62 (left), about 20 px thick; its inner end
   drops 22 px from rest to the glare and rises 40 px in triumph. The roar is 30 px from lip to lip (6.5 at rest) and
   about 110 px wide. Held by `drawnLength` at 55 px and 16 px.
5. **Expressions.** All six and the rest reach at least 0.9 of the face in a cycle. The closest pairs are rest and
   straining (18 px), straining and furious (20), rest and wanting (21.5); the farthest triumphant and furious (62).
   Measured on the moving crab (`expressionsRead`, a quarter of the right eye's width: 13.6 px), the closest pair will
   be between 14 and 22 px. By eye, from true-size stills, all six are told apart; the pair most likely confused is
   rest and straining if the slits do not read.
6. **Nothing hides it.** The brim is paint on the face's own top, so nothing overhangs. A brow plate 0.42 proud hides
   about 7 px of the eye under it from this camera. At rest and through every hold neither gauntlet crosses the face
   from `game`.
7. **Timing.** A snap is 0.13 s, four frames at 30 a second: weights -0.06, -0.09, 0.18, 1, so one frame carries 82%
   of the change. Held: wanting 2.57 s, triumphant 1.8, furious 1.24, wincing 0.55. Through those holds every channel
   is exactly its pose's number (`holdsStill`: 0 off).
8. **Checks.** `mask` passes 15 of 15 (its nine, and six of the face's: two eyes, the brow, the roar, the expressions,
   the holds). Every other variant as before. The base and islander `game` pictures do not change.
9. **Cost** (`mask`, `game`, t = 1; before: 129 draws, 149,621 triangles). After: 135 draws (two eyes, two brows, two
   tusks), about 152,500 to 155,500 triangles, about 45 more per-frame tables. GPU frame: at most two timer ticks more
   (0.13 ms) in either clock state; the face's paint covers under 2% of the picture.

### Scored (2026-11-12, after the build; on main at 9f7d0e2 merged in: the lineup, the taunt, `rig.audience`)

Pictures: `out/face_mask/expression_sheet.webp` (each expression from `face` and as a `game` crop with a true-size
inset), `vs_reference.webp` (each beside its reference), `mie_strip.webp` (the triumph and the glare frame by frame),
`game_through_cycle.webp`, `lineup.webp`, `cycle_mask_game.mp4` and `cycle_mask_face.mp4` (one 22 s cycle each).

| | predicted | measured |
|---|---|---|
| eyes, `minScreenSize` (a ball of the eye's radius) | 56 and 54 px | 56.0 and 53.4 px |
| eyes as drawn, `game` | discs 54 x 51 (right) and 36 x 49 (left) | by the theory on the final numbers 56 x 53 and 41 x 52; in the picture the right eye's white between its lids at rest is 46 x 31 px |
| pupils, rest to crossed | 71 px apart to 49; each moves its own width | 67 to 49 by the theory on the final numbers; in the pictures the near pupil is seen jammed down against the nose, the far one is small |
| white against its surround (luminance) | at least 4 to 1 | 4.70 to 1 (white 0.878, ring 0.187; `game`, t = 9.5; a throwaway script) |
| brow plate, right | 87 px long | 82.2 px (`spansOnScreen`, held at 55) |
| the roar, lip to lip | 30 px | 35.2 px (held at 16) |
| every expression reaches 0.9 of the face | 7 of 7 | 7 of 7 |
| closest pair | rest and straining, 18 px, in 14 to 22 | rest and wincing, 22.8 px (0.40 of the eye; the limit is 0.25): the cast changed after looking |
| told apart by eye at true size | 6 of 6 | 6 of 6 in the sheet's insets, my own judgement; the triumph is the one I would not name without the label |
| nothing hides the face | neither gauntlet crosses it at rest or in a hold | **missed**: the crusher crosses it on the way up (below) |
| a snap | 4 frames, one carrying 82% | as designed (`mie_strip.webp`) |
| held | wanting 2.57 s, triumphant 1.8, furious 1.24, wincing 0.55; 0 off | 2.53, 1.77, **0.80** (the glare was moved), 0.53, counting only 30 Hz samples strictly inside a hold; every channel exactly its pose's number |
| checks | `mask` 15 of 15 | 17 of 17 on the merged tree (its ten, and seven of the face's: `staysInView` came with the merge); every other variant as on main, the lineup 40 of 40 |
| draws | 135 | 141: an outlined strand is two draws, its body and its ink hull; I counted one |
| GPU frame | at most two ticks more (0.13 ms) | slow clock state, under load (other workers were rendering): 4.78 to 4.85 and 4.72 to 4.78 in two pairs taken back to back in one tree (one tick each); 4.72 to 4.92 in a third pair taken across two trees. One unpaired fast-state reading of the new face: 1.18 ms |

Where the reasoning was wrong:

- **The game camera saw one eye.** The theory measured each eye alone, as round 3's did, and both were big enough.
  The far eye was 60 degrees from the camera on the sphere's far side, behind the nose and its own brow plate. A flat
  eye answers where a pupil sits; it does not answer which way the face is turned. The face now turns toward its
  audience inside the helmet (its features slide across the sphere; the rim, the brim and the cheek guards stay): a
  fifth of the way round at rest and half in a mie, which is also what a mie is. Since the merge the audience is the
  rig's (`rig.audience`), so a staged crab turns to its own camera. The mask is tilted 0.4, not 0.3; the eyes are 1.1
  from the middle, not 1.25; the plates stand 0.3 proud, not 0.42.
- **The first glare had no pupils.** Crossed level, they sat under the brows' V. The theory moved the pupils and the
  brows and never laid one over the other. They cross to the nose and down now.
- **The crusher crosses the face.** I looked at two stills (rest and t = 11.3) and wrote that the gauntlets keep off
  it. On its way up the crusher's forearm lies across the face from the game camera until about 1.1 s before the slam
  (`staysInView` measures a point of it behind the palm's skeleton for 0.20 s; the forearm covers more for longer). A
  mie struck at the warning was half over before it was seen. It is struck a second before the slam now, when the claw
  has passed: the claw rises, then the face strikes. The hold is 0.84 s, not 1.24.
- **Triumph first read as fright**: brows flung apart over pin pupils in round whites. The brows keep their angle now
  and the lids are level.
- Not reasoning, but found by looking: the gape's teeth followed the grid's rows in a sawtooth (the jaw's stretch was
  a smooth step across two rows of the grid; it is a straight ramp across five now), and a fang's bump of a signed
  distance made every upper tooth a fang.

What the reference has that this lacks:

- red brow plates, thick sculpted slabs; these are black lacquer bars (chosen for the game camera: a dark shape on red);
- small eyes set deep under the brows; these are a third of the face each (chosen: the pupils must be seen to cross);
- a sculpted nose and the folds round the mouth as forms; here two nostrils and ink lines;
- a mouth plate with a heavy lower lip; here the lips are outlines;
- in the wince, a mouth dragged far to one side; here the pull barely shows;
- in the triumph, a huge closed bow of a mouth that carries the pride alone; here it reads from the `face` camera and
  is a thin line from `game`;
- a helmet brim that overhangs, big turned-back cheek guards, and the cords tied under the chin (none built).

Weak or unfinished:

- The triumph is the weakest pose: haughty, not triumphant.
- From `game` the far eye is three quarters the width of the near one; the nirami reads on the near eye.
- Only the face is still in a held pose: the body treads and sways under it (the rig's walk and its moving hold).
- The glare's hold is short because of the crusher's path. A crusher that rose outside the face would give the whole
  wind-up back.
- The contrast figure is from a throwaway script, not a check.
- `examples/bosses/lib/face_parts.ts` (the bruiser's) has brows, eyeballs and tusks of the same kind as this face's
  own; the two should be unified.
- The taunt's mie (the triumph with the eyes crossed, on `rig.dance.wave`) was looked at in the lineup's stills only.



## Lighting and shading (2026-11-12)

The owner, of the bruiser: "I like the cel shaded style, but it feels like it still might need some lighting and
shading; things seem a little too flat. I'm not sure. Perhaps make toggleable and explore all that." Then, of the nine
painted references (`shading/overview.webp`): "all those lighting options are amazing, let's leave those as a library."
So this round builds shading treatments that switch on and off and combine, and a library of named recipes over them.
No scene's default picture changes.

### Where today's picture is flat, and why (from `out/shade/now.webp`, written before any reference was opened)

The one number that explains it: from the `game` camera the sun (0.148, 0.791, 0.593) is 23.3 degrees off the view
axis. The camera looks down the light. With the terminator at N.L = 0, the shadow side of a ball covers 4% of its
disc, of a level limb 0 to 5%, of an upright one 8% (`.scratch/theory.ts`: the share of the visible section in
shadow). From `three` (60 degrees off) the same rule gives 25%, from `face` 18%, from `back` 15%. So the print has
two tones in the side views and one in the game.

- `game`: the great claw, its cream fingers and the head are three flat shapes with one key line; nothing lies under
  the claw, so it reads as a cut-out laid on the body. Head, carapace and claw arm are one brick area. The legs are
  sticks: none shades another, none sits in the water except by its foam. The cliff meets the shell with no shadow,
  so the island floats. The island's top is one green.
- `three`: the shadow side exists (the claw's lower edge, the legs' undersides) and is the best of today's shading.
  Parts still do not shade each other: the arm on the head, the head on the mouth.
- `face`: the brows do not shade the eyes; the head is one red with painted marks.
- `back`: the claw is wholly dark (it faces away), the trees stand on the grass with no shadow, the shell's rim is flat.

### What a printer does to turn a form, and what is ours

Inherited from the print: a second block overprinted for the shadow tone (one ink over every colour, so each ink has
its own shadow colour for free); bokashi, a graded wipe of that block, always along one direction of the picture (the
foot of a hill, the top of the sky); line (the engraver's cross-contour line that swells as the form turns from the
light); paper left unprinted for a light, and the uninked groove beside a line (the white-line cut). Inherited from
cel animation: two and three tones with hard edges, the terminator placed for the form and not for the physics, a
drop shadow under whatever overlaps.

Ours:

1. **The shadow block is a channel.** Under a recipe a body prints its lit inks, and the picture's alpha (the
   style's `mark`, which no woodblock pass reads today) carries the tone: 1 unprinted, falling to the shadow's depth
   where the block prints. Core shadow, contact band, bokashi and hatching each lower it in the body's own shader;
   cast shadows lower it afterwards by blending with `min` (a union: two shadows over each other are one shadow, as on
   one block); then one full-screen draw pulls the block through fixed-function blending
   (`dst * dstAlpha + ink * (1 - dstAlpha)`), per sample of the multisampled picture, before the water is drawn.
   Nothing darkens twice and no pass reads the picture back.
2. **A terminator measured round the section.** A bare threshold N.L < t makes a limb's shadow depend on how the limb
   points: at t = 0.35 a far leg coming down to the water (it points 8 degrees off the sun's own line, |L perp| = 0.15)
   is 100% dark while the near legs are 15%, and the share pulses as the legs walk. The painter's shadows follow each
   form's long axis whatever its direction. Rule: shadow where N.L < t |L perp|, where L perp is the sun's part across
   the surface's long direction (dP/dm, from the screen derivatives of the world point and the body coordinate). On a
   shaft this is "the same share of every section is in shadow", on an end cap it is still N.L. Predicted at t = 0.4:
   every leg 8 to 17% in the game, the far leg no longer black.
3. **Cast shadows from the capsules the water shadows already use**, drawn as boxes over the bodies (below).

### Cast shadows: the three ways, and the choice

| Way | What it shadows | Cost predicted | Stable | Host and format |
|---|---|---|---|---|
| A shadow map from the sun | everything, from every camera | the opaque pass again (0.3 to 0.5 ms: its cost is vertex work) and as many pipelines again | texel stairs of 0.7 px in the game, 2.6 px in `face`, moving with every moving caster | a pass, a texture, a second frame uniform: a format version and a loader change |
| Screen space, marching the depth | what the camera sees in front | one post pass, 16 taps a body pixel | one decision per pixel from a single-sample depth: the ragged edge the key line pass was removed for (2026-11-11) | none |
| Analytic capsules, one box per segment | every thing with a skeleton (legs, claws, carapace, reef, brows, eyes, antennae, the ships' spars): 518 segments on the bruiser | the water shadows' cost again, 0.13 ms measured for them, plus a full-screen pull: 0.15 to 0.25 ms | exact: a closed form per pixel, antialiased by the pixel's own size; no texels | one more blend state (`min`); the loader already reads every blend operation: no format change |

Chosen: capsules. A print's shadow is a simple shape; a capsule's is one, and it cannot crawl. What it gives up: the
head, the island's own outline (the reef's capsule stands in for it), the trees and the sails cast nothing.

From the game camera a cast shadow is a drop shadow: a receiver t behind its caster along the sun shows the shadow
0.40 t to the lower right on screen (32 degrees below level). The claw 2 above the shell: 27 px. A leg 0.5 over
another: 7 px. A brow 0.15 over an eye: 2 px in the game, 10 px in `face`.

### Predictions (written before the first render of any treatment)

Sizes are at 1280x800 from `game`, where a pixel at the crab is 0.029 units.

1. **Core, terminator at 0.4 of the section.** Shadow share of the visible form in the game: the shell and claw palm
   about 22% (a crescent on the lower right, away from the sun on the upper left), a level leg 8 to 17%, the far legs
   the same as the near ones. In `three` about half of each form, in `face` 40%. Failure to look for: half-dark
   forms in `three` reading as night. A plain sun threshold at the same number will show the far legs all dark.
2. **Highlight** (N.H above 0.974, a hard shape in a lighter ink): 5% of a ball's disc; a blob about 60 px across on
   the shell if the island leaves it room, 35 px on the claw palm, a 5 px stripe along each leg. It sits just up and
   left of each form's middle (H is 11.6 degrees off the view). Failure to look for: plastic. If the leg stripes are
   busy at game size the size threshold has to scale with the form.
3. **Cast.** The claw's shadow on the head and shell is the largest single change in the game picture: a shape 20 to
   30 px wide to the claw's lower right. Legs on legs 5 to 8 px. The carapace's capsule puts the upper legs in shadow
   where they leave the body. In `face` the brows put a band about 10 px deep on the top of each eye. Holds still by
   construction; in the sun swing the shadows move only as the sun does. Unknown: whether the capsules' radii match
   the drawn sections well enough at the claw (a palm is wider than it is thick).
4. **Contact.** A band where a body stands in the water, 0.2 units high: 4 to 5 px on a leg in the game. Predicted
   weak alone at that width; the cast shadows do the work of "parts sit on each other" because the game camera looks
   down the sun.
5. **Bokashi.** A wash graded by height over the water (full at the waterline, gone 2.2 up): the feet of the legs,
   the shell's rim and the cliff darken toward the water; the raised claw is untouched. Along one direction of the
   picture, so it cannot band or mottle a form.
6. **Hatching.** Lines across each limb (constant arc length along the spine, 0.22 apart: 7.5 px in the game), their
   weight swelling from nothing at the terminator to solid in the deep shadow. At 7.5 px a period it is drawable in the
   game; under about 4 px a period the lines are replaced by their share of tone, as the sea's lines are.
7. **Rim.** A cream edge where the surface turns from the eye on the sunward side. Its width follows curvature, so
   flat grazing faces will take a wide band: predicted the weakest, and only readable over a body printed darker.

Cost, in a normal frame of 2.4 ms as measured today under load (1.2 in the fast clock state): each of core, highlight,
contact, bokashi, hatching and rim under one tick of the timer (0.065 ms), unresolved; cast 0.15 to 0.25 ms. Budget
for the recommended mix: 0.3 ms.

### Taken over (2026-11-12, second engineer): predictions for the video's look, the palette and the sunset

Written before the first render of any of them. The earlier stills of `hatched` (black lines 0.2 apart, swelling from
0.5 of N.K above the terminator) showed what to change: black lines over cream read as a zebra; the hatched zone
reached most of every form; from close the lines were few and fat (the octaves only merged lines, never split them).

**B. `hatched`, rebuilt.** Lines in each colour's own shade (a block of 55% near-black brown at 60% value: brick lines
(0.26, 0.08, 0.06), cream lines a warm grey (0.29, 0.25, 0.22)); the zone from 0.14 of N.K above a section terminator at
0.2 (offLens 45) to full weight 0.55 below; weight 0.62 of a period at most; octaves both ways so the period on screen
is always 3.4 to 6.8 px (the reference's lines are about 4 px apart); each line's end moved by a hash of its index
(ragged 0.1 of N.K). Predicted, from `game` at 1280x800 (0.029 units a pixel, so level -1.02: about 3.5 px a
period): the hatching covers about a quarter of each form's disc on its lower right, the lines one to two pixels wide,
reading at true size as a darker textured tone more than as lines; the cream claw tips keep their lit cream with grey
hatching only along their lower edge. Close (the lineup, the `face` view) the same 3.4 to 6.8 px period: an engraving,
not stripes. Failure to look for: crawling where a level crossfades (lines are a subset of the finer level's, so none
should move, only fade), and the ragged ends reading as noise at game size.

**C. The reference palette** (`referencePalette`; with `hatched` it is `engraving`). The crab's lit brick (0.66, 0.22,
0.13) becomes (0.84, 0.26, 0.13) (the references sample 0.81 to 0.87, 0.23 to 0.28, 0.12 to 0.15); blotches at 0.74 of
that, (0.62, 0.19, 0.10) against the reference's (0.58, 0.18, 0.11), over about a quarter of the red, 0.3 to 0.6
units across (10 to 20 px from `game`, several times that in the lineup); the key line 0.07 wide (from 0.05) and up to
0.05 more inside the hull on the shadow side; the sea one deep ink (0.08, 0.17, 0.30), troughs (0.055, 0.12, 0.24),
against the reference's (0.07 to 0.11, 0.17 to 0.22, 0.28 to 0.32), lines cream and a third of today's width. Unknown:
the noise's spread, so the share mottled could be 10% or 40%; the darker dark brick and coral are left as painted.

**D. `sunset`.** The sun low behind the crabs (`--input sun=-0.95,0.3,0.05` on the lineup, whose camera looks down -x).
The forms are turned by a light fixed 35 degrees left of and 30 above the lens (hatched as in B), so every face keeps
its lit ink and only its lower right is hatched; the block is violet and lifted (50% of (0.3, 0.12, 0.32) at 0.72: brick
lines (0.41, 0.14, 0.16), never black); a gold rim (1, 0.7, 0.32) 0.09 wide on the silhouette toward the sun, which from
the lineup is the crabs' tops and far sides. Failure to look for: the rim lost where the sun is straight behind (it
lights only edges that face it by 0.12 of N.L), and the fronts reading as noon because nothing darkens them.

Cost (the single crab from `game`): `hatched`, `referencePalette` and `engraving` add one noise3 pair and a few
derivatives a body fragment: under a tick (0.065 ms), unresolved. On the four crabs at 1280x800 with about four times
the body pixels: `engraving` 0.1 to 0.2 ms.

### Scored (the same day, on main d9e2ed6 merged in)

Pictures: `out/shade/sheet_looks.webp` (every look on the bruiser from `game`, true size and doubled, and `face`, each
beside its reference), `sheet_lineup.webp` (the lineup at t = 2.5 and 18 in the print as it is, `hatched`, `engraving`,
`engravingSunset`), `sheet_video_look.webp` (`engraving` beside `lineup_wide` and `stalk_hybrid`).

**B, `hatched`.** Missed on the first render, in the direction of too little: with the section terminator at 0.2 and the
lines starting 0.14 above it, the hatched zone was a thin crescent (about a tenth of a form, not a quarter) and from
`game` the crab looked unlit. My quarter came from the earlier core prediction at 0.4, which I carried to a lower
threshold without recomputing. Fixed by the terminator at 0.3 and a key 60 degrees off the lens: from `game` the claw's
lower half is now engraved as in `shade_hatched`. Second miss: the octave level was taken from the world size of a pixel,
and on the head in `face`, whose length runs toward the camera, the rings crowded to two pixels apart; the level is now
measured on the lines' own screen rate. Third: rings seen end on (an eyelid) printed a bullseye; they are now left out
within about 20 degrees. The ink in each colour's own shade held: no zebra on the cream. At the lineup's present
camera (distant) the hatching is a darker tone with texture, as predicted for small forms. Not measured: crawl in
motion beyond looking at the 22 s clip (`clips/crab_hatched_22s.mp4`); I saw no swimming lines.

**C, the palette.** The colours land where they were set (they are set from the samples). Mottling missed: size 0.55
and cut 0.18 covered about 40% of the red in large merged clouds; at 0.42 and 0.3 it is sparse, irregular blotches,
closer to `stalk_hybrid`'s, though theirs are rounder and denser in small clusters. The param `woodblock.outline` could
not take a second default in one process (two styles built by `--light` collided), so the heavier line is a factor on
the param. The deep sea reads as the reference's, with thin cream lines; its far haze was first a pale grey band and is
now a dark blue. What the references still have that ours does not: a hand-cut key line that varies along each stroke
(ours swells only by light), blotches with small satellites, heavier brows and pupils, a coarser paper.

**D, `sunset`.** The fronts stay at their lit ink and the gold rim reads on the sunward edges, as predicted. The failure
I named came true: the first render looked like noon with a gold outline. The undersides are now washed into the violet
block (a bokashi from the form's middle down), which gives the backlit read without blacking a face.

**Where the terminator should sit.** Not at N.L = 0 of the sun. From the game camera the sun is 23 degrees off the
view, so a sun terminator leaves a ball 4% in shadow, and from above a level limb's terminator runs along its top or
bottom edge depending on how it points, not along its length. The painter's shadows run along each form's long axis on
the side away from a light kept well off the lens. So: the light the forms are turned by is the sun swung 45 to 60
degrees off the lens (`offLens`), and the terminator is measured round the section (`section`), N.K < at |K across the
form|, so every limb shows the same share in shadow, at 0.3 to 0.4 of the section. Cast shadows still fall along the
true sun.
