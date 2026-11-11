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

## Scored (2026-11-12)

Built as designed, with three additions asked for while it was being built: each crab has a **temperament**
(`numbers.temper`: how sharply it moves into a pose, how early it starts so as to arrive on the beat, how long it
holds, how much it twitches, how late its loose parts follow, whether its body rises or bows, how broad its moves
are, whether both claws go up together), which one rig, one taunt and one pair of attacks play in four manners; the
taunt's beats are on the rig as `rig.dance` (`on`, `wave`, `swing`, `step`, `answer`), beside the attacks'; and main
was merged twice (the bruiser, the headland back, the contact foam's patterns).

| | Predicted | Measured |
|---|---|---|
| frame at t = 0 and at t = 24 | the same hash | `a06b07e3a3d98ae2` both |
| mean pixel change across the seam | within 1.5 times an ordinary consecutive pair | 1.412 against a median of 1.403 (0.93 to 2.64 over 24 instants): 1.01 times |
| every check of every crab, with the lineup's seconds | passes | 40 of 40 (ten each) after two changes to the throw, below |
| draws | about 460 | 526 |
| render pipelines | about 480 | 531 (and 22 compute) |
| pose tables | about 400 K | 460,592 bytes in one buffer; 929 tables |
| `loft_detail` slots of 4096 | about 280 | 479 |
| WGSL | about 13 MB | 16.4 MB in 346 modules |
| load | 25 s or more | 83 s at best, 276 to 358 s with the machine loaded (its load average was 40); compile 49 to 98 s |
| normal frame, 1280 x 800, fast state | 4.1 ms, 2.3 times one crab | **2.88 ms: 1.6 times one crab (1.77)** |
| normal frame, slow state | about 11 ms | 6.95 ms: 1.45 times (4.78) |
| normal frame, 1920 x 1080 | - | 3.87 fast, 9.70 slow |
| one crab alone, placed by inputs at its default pose | pixel-identical: `625dcf706bbc944c 4b25ec069f162af8 044800b6ce69f7ff` | **not identical**: `ef993827ba43088f d73059d036e54e12 85dd5e2c28c85660`; at t = 1, 12 pixels of 256,000 differ, each by 1 of 255 |

Where I was wrong:

1. **Placing by an input is not free of the last bit.** The values are the same (a turn by cos 0 and sin 0, a shift by
   0), but numbers the build used to fold (the feet's homes, the walk's frame) are now computed on the GPU with the
   pose, and the leg chains that follow from them land a unit in the last place away: twelve pixels along the right
   legs change by one level. Isolated by experiment: with the walk left unplaced and everything else as built the
   frame is byte-identical; with the pose's cosine and sine made build constants it is not (the shift by the input
   remains). So identity to the byte needs a crab whose pose is not an input at all, and I did not build that second
   kind: the requirement was inputs.
2. **Four crabs cost 1.6 times one, not 2.3.** I priced a crab at what it costs filling a third of the game camera's
   frame. In the lineup each is a sixth of the frame's width and far away, and vertex work is small beside the passes
   and the sea, which are shared. Cost does grow with the number of bosses, but through the pipelines and the load, not
   the frame: the first measured limit is the load (five minutes on a busy machine).
3. **The level-of-detail slots** grew 6.8 times for four crabs, not 4: the spider crab's thorns and the keep's parts are
   instanced, and an instanced draw takes a slot per instance. 479 of 4096.
4. **The throw as first staged did not pass its own checks**: the islander's ship left 0.50 from its carapace (the
   margin is 0.5) and the spider crab's crossed the edge of its own island. The wind-back no longer goes toward the
   face (`side` 0) and the landing moved from z = -13 to -15. Thrown back over the shoulder the hull takes 0.32 to
   0.37 s to clear the fingers (0.12 to 0.18 thrown ahead): `letsGoClean` allows 0.45.

What the pictures say (`out/lineup/strip_per_bar.webp`, `strip_accents.webp`): the cascade of slams reads left to
right, a claw up while the one before is down; four boats leave four claws on one frame; in the taunt the islander's
claw swings out broad, the fiddler's is small and quick, the samurai's gauntlet goes up and stays, the spider crab
sinks into a bow as the others rise. New boats come up from the bottom of the frame through the taunt and the slam.

What is weak or unfinished:

- The frame is a band: four crabs side by side in 16:9 fill a fifth of its height. Sky above, sea below.
- The spider crab's tongs do not visibly rise together in the taunt (its bow reads; its claws mostly hang).
- The fiddler's twitch is a sine at eight a second; it reads as a shake, not as overshoot.
- The shoved boats are carried along the line and toward the camera; seen end on while they ride, they read less well
  than in the single crab's game camera.
- The thrown boats' landings are far behind the line, small; they sink there. A boat that sinks and a fresh one that
  sails in is a story the owner did not ask for.
