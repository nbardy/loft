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
