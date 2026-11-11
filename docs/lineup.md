# The crab lineup: four bosses in one scene, on a beat

Written before the code (2026-11-11) and committed; scored at the end. The scene is `examples/bosses/crab_lineup.ts`:
four crabs side by side facing the camera, each with a boat to throw and a boat to shove, doing a slam, a throw and a
taunt in time, as one loop.

## What the library needs

1. **A boss placed by a pose.** The crab's rig is built at the world's origin from absolute numbers. It becomes
   placeable by `boss.at` (x, z) and `boss.heading` (radians from +x toward +z), inputs with defaults, with the
   vocabulary of `src/geom/placing.ts`. Everything the boss owns is authored in the boss's own axes (x forward, z to
   its right) and carried to the world by that pose: the walk and its feet, the body and shell frames, the attacks'
   hit shapes and warnings, the hand-off readouts, the stand-in boats' places. The boss's other inputs that are places
   (`crush.centre`, `boat1.at`, `throw.land`, `boat2.at`) and headings are therefore in the boss's axes: their defaults
   are numbers, and a number cannot follow a pose that is itself an input. At the default pose (0, 0; 0) the boss's
   axes are the world's and nothing changes: **one crab alone must be pixel-identical**, the base's proof frames
   `625dcf706bbc944c 4b25ec069f162af8 044800b6ce69f7ff` before and after.
2. **Several bosses in one scene.** Names collide (things, tables, inputs). A namespace is a dynamic scope
   (`namespaced(id, build)`): inside it every name a thing, a table or an input is made with is prefixed, at the few
   constructors where names enter (surfaces, bodies, copies, tabulated tables, posed lists, telegraphs, inputs), so no
   option file changes. Outside any namespace nothing is prefixed, which is (1)'s identity again. One sea is shared:
   a boss is built in two stages (its attacks, which is where its ring wave comes from; then, on the sea made with every
   boss's ring, the rest), and `lineup([...])` composes them.
3. **A taunt.** An `emote`: an action with no hit, a schedule (`once` / `every`) and accents (seconds after it arms),
   whose beats a body and a face read as they read an attack's. The rig turns it into a dance for any claws option: the
   great claw waves up, out and down on each accent, the body rises on the legs with it and sidesteps on the off-beat,
   the small claw answers half a beat late.
4. **A tempo.** One clock in beats.

## The loop, predicted

**120 BPM**, 4/4: a beat is 0.5 s, a bar 2 s. **12 bars, 24 s.** Lineup, left to right: islander, fiddler, mask,
stilt (crab k = 0 .. 3). Every accent on a beat:

| Bars | Seconds | What |
|---|---|---|
| 1 to 3 | 0 to 6 | **Slam, a cascade.** Crab k's claw starts up at 0.5 + 0.5 k and lands at 2.0 + 0.5 k: beats 4, 5, 6, 7. Each ring takes its boat 0.72 s later and carries it toward the camera |
| 4 to 8 | 6 to 16 | **Throw, in unison.** Warnings at 6.5; claws rise at 7.0; all four grips close at 9.5; squeezes at 10.0 and 11.0; a held, cocked pause; the whip starts at 11.5 and all four let go at 12.0 (bar 7's downbeat). The boats fly 3.0 s, back over the crabs' left shoulders and away, and come down at 15.0 |
| 9 to 12 | 16 to 24 | **Taunt.** Bars 9 and 10, call and answer: crabs 0 and 2 wave on beats 1 and 3, crabs 1 and 3 on beats 2 and 4. Bar 11, a cascade: 0, 1, 2, 3 on successive beats. Bar 12, unison: all four on beats 1 and 3, then still for the last beat before the first claw of the next loop rises |

To make those land on beats the attacks' own seconds move a little, by numbers the lineup overrides (the single crab
keeps its own): the crush's wind-up 1.2 (was 1.15) so it starts 1.5 s before it lands; the grab's reach 1.38 (1.3) so
the claw rises 2.5 s before it closes; the carry 2.0 (2.7) so the two squeezes fall on beats; the stroke 0.5 (0.4),
one beat; the flight 3.0 (3.2).

**No seam.** The period is declared by the scene and the host wraps its clock, so t = 24 is t = 0 exactly; what has
to be made true is that nothing jumps at 24 - dt. Every attack repeats every 24 s; the walk's period is 2 s (a bar);
`alive` is given the loop's period inside the lineup; the sea's two swells are given speeds whose periods are 12 and
8 s. The thrown boats, crushed twice and thrown 30 units, go down where they land (under an opaque sea) and new boats
sail in from below the frame during the taunt; the shoved boats sail back to their places. Not made periodic: the
contact foam's wobble (two fixed rates in the sea's shader) and the ships' sail flutter, both sub-pixel to a pixel or
two at the seam.

| Check | Predicted |
|---|---|
| frame at t = 0 and at t = 24 | the same picture, the same hash |
| mean pixel change across the seam (23.967 to 0) | within 1.5 times the mean change between two ordinary consecutive frames |
| every named check of every crab | passes, with the lineup's seconds |

## Cost, predicted

The single crab's normal frame (`game`, 1280 x 800) is 1.77 ms in the fast clock state and 4.78 in the slow, of which
the sea, the sky, the clearance passes and the post pass (about 0.9 fast) do not grow with the number of bosses.

| | One crab | Four, predicted |
|---|---|---|
| draws | 117 | about 460 |
| render pipelines | 121 | about 480 |
| pose tables, bytes | 100 K | about 400 K |
| `loft_detail` slots of 4096 | 70 | about 280 |
| shadow capsules | 526 | about 2100 |
| WGSL | 3.4 MB | about 13 MB |
| load (pipelines) | 6 s quiet | 25 s or more |
| normal frame, fast state | 1.77 ms | 0.9 + 4 x 0.8 = **4.1 ms: 2.3 times one crab, not 4** |
| normal frame, slow state | 4.78 ms | about 11 ms |

The limits I expect to come near: the number of storage buffers a shader stage may bind if the tables no longer fit
one binding, and the load time. Wrong if: the frame is more than 3 times one crab's; a build refuses on a limit I did
not name.
