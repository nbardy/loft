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
staging: **the eyes face the audience and never quite leave it**. The socket turns three tenths of the way to what it
looks at and the iris slides the rest across the white, so it watches the ship from the side of its eye, and the game
camera always sees white, iris and pupil. Two lids an eye (a dark brow above, a brick cheek below), a moss-green iris
(the island's own ink), everything bigger on the great claw's side (the right eye is larger, higher, wears its lid
higher and leads by 0.14 s), and a handlebar moustache of two curled antennae.

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
| wince | the slam, 0.5 s | shut tight, stalks jolted down and apart, the lopsidedness thrown over |

Expression references were to be generated first (`tools/crab_faces.py`: six moods, each close and at game distance).
A process from `~/git/flux-klein-fast` was running when this started, so the script was written and not run, and the
shapes come from the round 2 fiddler pictures.

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

