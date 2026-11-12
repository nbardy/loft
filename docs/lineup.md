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
| throw lands | (-24, -15), behind the line, out of sight | **(38, 6), (38, 0), (38, -4), (38, -8)**, left to right: thrown forward, it arcs up in front of the line and lands in the foreground sea, in frame, the four fanned (with the toe-in about 18 either side of the middle). First staged at (26, -8): `throwLands` wants a throw of 25 at least, and that carried 15 |
| how the boats get home | thrown boats sink, new ones sail in; shoved boats sail back | **on cuts** (`Show` `cut`): the crush's cycle turns at the loop's seam (each crab's warning starts at 0: 2.0 + 0.5 k s before its slam) and the grab's at 6.0 (a warning of seven beats); both are cuts. The shoved boats stay where the ride left them until the seam (`keep` the period) |

The throw thrown ahead also clears the fingers sooner (0.12 to 0.18 s, against 0.32 to 0.37 back over the shoulder:
docs/throw_and_shove.md). The slam lands more in front of the face than before (the disc moved 4 toward the middle):
the claw crosses the face for the slam's half second.

### The shots (cuts on downbeats; the camera at t = 24 is the camera at t = 0, so the loop closes)

| Format | From | Shot | Camera: eye, target, fovY |
|---|---|---|---|
| 16:9 1920x1080 | 0.0 | slam (the main framing) | (120, 3, 0), (0, 10.5, 0), 0.28 |
| | 6.0 (bar 4) | throw: pulled back so the landings are in frame | (138, 3, 0), (0, 11, 0), 0.28 |
| | 16.0 (bar 9) | taunt: a push in over the eight seconds, eased | (122, 3, 0) to (112, 3.5, 0), target (0, 10.5, 0) to (0, 10, 0), 0.28 (first (108, 3.5) to (100, 4): the islander was cut in half) |
| square 1080x1080 | 0.0 | slam: a truck along the row following the cascade, 1.75 to 3.75 s | (100, 3.5, z), (0, 8, z), 0.25; z from 12.5 (islander and fiddler) to -12.5 (samurai and spider crab) |
| | 6.0 | throw: the middle pair and half of each neighbour | (135, 3.5, 0), (0, 11, 0), 0.25 |
| | 16.0 | taunt: a truck along the row, 16.5 to 23.5 s | (96, 3.5, z), (0, 8, z), 0.25; z from 12.5 to -12.5 |

The square was first planned at fovY 0.4 from 72 to 100 (about 59 mm): the strip left a third of it sky; closer, at 60, the
boats thrown 38 forward landed 22 from the lens and filled its foreground. A longer lens from further gives the same
framing with that foreground three times further off.

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

### Scored (2026-11-12)

Three low-resolution strips (960x540 and 540x540, one load each) and one load for the deliverables. Measured on the
strips' 960x540 tiles, doubled for 1080p; faces measured as the eyes and mouth together.

| Format, shot | | Line | Tallest | Faces (islander, fiddler, samurai, spider crab) | Horizon |
|---|---|---|---|---|---|
| 16:9 slam (main) | predicted | 100% | 45% | 104, 115, 94, 78 | 72% |
| | measured | **98%** | **45%** (the spider crab, 490 px) | about 200, 240, 130, 120 | 74% |
| 16:9 throw (widest) | predicted | 86% | 39% | 89, 99, 80, 68 | 71% |
| | measured | **85%** | 40% | about 170, 200, 130, 100 | 74% |
| 16:9 taunt, near its end (tightest) | predicted | 122% (as first planned); 108% (as built, at 112) | 49% | 112 to 140 | 71% |
| | measured | just over 100% (the islander's claw tip at the edge) | 42% | about 200, 240, 180, 120 | 74% |
| square trucks | measured | over 100% (two crabs, parts of two) | about 60% | over 150 | about 70% |

Where the reasoning was wrong or loose:

1. **The faces were boxes I guessed, and too small.** The stand-in gave the faces 78 to 115 px at the slam; the
   picture gives 120 to 240. The fiddler's and islander's eyes stand on stalks well above the box I drew, and the
   samurai's face is the whole front of its shell. The target (90 px) holds with a wide margin everywhere; the
   prediction was conservative for the wrong reason.
2. **The spider crab is not tallest in the taunt.** Its taunt is a bow: it drops 2.2 on every accent, so its largest
   moment is the slam's wind-up, not the taunt. The stand-in, which knows only the largest box, said 49% for the taunt's
   end; 42% is what a bowing crab gives.
3. **The square was planned wrong twice** (see the shots): a wide lens from close puts the thrown boats' sails into the
   foreground. The long lens from further away is what the references meant by "longer lens" for the square too.
4. **The throw as first staged failed two checks.** Landing 26 forward carried 15 (a throw must carry 25); landing 38
   forward, the hand's turn for two tumbles (60 degrees) passed the release velocity, which a forward throw leaves only
   about 48 degrees off the keel, and the hull left the fingers (0.35 to 0.62 off). One tumble: 40 of 40 pass.
5. **The spider crab's tongs did not rise at lift 2.6**: the claw folds its palm 0.55 rad toward the face from the
   forearm, so a wrist only 4 above the body put the tongs across the face. At 3.6 the forearm stands steep and the
   tongs make a V over the shrine (the reference).

What the strips say:

- The slam reads as a cascade from left to right, a claw up while the one before is down, a splash at each landing,
  the boats on the ring's face carried forward and left, side on.
- The throw: four boats in the air at once, flat and tumbling against the sunset, then four splashes in the foreground.
  Through a long lens a throw toward the camera is mostly up and down; it reads, but less than a throw across would.
- The taunt: four manners. The islander's claw swings broad and wide; the fiddler's great claw lifts small and quick
  and ticks on the eighths (its tick cannot be seen in a strip: it is in the clip); the samurai's gauntlet snaps up and
  stays dead still for 0.6 s; the spider crab bows forward deepest with both tongs up in a V.
- Weak: about eight boats stand along the waterline in front of the crabs through the slam (each crab's two, in lanes
  only 12.5 wide), a fence across their legs; the faces clear the sails, the legs do not. The islander's raised claw
  touches the left edge at its widest.

The deliverables (`~/git/loft/out/lineup/`, after merging main d9e2ed6 with the planted feet): the four clips, H.264
yuv420p, exactly 24.000 s (720 and 1440 frames). The seam: t = 0 and t = 24 the same frame (`9a5f8e3be9119fa3` wide,
`bebf51d371a75567` square); the seam itself is a cut, as every shot change is, and the shot running into it has no jump
of its own: its last two frames change 0.91 times (wide) and 1.32 times (square) the median of ordinary consecutive
frames. So the sails' flutter and the contact foam's wobble, which are still not periodic, step only on a cut. Frame time
5.24 ms at 1920 x 1080 and 4.06 at 1080 x 1080 (GPU, least of 8 rounds). Checks: the lineup 44 of 44; the seven
single-crab variants all pass (base 16, stilt 16, fiddler 15, fiddlerBaseDisc 15, mask 11, islander 14, bruiser 16);
the single crab's base `game` frames are byte-identical to main's (`00623051d0eb03ef`, `9ca615e4fe5b5b4c`,
`322b7f7a5d7aa8c0` at t = 1.0, 3.6, 11.3, main's own files swapped in as the control).

Re-render, one load, every deliverable:

    cd ~/git/_wt/loft-stage && bun tools/lineup_deliver.ts ~/git/loft/out/lineup all --line islander,fiddler,mask,stilt --look default --formats wide,square --fps 30,60 --pick 16.5,22,13.5,2,3.5,12.3,15.15,21.5,9.5

`--line islander,fiddler,mask,bruiser` puts the bruiser in the fourth slot; `--look` takes a name from `LOOKS` in the
scene (today only `default`; an unknown one is refused, so a look that has not landed fails loudly instead of rendering
the default).

## Second staging (2026-11-12, after the owner's look at the first): predicted before the render

Two faults in the first staging's own sheet. **A fence of boats**: eight boats along the waterline in front of the crabs
for most of the clip, sails over legs, claws and faces; the target picture has one small boat. **A band of crabs**: the
row's bodies a quarter of the height under 40% sky. The targets now: the low crabs' bodies (shell top to waterline, no
claws, no buildings) at least 35% of the height in the main framing; raised claws and buildings may reach the top 10%;
the horizon 60 to 70% down; the nearest feet and their foam within the bottom 15%; the taunt's push in ends on two or
three faces.

**Why a chevron and a closer, wider lens.** Our crabs' bodies are small beside their legs: the low crabs' bodies
stand 3.4 (fiddler) to 4.4 (samurai) above the water, measured on the strips, on a 20-unit leg span. A body 35% of the
height wants a frame at most 3.4 / 0.35 = 9.7 units tall at that crab, 17 wide in 16:9: two crabs, not four. So the
main framing is the middle pair (the fiddler and the samurai, the low crabs), brought 8 forward and 11 apart, with the
outer pair 8 back and 12 out, where perspective makes them smaller and puts them inside the frame behind the middle
pair's outer legs, faces clear. A lens of fovY 0.4 (about 59 mm) from 32 in front of the middle pair, the eye 2 above
the water (below every face): the 85 mm of the first staging would have to stand 46 away for the same size and then
flattens the depth that the chevron is for. The slam, which must show four claws and four rides, stays wide.

**The boats, each on screen only around its beat.**

| Boat | Before its beat | Its beat | After |
|---|---|---|---|
| shoved (one a crab) | at its disc's front edge, the side away from the middle, from the seam (0.0) to its slam (2.0 to 3.5): the slam shot's lead-in | the ring carries it forward and outward (left pair to the left, right pair to the right), side on, out of frame | its ride ends out of every later frame; it is home on the cut at the seam |
| thrown (one a crab) | parked out of every frame (below) until the cut at 6.0, then at its place at the crab's front-left | taken 9.5, thrown 12.0 forward, over the camera: it leaves the picture by its top and lands out of it, behind the lens (the outer pair throw 50 forward, the middle pair 38) | landed out of frame; at the cut into the taunt (16.0) moved by the host to a place far behind the row (`hostInputs`, `throw.land` = (-70, 0) in its axes) and home on the cut at 6.0 |

So outside the two events no boat is in the picture: the taunt has none, and the throw shot shows only the boats the
claws are about to take (two of them in frame: the islander's and the fiddler's; the samurai's and the spider crab's
wait beyond the right edge). The parking is a host input, not the scene: the scene alone (a viewer of `crab_lineup.ts`)
shows the landed boats where they land, behind the main camera. The checks read the defaults, the true landings.

**Shots** (`examples/bosses/lineup_shots.ts`; fovY 0.4 throughout):

| Format | From | Shot | Eye, target |
|---|---|---|---|
| 16:9 | 0.0 | slam, wide: all four | (58, 3, 0), (0, 6.5, 0) |
| | 6.0 | throw: the main framing | (40, 2, 0), (0, 4.8, 0) |
| | 16.0 | taunt: from the main framing, pushing in to the middle pair's faces by 24.0 | (40, 2, 0) to (30, 2, 0), target 4.8 to 4.5 up |
| square | 0.0 | slam, wide | (62, 3, 0), (0, 6.5, 0) |
| | 6.0 | throw: the middle pair, the outer pair at the edges | (44, 2, 0), (0, 5, 0) |
| | 16.0 | taunt: pushing in to the middle pair's faces | (44, 2, 0) to (33, 2, 0), target 5 to 4.5 up |

**Predicted** (`bun tools/lineup_framing.ts`; bodies and faces from boxes read off the strips; left to right islander,
fiddler, samurai, spider crab):

| Shot | Bodies (share of height) | Faces (px) | Horizon | Feet reach | Line |
|---|---|---|---|---|---|
| 16:9 slam (wide) | 20, 21, 26, 17% | 152, 226, 183, 115 | 65% | 85% | all four in, claws to the edges |
| 16:9 throw (main) | 28, **35, 44**, 24% | 218, 383, 309, 167 | 67% | **93%** | outer pair inside the frame, claws and buildings past the top |
| 16:9 taunt end | 37, 56, 71, 32% | 287, **623, 503**, 219 | 71% | 115% | two faces fill it; the outer pair at the edges |
| square slam | 19, 19, 24, 16% | 143, 207, 167, 108 | 64% | 82% | all four, small |
| square throw | 26, 30, 38, 22% | 199, 331, 268, 153 | 67% | 88% | the middle pair; the outer pair's faces at the edges |
| square taunt end | 34, 48, 60, 29% | 262, 524, 423, 201 | 69% | 105% | two faces |

Targets: the main framing's low bodies at least 35%: the fiddler 35, the samurai 44 (the islander, outer and further,
28). The horizon 60 to 70%: 65 to 71 (the taunt's end 71, just over). Feet in the bottom 15%: 93% in the main framing,
85% in the slam. Boats outside the events: none in frame. The 16:9 slam misses the body target (17 to 26%) by design:
it is the one shot that has to hold four slams and four rides. Checks: 44 of 44 with the new places and throws.

### Scored (second staging)

One strip (960x540), one fix (the outer pair from 12 to 16 out, the main framing from 40 to 44 away), one full render.

- **Boats: done.** No boat in frame outside the two events. The slam shows the four shoved boats at their discs and
  then riding outward out of frame; the throw shows the boats being taken and then tumbling out over the top of the
  frame; from 15.15 to the seam there are none. Checks 44 of 44.
- **The middle pair: big.** Measured on the strip's main framing (from 40 away), the fiddler's shell top to waterline
  is about 40% of the height and the samurai's about 50% (predicted 35 and 44); the horizon about 63% (predicted 67);
  the white water at the feet at the bottom of the frame; the sun half hidden behind them; the taunt's push ends on
  two faces, each more than a third of the height.
- **The outer pair: lost in the main framing.** Wrong in the reasoning: the stand-in's bodies (8.2 wide) were narrower
  than the drawn ones (about 10), and it had no great claw or eyestalks in its boxes, so it said the outer faces stood
  clear past the middle pair's edges. In the picture the fiddler's great claw and eyestalk stand exactly where the
  islander's face would show, and the samurai's armoured legs and gauntlet where the spider crab's would; moving them
  out to 16 and the camera back to 44 did not uncover them. The islander is not seen in the throw or the taunt.
- **The underlying conflict.** Our crabs' bodies are small on a 20-unit leg span (3.4 to 4.4 high). A body at 35% of
  the height leaves a frame two crabs wide, so the main framing can have big bodies or four characters, not both. The
  target picture has both because its crabs are drawn with bodies three to four times larger against their legs.

## Third staging (2026-11-12): the edit gives each crab its moment, predicted before the render

The second staging's conflict (bodies of 3.4 to 4.4 on 20-unit legs leave a frame two crabs wide when a body is a
third of its height) is answered in the edit, not the art: each crab is framed alone and large on its own beats, and
the frame holds all four only where the ensemble is the point. Formation: an open row, 20 apart (16 left the fiddler's
great claw within reach of the islander's face), nearly facing the camera; the props keep their lanes and their
staging (shoved boats riding outward out of frame, throws forward over the singles' cameras, landed boats parked by the
host between the cuts at 16.0 and 6.0). Cameras: fovY 0.4; a single straight in front of its crab, eye 3 above the
water (2 grazed the feet's foam collar into a line), 26 to 31 away for the low crabs, 40 for the spider crab aimed at
7 up; a pair from 82; the wides from 114 (first bar, the release, the splashes) and 100 (the last bar, so the seam is a
cut that hides the shoved boats going home).

**The edit** (beats from the loop's first downbeat; 48 in the loop; every cut on a beat; bodies by the stand-in times
1.14, the second staging's measured ratio):

| Beats | Shot | Body of its subject |
|---|---|---|
| 0 to 4 | wide, all four: the cascade's wind-ups | 10 to 13% (the spider crab 24%) |
| 4, 5, 6, 7 | islander, fiddler, samurai, spider crab alone, each on its own slam | 57, 51, 52, 73% |
| 8 to 10, 10 to 12 | the left pair, the right pair: the rings carry the boats out | 13 to 17%, 17 and 33% |
| 12 to 15, 15 to 18 | the left pair, the right pair: claws rise to the boats | as above |
| 18, 19, 20, 21 | each alone as the grips close (19) and squeeze (20) | 57, 51, 52, 73% |
| 22 to 26 | wide: the whip (23), the release of all four (24, 12.0), the flight | 10 to 24% |
| 26, 27, 28, 29 | each alone, watching its boat go over the camera | as the singles |
| 30 to 32 | wide: the four splashes in the foreground (30, 15.0) | 10 to 24% |
| 32 to 39 | call and answer, each alone on its accent: islander 32, fiddler 33, samurai 34, spider crab 35, again 36 to 39 | as the singles |
| 40, 41, 42, 43 | the cascade, a single a beat | as the singles |
| 44 to 48 | wide (tighter): the unison bar; the loop closes on all four | 11 to 27% |

Large (body at least 35% of the height): **6 beats, 3.0 s, for each crab**; equal by construction (from 58 the spider
crab, tall on its stilts, was large in its pair too and had 11). The spider crab's body is shell top to waterline,
9 units on its stilts: it is large from much further than the low crabs, so its single stands at 40.

Square: the same edit with the middle wides as pairs (the whip and release on the left pair then the right, the
splashes on the middle pair); the wide only for the first and last bars. Singles give the same bodies as in 16:9 (the
height sets them).

Checks predicted: 44 of 44 (the crabs' places changed, nothing relative to each crab did). The foam at standing feet:
the style's collar is there by default; whether it reads from 3 above the water is the strip's question (item 5 of the
owner's notes): if not, the singles' eye goes up until it does.

### Scored (third staging)

One sheet of every shot's first frame (one load), two fixes, one full render.

The fixes after the sheet: (1) in the slam singles each shoved boat, waiting at its disc's front edge, stood 11 to 15
from the camera and its sails filled the frame: it now waits beside the disc, out to the side between the crabs
(bearing 1.4 outward), and rides sideways, seen in the pairs that follow; (2) its ride ended in front of the neighbour
(20 apart), so a sail stood before the spider crab through its throw and taunt singles: the host now parks the shoved
boats too, from the cut at 6.0 to the seam (`boat2.at`, as the thrown boats' `throw.land`). And the singles were aimed
lower: their near feet were below the frame's edge, so the foam at standing feet could not be judged.

| | Predicted | Measured (the singles, shell top to waterline) |
|---|---|---|
| islander | 57% | about 44% |
| fiddler | 51% | about 35% |
| samurai | 52% | about 54% |
| spider crab | 73% | about 54% |
| large time each | 6 beats (3.0 s) | 6 beats each: the singles are the only frames over 35% (pairs 12 to 29%, wides 9 to 24%) |
| neighbour across a single's face | none | none |
| boats outside the events | none | none (the slam singles show each crab's own boat beside its disc; the grip singles the boat about to be taken; the wides the release and the four splashes) |
| foam at standing feet | the style's collar, if it reads from 3 above the water | it is there and reads as small cream patches at each foot in every single; from 2 above the water it was a line |
| horizon | | about 53% in the singles (aimed nearly level so the feet stay in), 55 to 60% in the pairs and wides |

Wrong in the reasoning: the stand-in's bodies are boxes, and the low crabs' singles aimed lower lose a little height
to perspective (the fiddler's 51% predicted at the first aim; 35% measured at the lower aim, which also puts its great
claw across the frame's top-left). The four are within 20% of each other in large time by construction (equal beats);
in body share they range from 35% (fiddler) to 54%.

Character sheet: `tools/lineup_characters.sh` (each crab's best large frame cut from the 1080p clip beside its
expression reference): the fiddler, the samurai and the spider crab read as their sheets; the islander's face is the
weakest: small eyes on stalks, turned a little away, without the reference's brows and moustache (its face is the
dandy; the reference sheet is the bruiser-like head).

## A drilled group (2026-11-12): predicted before the code

The owner: "sync the crab movements so they're like a kpop group". Earlier, on the same video: "they do their 1) slam
water, 2) throw boat, 3) dance/emote taunt ... nice rhythm and loop", and "when dancing they lean and bend the knees and
feet are planted". What I take it to mean: one move at one instant at one size from every member, a formation with a
centre, framed wide enough that the sync is what you see; character in the faces and in short solos.

What today's video does instead (checked on `out/lineup_engraving/strip_per_bar.webp`): one move (a claw wave) played in
four manners (`temper` sets each crab's arrival, snap, hold, rise: the spider crab bows as the others rise); unison only
in two accents of the last bar; about forty shots in 24 s, most of them one crab alone; an open row 20 apart that never
changes; the slam a cascade.

### What is inherited and what is mine

Inherited from stage dance: counting in eights; key poses that arrive on a count and are held until the next ("hit and
hold"); the groove (knees down on every beat); unison as the default with canon (a ripple down the line), mirror (the
halves opposite about the centre) and solos as contrast; formations with a centre and changes that travel on the beat;
a struck final pose ("the button"). Inherited from Loft: the emote and each crab's `temper` (now only for its solo), the
stance's carriage, the gait, `anticipated` and the timing of `struck`.

Mine: (1) a dance is **one score for the group**, evaluated per member as closed forms, so unison is a property of the
data (every member reads the same key, ease and size) rather than of four performances that try to agree; the per-crab
manner enters only through a solo. (2) **Every channel is a track of key poses** (lean, level, facing, place, each claw):
a value that arrives on a count and is held, the sum of eased steps, cyclic, so the loop closes by construction (the
value before the first key is the last key's). A formation change is the same thing on a place channel, and a turn on
a facing channel; canon is a time shift per member, mirror a sign per member. (3) **A claw accent is an aim from the
shoulder in arm spans**, so a fiddler's banner, a samurai's gauntlet and a spider crab's tongs go to the same angle at
the same instant, each with its own arm. (4) **Feet follow the body's path by stepping on the beat**: the gait is
given the formation path itself as its course, paced in beats, and a foot whose next plant is where it stands does not
lift. So the crab stands planted when the path is still and walks sideways (really stepping) when it changes place or
turns, with no blend between a standing and a walking foot. (5) **A sync instrument** measured on the CPU from each
member's own rig (body frame, display wrist), not from the score.

### The score as a user writes it (examples/bosses/crab_lineup.ts)

    const UP = [0.2, 0.76, 0.12], OUT = [0.2, 0.28, 0.76], ACROSS = [0.6, 0.28, -0.44];  // wrist aims from the shoulder, in arm spans
    export const DANCE = routine({
      beat: BEAT, length: 64, centre: 2, start: "V",
      places: { V: [...], line: [...], diagonal: [...] },          // each member's (x, z) in the world, by formation
      manner: { arrive: 0.15, snap: 4 },                          // the group's: every key not in a solo
      size: { bounce: 0.22, lean: 0.4, roll: 0.08, level: 0.55 },
      score: [
        { at: 1, level: 0, claw: "rest", small: "rest" },
        { at: 2, lean: 1 }, { at: 3, lean: -1 },
        { at: 8, go: "line", over: 4, lean: 0 },
        ...
        { at: 44, who: { canon: 0.5 }, claw: UP, level: 1 },
        { at: 46, who: "mirror", lean: -1, claw: OUT, small: OUT },
        { solo: 0, on: [48, 49] }, ...
        { at: 62, go: "V", over: 4, hit: 3, claw: UP, small: OUT, level: 1 },
      ],
    });

A key is the count it lands on (`at`), what it sets, and optionally `who` (default: everyone together), `over` (counts
it takes; a travel or a turn takes at least one, so its feet can step), `hit` (counts it is held dead still, struck in
a few frames). The library's new names: `routine` and its spec (`beat`, `length`, `centre`, `start`, `places`, `manner`,
`size`, `score`), and the instrument `inSync`. The rig reads one new staging kind (`taunt: { kind: "routine" }`).

### The score, beat by beat (120 BPM, 4/4; 16 bars, 64 counts, **32 s**)

The loop grows from 24 s to 32 s: a phrase of moves, the slam and the throw do not fit 12 bars. Order left to right as
the camera sees it: **fiddler (0), samurai (1), islander (2, the centre), bruiser (3), spider crab (4)**. A crab's right is the
camera's left; lean +1 is onto its own right.

| Counts | Move | Who | Formation | Shot (16:9) |
|---|---|---|---|---|
| 62 to 1 | the button: struck in 0.12 s, held dead still three counts across the seam (claws up and out, full rise) | all | V | V, still |
| 1 | release: level 0, claws rest | all | V | V |
| 2, 3 | lean right, lean left | all | V | V |
| 4 to 8 | sidestep into the line (4 counts, a step a count) | all | V to line | pulls back to the line |
| 8 | crouch (level -0.6): the slam's load | all | line | line |
| 9 to 12 | **the slam**: five claws up on 9, five down on **12** (6.0 s) | all | line | line |
| 16 to 26 | **the throw**: claws rise 16, grips close 21, squeezes 22 and 24, whip 25, five boats let go on **26** (13.0 s) | all | line | line, closer (cut on 16) |
| 27, 29 | great claw punched up, full rise; down | all | line | line |
| 30, 31, 32 | lean left, right, centre; the boats land on 32 | all | line | line |
| 33 | | | | the islander's face (cut on 33) |
| 34 to 38 | sidestep into the diagonal (front: the fiddler, the new centre) | all | line to diagonal | pushes in to the diagonal (cut on 34) |
| 38, 39 | punch up and rise; down, lean left | all | diagonal | diagonal |
| 40, 41 | **quarter turn** to the right diagonal (one count, stepping), sweep out; lean right | all | diagonal | diagonal |
| 42, 43 | turn back, both claws across in front; deep crouch, claws rest | all | diagonal | diagonal |
| 44 to 46 | **canon**: punch up and rise, a ripple down the line half a count apart (44, 44.5, 45, 45.5, 46) | canon | diagonal | diagonal |
| 46, 47 | **mirror**: lean out and both claws out; lean in, claws rest | mirror | diagonal | diagonal |
| 48 to 57 | **solos**, two counts each in its own manner (fiddler 48, samurai 50, islander 52, bruiser 54, spider crab 56); the rest hold a low frame (level -0.3, great claw across) and keep the groove | solo | diagonal | each soloist alone (cuts on 48, 50, 52, 54, 56) |
| 58 to 62 | sidestep back into the V; the button on 62 | all | diagonal to V | pushes back to the V (cut on 58), still from 62 |

Unison: 50 of 64 counts (78%); canon 2, mirror 2, solos 10. The groove (knees down on every count, 0.22) is under every
count except the button's hold, and fades under the attacks' postures (the attacks own the body there).

The attacks' seconds (overrides of the crab's numbers, the same for all five): the slam lands on count 12 (crushFirst
6.0, wind-up three counts as before), warned four counts ahead, so its cycle turns on count 8; the grip closes on 21
(grabClose 10.5), warned five counts ahead, so the thrown boat is home on the cut at 16; the rest as today (reach five
counts, hold five, a flight of six). `crushPeriod` 32. The whole crab's `temper` is the group's in the lineup (snap 4,
lag 0.2, no jitter), so the slam's rear and the shell's lag are in unison too; each crab's own temper is kept for its
solo. The shoved boats are parked out of frame by the host for the whole video (the slam is about the water); the thrown
boats are parked from the cut on 33 to the cut on 16. The sea's swells get periods of 16 s and 8 s (whole turns in 32).

### The formations (world x toward the camera, z the camera's left; every crab faces +x)

| Member | V | line | diagonal |
|---|---|---|---|
| fiddler (0) | (-6, 26) | (0, 32) | (8, 20) |
| samurai (1) | (-3, 13) | (0, 19) | (4, 13) |
| islander (2) | (0, 0) | (0, 6) | (0, 6) |
| bruiser (3) | (-3, -7) | (0, -7) | (-4, -1) |
| spider crab (4) | (-6, -14) | (0, -20) | (-8, -8) |

Why these: every crab carries its great claw on its own right, the camera's left. In a V the left wing stands behind
a neighbour's claw, so the V is lopsided: 13 apart on the left, 7 on the right (12.5 left the fiddler's claw on the
islander's face in the third staging). The diagonal (front-left to back-right) puts every claw on the side away from
the crabs behind it, so it can be tight (4 deep, 7 across): it is the formation for the chorus and the largest bodies.
The line is for the attacks: each crab's disc and boat in its own lane (13 apart). Members never cross: the order left
to right is kept, the new centre is whoever is in front.

Travel per change (four counts, a step a count): V to line 6 to 8.5; line to diagonal 14.4 (fiddler, spider crab);
diagonal to V 15.2 (fiddler). A step of up to 3.8: the walk's stride is 3.0.

### Framing, predicted (`tools/lineup_framing.ts`; bodies shell top to waterline as a share of the height; face window the share of the face box no nearer member's body or great claw covers; left to right)

| Shot | Bodies (%) | Face windows (%) | Faces (px) | Bodies span |
|---|---|---|---|---|
| 16:9 V, eye (55, 3, 6), fovY 0.5 | 13, 18, 19, 18, 19 | 100, 100, 100, 86, 100 | 213, 122, 216, 144, 99 | 4 to 94% of the width |
| 16:9 line, (95, 3, 6), fovY 0.4 | 10, 13, 13, 13, 15 | all 100 | 165, 90, 150, 106, 78 | 6 to 94% |
| 16:9 diagonal, (63, 3, 6) at (8, 4.5, 6), fovY 0.4 | 19, 22, 21, 19, 20 | 99, 92, 100, 92, 100 | 301, 151, 234, 154, 105 | 2 to 85% |
| 16:9 diagonal pushed to (55, 3, 6) | 22, 26, 24, 22, 23 | 99, 92, 100, 92, 100 | 360, 178, 272, 178, 120 | -7 to 90% (the fiddler's legs cut) |
| square V, (62, 3, 4), fovY 0.6 | 10, 13, 14, 13, 14 | 100, 100, 100, 93, 100 | 156, 89, 156, 105, 73 | the outer pair cut by the edges |
| square diagonal, (66, 3, 6), fovY 0.5 | 14, 16, 15, 14, 15 | 99, 92, 100, 92, 100 | 225, 113, 177, 117, 80 | -14 to 98% |

The group's bodies cannot be large: five crabs on 20-wide legs in one 16:9 frame give 13 to 26% (the search over
spacing, depth, distance and lens found nothing past 18.5% for the smallest body with every face at least 85% clear).
The stand-in read bodies 14% small and faces about half size before (second and third stagings), so I expect 15 to 30%
bodies and faces past 150 px in the pictures. The edit answers it by holding the group in frame for 52 of 64 counts and
pushing in on the diagonal, and by the five solos (singles as in the third staging: bodies 35 to 54%). The square
cannot hold five: its group shots keep the middle three whole and cut the ends.

### The instrument, predicted

`DANCE.inSync(readouts)`, a check in the lineup's `check`: for every key of the score and every channel it sets, each
member's arrival (when its readout crosses halfway between its value before the move and its value after) and size
(the value at the key, from its own rest), from the members' rigs: body lift (its frame's height less its ride), lean
(the body's roll), facing (the body's heading less the member's own), display claw (its wrist's height over its
shoulder in arm spans). Sampled at 240 a second around each key.

| | Limit | Predicted |
|---|---|---|
| unison keys: arrivals across members | within 1/60 s | under 1 ms (one expression, f32 rounding); the claws up to 5 ms (their rest drift differs) |
| unison keys: sizes across members | within 5% | under 0.1% |
| canon: arrival offsets against the scored 0.25 s | within 1/60 s | under 1 ms |
| mirror: sizes, opposite in sign | within 5% | under 0.1% |
| solos | not measured (their own manner) | |

Wrong if the attacks' braces or the walk leak a per-crab difference into a unison key (different ride, legs, or numbers
under `stance`): then the score is right and the rig is not.

### Checks and the seam, predicted

Every check of every crab with the new seconds and places: **55 of 55** (11 each), and the instrument: **56**. The legs'
reach (`limbsReach`) is the one at risk: lean 0.4 with a rise of 0.55 over planted feet is near today's worst (0.960).
The frame at t = 0 and t = 32: the same hash, and inside one shot: the button is held dead still across the seam with
the camera still, so the change across the seam is an ordinary frame's (the sea's only), at most its median.

### The cost of a fifth member, predicted

From the four-crab measurements (draws 526, pipelines 531, WGSL 16.4 MB, load 83 s at best, 1920 x 1080 frame 5.24
ms), a crab costs about a quarter of each except the shared passes: **draws about 660, pipelines about 665, WGSL about
21 MB, load about 105 s on a quiet machine (5 to 7 minutes loaded), frame at 1920 x 1080 about 6.2 ms** (the passes
and the sea about 1.5, each crab about 0.95). The score adds per crab a few dozen per-frame scalars and a gait whose
course is the path (16 evaluations of a short piecewise path): under 0.1 ms. Five stops being reasonable if the load
passes 10 minutes or a build refuses on a binding limit; then four, and the hermit stays the centre.