- Not periodic in the loop, so they step at the seam by under a pixel or two: the sea's contact-foam wobble, the sails'
  flutter and gusts. The seam's measured change is that of an ordinary frame.
- A built directory of the lineup does not carry the scene's period (the files format is untouched), so only the
  scene path wraps its clock.
- `namespaced` prefixes names at the constructors the crab uses (surfaces, bodies, copies, tabulated and posed tables,
  rope lines, telegraphs, inputs). Fields, reliefs, impostors, glows, media and rain are not yet scoped.
- The faces of the fiddler, the samurai and the spider crab do not yet read the taunt (`rig.dance.on`); the dandy's
  does.

## Staging (2026-11-12): predicted before the first render

The first cut was right in its machinery and wrong in its staging: four crabs a fifth of the frame's height under an
empty sky, the shoved boats coming at the camera end on, the thrown boats lost behind the line. The owner's notes on
it: a longer lens (less background, more face), a lower camera, closer in, a sunset at the horizon. The references are
`docs/art_direction/lineup/` (a low, close, long-lens row at sunset; the camera just above the water, below the faces;
the horizon 70 to 75% down; the row filling 90 to 95% of the width). Staging only: the attacks, the rig and the
library are as they were, except where named below.

Numbers stated once in `examples/bosses/lineup_shots.ts` (tempo, formation, shots); checked on paper by
`bun tools/lineup_framing.ts`, which projects each crab as boxes (its largest moment, its body, its face) through each
shot's camera on the CPU. The boxes are calibrated on the first cut's frame at 22.0 s: there the stand-in gives the
spider crab 139 px where the picture shows about 125, and the line 85% where it shows 80%. **The stand-in reads about
10% large, so a prediction below is its number times 0.9.**

### Why this formation and lens

