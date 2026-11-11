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