- **A long lens from in front cannot shorten a straight line of four.** The line's width on screen is its width in the
  world over the frame's width at its distance; the lens sets only the distance at which that ratio is reached.
  Width and height go together: four crabs about 63 units across (centres 37.5 apart, plus the islander's claw and
  the spider crab's legs) fill 92% of a 16:9 frame that is 38 units tall at their distance, so the tallest (15.5 to the
  spider crab's lantern) is 40 to 45% of the height whatever the lens. The only ways to more are overlap (closer
  spacing) or depth (some crabs nearer). Overlap: centres 12.5 apart, legs interleaved (the references' density).
  Depth: the middle pair 3 forward (a shallow chevron, as `lineup_wide_alt` has it, but slight).
- **Why not more depth.** Each crab's slam disc, its boat and its shoved boat's ride are on the water in front of it. In
  a chevron deeper than about 3, or a two-row zig-zag (which the stand-in liked best for size: 56% tall), the back
  crabs' boats and discs fall under the front crabs' legs. Their places are moved instead (below), into a lane in front
  of each crab, and the lanes need the row.
- **Why 0.28 rad (about 85 mm).** With the eye 3 above the water the horizon lands 70 to 72% down and runs behind the
  legs; at 0.24 (99 mm) the faces gain 3% and the camera goes 135 away; at 0.38 (64 mm) the middle pair, 3 nearer,
  grows 2% more than the outer pair. 0.28 is the reference notes' long end and the faces pass 90 px except the spider
  crab's (below).
- **The square** cannot hold four across large: each crab a quarter of the width is a quarter of the height. An oblique
  view (the reference square) shortens the line only with a wide lens from close, and then the far crab hides behind
  the near ones (its face 51 to 100% covered in the stand-in). So the square trucks along the row two crabs at a time
  (fovY 0.4, about 59 mm, from 72 to 75 away) for the slam and the taunt, and holds a wider frame on the middle pair
  for the throw.

### The formation and the props (crab axes: x forward, z the crab's right, the camera's left)

| | First cut | Now |
|---|---|---|
| places | a row, 22 apart, at x = 0 | a row, **12.5 apart**; the middle pair at x = 3 |
| heading | toed in 0.08 per place | each faces the audience point (104, 3.5, 0) |
| slam disc centre | (9.6, 5.6) | **(10, 1.5)**: in the crab's lane, clear of its neighbour's boat by 1.4 |
| boat taken (grab bearing) | -0.75 (stilt -0.55) | **-0.26** for all four: the boat at (12.6, -4.6), keel nearly toward the camera |
| shoved boat | bearing -0.9 (toward the camera), heading 1.9 | **bearing 0.6, heading -2.2**: it rides forward and to the camera's left, side on (its keel 54 degrees off the camera axis), stern first: across the frame, ending 14 left of its crab |
| throw lands | (-24, -15), behind the line, out of sight | **(26, -8)** (the spider crab (26, -5)): thrown forward and to the camera's right, it arcs up in front of the line and lands in the foreground sea, in frame |
| how the boats get home | thrown boats sink, new ones sail in; shoved boats sail back | **on cuts** (`Show` `cut`): the crush's cycle turns at the loop's seam (each crab's warning starts at 0: 2.0 + 0.5 k s before its slam) and the grab's at 6.0 (a warning of seven beats); both are cuts. The shoved boats stay where the ride left them until the seam (`keep` the period) |

The throw thrown ahead also clears the fingers sooner (0.12 to 0.18 s, against 0.32 to 0.37 back over the shoulder:
docs/throw_and_shove.md). The slam lands more in front of the face than before (the disc moved 4 toward the middle):
the claw crosses the face for the slam's half second.

### The shots (cuts on downbeats; the camera at t = 24 is the camera at t = 0, so the loop closes)

| Format | From | Shot | Camera: eye, target, fovY |
|---|---|---|---|
| 16:9 1920x1080 | 0.0 | slam (the main framing) | (120, 3, 0), (0, 10.5, 0), 0.28 |
| | 6.0 (bar 4) | throw: pulled back so the landings are in frame | (138, 3, 0), (0, 11, 0), 0.28 |
| | 16.0 (bar 9) | taunt: a push in over the eight seconds, eased | (108, 3.5, 0) to (100, 4, 0), target (0, 9.5, 0) to (0, 9, 0), 0.28 |
| square 1080x1080 | 0.0 | slam: a truck along the row following the cascade, 1.75 to 3.75 s | (75, 3.5, z), (0, 8.5, z), 0.4; z from 12.5 (islander and fiddler) to -12.5 (samurai and spider crab) |
| | 6.0 | throw: the middle pair and half of each neighbour | (100, 3.5, 0), (0, 9, 0), 0.4 |
| | 16.0 | taunt: a truck along the row, 16.5 to 23.5 s | (72, 3.5, z), (0, 8.5, z), 0.4; z from 12.5 to -12.5 |

### Framing, predicted (stand-in times 0.9; at each crab's largest moment; left to right islander, fiddler, samurai, spider crab)

| Format, shot | Line's width | Crab heights (px) | Tallest | Faces (px) | Horizon |
|---|---|---|---|---|---|
| 16:9 slam | 100% (the islander's raised claw at the left edge) | 369, 405, 341, 485 | **45%** | 104, 115, 94, **78** | 72% |
| 16:9 throw (widest) | **86%** | 318, 347, 293, 419 | 39% | 89, 99, 80, 68 | 71% |
| 16:9 taunt, start | 112% | 414, 455, 383, 544 | 50% | 116, 130, 104, 87 | 70% |
| 16:9 taunt, end (tightest) | 122% | 451, 496, 417, 591 | **55%** | **126, 140, 114, 95** | 68% |
| square slam / taunt (two crabs) | over 200% (two in frame, two out) | 433 to 589 | 52 to 55% | 94 to 142 | 66 to 67% |
| square throw | 150% | 313, 345, 290, 410 | 38% | 87, 98, 79, 66 | 64% |

Targets: line at least 85% of the width, tallest at least 45% of the height, faces at least 90 px at the taunt's main
framing. Met by the 16:9 slam (100%, 45%) and the taunt (faces 95 to 140 at its end; 87 for the spider crab at its
start); the 16:9 throw is the widest moment and gives up height (39%) so that the four boats land in frame. The
square meets them in its two-crab shots and not in its throw (38%). Where I am least sure: the spider crab's face box
(a guess, 2.6 tall, 5.6 to 8.2 up); its eyes now hang on drooping stalks (main 941a2fe), so the face may read larger.

Every prop of the 16:9 slam and throw is in frame (the stand-in's screen places: discs 16 to 78% across at 82% down;
landings 37 to 97% across) except the islander's shoved boat, which rides out of the left edge (ends at -11%), and the
spider crab's landing at 97% in the slam shot (90% in the throw shot, where it lands).

### The sky

A sunset by the style's own sky, set from the scene: `WoodblockSky` gains, additively, a grade of more than two stops
and a sun disc (defaults that print exactly the old sky: the single crab's base `game` hashes at 1280x800 must stay
`4d1bff1565296878`, `b6bceaf3bf6754fa`, `c1fe2acaa48c6e88` at t = 1.0, 3.6, 11.3). The hard dark stripe at the
horizon is the sky's `below` ink (`SEA.deep`) seen between the far edge of the sea and the horizon: printed in
orange-gold it becomes the strip of light on the far sea. The sun: vermilion, about 0.07 rad in radius, its centre on
the horizon behind the gap between the middle pair, so it shows between and above the bodies, half set.

### Checks, predicted

| Check | Predicted |
|---|---|
| every named check of every crab with the new places | passes (40 of 40); the grab check on the new bearing is the one at risk |
| frame at t = 0 and at t = 24 | the same hash |
| the seam | it is a cut now (the taunt's push to the slam's frame), so the old proof (the seam's change like an ordinary frame's) no longer applies; what must hold is that within each shot nothing jumps: the change across the seam measured as before is a cut's, and the change across 23.967 to 23.933 an ordinary frame's |
