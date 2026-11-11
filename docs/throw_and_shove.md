# The throw and the shove

Written before the code (2026-11-11) and committed; scored against the built scene in the last section. It extends
`docs/attacks_that_hold.md`: a hold may end in a throw, and an attack may leave a wave on the water that shoves what
floats. The scene is the Reefback Titan (`examples/bosses/reefback_titan.ts`).

## One rule: velocity is continuous at every hand-off

A thing changes owner four times in a throw: the grip takes it, the stroke accelerates it, the air has it, the water
takes it back. The rule is that its velocity is the same on both sides of each hand-off. Everything else follows.

1. **Carry to stroke.** The carry (the lift and the squeeze, as today) ends at rest at the wind-back place W.
2. **Stroke.** Uniform acceleration from rest for sigma seconds along the release velocity v0:
   `p(tau) = W + v0 tau^2 / (2 sigma)`. It ends at the release point `P = W + v0 sigma / 2` moving at v0. Inherited:
   constant acceleration is the least peak force for a given stroke length and release speed (and the fastest stroke
   under a force limit), so it is what a grip that must not lose its prize would do.
3. **Flight.** Ballistic: `p(s) = P + v0 s - g s^2 / 2` (inherited). The scene states where the thing is drawn back to
   (W: it is where the carry ends, so `carry.to` keeps its meaning), where it lands (L) and for how long it flies (T).
   Then, since `P` depends on v0, solving `L = W + v0 (T + sigma / 2) - g T^2 / 2` gives the closed form

   ```
   v0 = ((L - W) / T + up g T / 2) / (1 + sigma / (2 T))        P = W + v0 sigma / 2
   ```

   Nobody states the release point or the release speed: they follow from the wind-back, the landing and the flight
   time. The wind-up is the carry's end, behind the release point along the throw, for free.
4. **Touchdown.** The water is a drag and a spring, and it too receives the thing at the thing's velocity. Horizontally
   a linear drag with time constant tau_d: `x = L + vh tau_d (1 - exp(-s / tau_d))`, so it skids `|vh| tau_d` and stops.
   Vertically a damped heave of frequency f and damping zeta about the floating height, started with the touchdown's
   downward speed vd: `y = L.y - (vd / wd) exp(-zeta w s) sin(wd s)`. The depth of the plunge and the length of the
   skid are therefore not art numbers: they follow from how it lands. (Mine: I have not seen the landing treated as
   one more velocity-continuous hand-off; the usual form is a squash keyed at contact.)
5. **The gripper** follows through from P along v0, decelerating uniformly to rest over rho seconds (it travels
   `v0 rho / 2` more), opening as it goes, then goes home.

The tumble is a rigid turn about one axis at a constant rate, a whole number of turns over the flight, so the thing
lands upright. Its rate starts at the release (a flick of the fingers): this is the one place the rule is not kept,
because the claw's palm does not turn.

Gravity is one number for the world: **g = 5 units/s^2**. If a unit is a metre that is half of Earth's; if the sloop
(3.8 units) is a real 19 m ship it is 2.5 times Earth's. It is slow enough that the flight hangs for 2.6 s and the crab
reads as large, and fast enough that the stroke (0.5 s) still reads as a whip.

## The wave uses the same gravity

A ring wave from the slam. The sea here is shallow: the crab stands on the bottom, 1.7 below the surface
(`S.floor`), and the pulse is longer than that, so it is a shallow-water wave: it does not disperse, and its speed is

```
c = sqrt(g depth) = sqrt(5 x 1.7) = 2.92 units/s
```

(inherited: Lagrange's shallow-water speed; mine: taking g from the throw and the depth from the crab's own floor, so
the wave's speed is not a free number). Its energy spreads round a growing circle, so its height falls as
`sqrt(R / r)` (inherited). The front leaves the disc's edge at the slam, so it reaches distance d at `(d - R) / c`,
and ahead of the front the water is exactly as it was.

```
rise(r, tau) = H sqrt(R / max(r, R)) P((R + c tau - r) / W) fade(r)        P = 0 outside (0, 1)
```

P is one crest and the trough behind it, C1 at the front; `fade` takes it to nothing at `reach`.

**The shove.** What the wave gives a floating thing is an impulse when its front arrives: a change of velocity,
outward, in units per second,

```
impulse(x) = n  J(d)        J(d) = (D0 / tau_w) sqrt(R / max(d, R)) fade(d)        delivered at tau = (d - R) / c
```

for a thing whose place is x (distance d from the centre, outward unit n). The impulse is proportional to the local
height of the wave. Loft's stand-in for a boat takes that impulse under a linear drag of time constant tau_w, the
same law as the thrown ship's skid:

```
pushed(x, tau) = impulse(x)  tau_w  (1 - exp(-s / tau_w))        s = max(tau - (d - R) / c, 0)
```

so it ends `D(d) = J(d) tau_w = D0 sqrt(R / d)` outward, and it is exactly 0 until the front arrives. It is a closed
form of the clock and the thing's own place. The constructor refuses a ring whose impulse at the disc's edge,
`D0 / tau_w`, exceeds c (mine: a thing cannot be pushed faster than the wave that pushes it). The thing's height and
heel are not animated at all: a boat placed with `afloat` at `x + pushed` reads the sea's height there, ring
included.

(As first committed, the shove was an eased displacement over a `ride` time. It was replaced before any picture was
judged, when the owner asked that a game be able to run its own boats: an eased displacement sampled at a boat's own
moving place is not the same motion as one sampled at its starting place, so "the field" and "the stand-in" would
have disagreed. An impulse is delivered at one instant at one place, so a game that adds it to its boat and applies
the same drag gets the stand-in's motion exactly. The final offset is unchanged, D0 sqrt(R / d).)

**Why an aftermath and not a third effect.** The effect says what the attack does to its target and what it owns
(nothing, or a held thing). The wave is what it does to the water, and any effect may have one. As a third `Effect`
variant, "instant with a wave" would have to repeat the instant's handling in every dispatcher and a hold could not
have a wave at all. So it is a second sum type beside the effect: `Aftermath = still | ring`, one dispatcher.

**Cost.** The ring is a `SeaDeform` whose `reaches` is the live annulus (between the front and the tail, after the
slam, inside `reach`): it is false everywhere when no ring is running, so the sea's deformed cases run on no pixel
then, and on the annulus only while it runs.

## The crab, predicted (base variant)

The loop is 18 s (was 16): the Grab closes at 3.0, Claw Crush arms at 11.5. Both boats jump back to their places at
the loop's seam (t = 0), where both attacks are over. Every place below is the default of a host input (see "The
contract"); the numbers are the standalone show's.

**The throw.** The sloop is taken at (10.49, 0.25, -8.40), 13.4 from the crab's middle. The carry draws it back 2.7
along its keel, 0.9 to starboard and 2.15 up, to W = (9.13, 2.40, -5.90), in 2.6 s (two squeezes); the stroke is
0.5 s; it lands at L = (5.70, 0.25, -21.00) after T = 2.6 s. That landing is the farthest open water from the crab
that the `game` camera shows with the mast in frame (its far right corner): radially outward the frame ends 6 units
from where the ship is taken.

| Quantity | Predicted |
|---|---|
| release velocity v0 | (-1.20, 5.18, -5.30), speed 7.50, 43.6 degrees up |
| release point P | (8.83, 3.69, -7.22), 1.88 along the stroke from W (acceleration 15.0) |
| flight | 2.60 s, released at t = 6.10, touchdown at t = 8.70 |
| apex | y = 6.37 (6.12 above its floating height), 1.04 s after the release |
| distance flown (horizontal, P to L) | 14.13 |
| touchdown speed | 7.82 down, 5.43 along |
| plunge (heave 0.9 Hz, damping 0.45) | 0.79 below floating height 0.22 s after touchdown, then 0.16 above |
| skid (drag 0.2 s) | 1.09, to rest at (5.46, -22.06) |
| where it ends | 22.4 from the crab's middle (3.4 carapace lengths of 6.6), 14.6 from where it was taken |
| the claw | follows through 1.13 past P in 0.3 s; the grip's middle stays within 4.3 of its arm's centre (span 4.9) |

**The wave.** R = 3.4, H = 0.8 at the disc's edge, W = 6, reach 16. Boat two floats 7.5 from the disc's centre at
(14.38, -0.18), outside the disc, broadside to the ring.

| Quantity | Predicted |
|---|---|
| wave speed | 2.92 |
| front reaches boat two | 1.41 s after the slam (t = 12.91) |
| crest height at boat two's place | 0.54 |
| impulse at boat two (D0 = 4.6, drag 1.6 s) | 1.94 units/s outward (0.66 c); at the disc's edge 2.88 (0.99 c) |
| boat two's final offset | 3.10 outward; 2.98 of it by the loop's seam (5.1 s after the front) |
| before the front | pushed and rise exactly 0 |

From the `game` camera boat two moves about 117 pixels at 1280 x 800, a little more than its own length on screen.

**Checks.** All five variants pass, with the throw written in the ship's own axes (keel, starboard, up) so each
variant's ship is drawn back relative to where its own claw takes it: `thrown` (velocity continuous at the release,
above its floating height until touchdown, lands where and when stated, travels at least 12), `fliesClear` (the flying
ship keeps clear of the carapace and the atoll), `gripReaches` over the hold including the stroke, `shoves` (boat two
ends at least 2.5 out) and `frontFirst` (nothing at boat two moves before the front). (As built the first is two
checks: `releasesAt`, the hand-off, which holds whoever flies the ship, and `throwLands`, the stand-in's flight.)

**GPU frame time** (`game` camera, 1280 x 800). Priced first by removal: the scene without its ship is 0.26 ms
faster in the slow clock state (4.46 to 4.13 median, 14 paired rounds), so a ship is about 0.3 ms slow, 0.1 ms fast.

| | Predicted change |
|---|---|
| second ship (25 draws) | +0.30 ms slow state, +0.10 ms fast |
| the ring while no ring runs (t = 1): one distance and three compares per sea pixel, one `rise` per sea vertex | +0.00 to +0.07 (one tick) |
| splash foam, new frame tables | under one tick |
| **normal frame at t = 1** | **+0.3 ms slow (4.46 to about 4.8), +0.1 ms fast (1.75 to about 1.85): +7%** |
| while the ring runs (annulus up to a third of the sea's pixels in the deformed case) | a further +0.15 ms slow, +0.05 fast |

Wrong if: any of the closed-form numbers above is off by more than 1%; the frame at t = 1 rises by more than 0.45 ms
(slow) or 0.17 ms (fast); a variant fails a check with these numbers.

## The contract: what a game sets, what it reads, who owns a boat when

The scene plays on its own, and it is also the picture of a game that owns its boats. Both read the same things.

**Who places a boat is a sum type** (`Placer`, src/geom/placing.ts): `standIn` (a closed form of Loft's, so the scene
plays with no game) or `host` (the game passes the boat's pose every frame and Loft draws it there). It is classified
once, by the design's `boats` slot (`base` is the stand-ins; `--variant boats=host`), and `placedBy` is its one
dispatcher. While the Grab holds the first boat it is the crab's in both.

| When | First boat (the Grab's) | Second boat (under the wave) |
|---|---|---|
| before `grab.arm` | its placer's: afloat at `boat1.at`, or the host's pose | its placer's, always: afloat at `boat2.at` plus the stand-in's drift, or the host's pose |
| `grab.arm - 0.22` to `grab.arm` (the grip closes) | blended from its placer's frame into the socket | |
| `grab.arm` to the release (3.1 s) | the crab's: the socket | |
| from the release | its placer's at once: the stand-in's flight, landing and bob; or the host's pose | |

**Inputs** (`--input name=value`; a vector is `x,z` or `x,y,z`). Units: world units, radians, seconds on the scene's
clock. The defaults are the show.

| Input | Meaning | Default (base) |
|---|---|---|
| `grab.arm` | when the Grab's grip closes; it repeats every 18 s from there | 3.0 |
| `crush.arm` | when Claw Crush lands | 11.5 |
| `crush.centre` | the slam's disc on the water (x, z); its radius is the variant's number | 9.6, 5.6 |
| `boat1.at` | where the first boat floats when the Grab arms (x, z). Hold it fixed from the warning to the release | 10.49, -8.40 (each variant's own: where its claw reaches) |
| `boat1.heading` | its heading then, radians from +x toward +z. The claw comes along it | -0.75 |
| `boat1.beam` | its width across the grip: the grip opens and closes by it | 1.19 |
| `throw.land` | where the throw is aimed to land (x, z). The release velocity follows from it | 5.7, -21.0 |
| `boat2.at`, `boat2.heading` | (stand-in only) where the second boat floats when Claw Crush arms, and its heading | 14.38, -0.18; -0.25 |
| `boat1.pose.at`, `.heading`, `.pitch`, `.heel` | (`boats=host` only) the first boat's transform, every frame: its origin (waterline amidships; x, y, z), heading, pitch (bow up), heel (to starboard) | its start, level |
| `boat2.pose.at`, `.heading`, `.pitch`, `.heel` | (`boats=host` only) the second boat's | its start, level |

No formula holds a boat's place as a constant: the warning, the reach, the grip, the wind-back, the throw, the splash
and the shove are expressions of these. A game is trusted to arm the Grab only where the claw reaches (the arm spans
4.9 from the shoulder; `gripReaches` measures the default).

**Readouts** (closed forms exported by the scene module as `handoff`, `holds`, `waves`; a host evaluates them on the
CPU each frame, as the checks do).

| Readout | Is | Units |
|---|---|---|
| `handoff.held` | 1 while the crab has the first boat | - |
| `handoff.socket` | the boat's frame while held: origin (its waterline amidships), bow x, mast y, starboard z. Level, at the heading it was taken at: the palm does not turn | world |
| `handoff.squeeze` | its size along its own x, y, z while held (narrowed across the beam, volume kept) | factors |
| `handoff.release.time`, `.at`, `.velocity` | the instant the grip lets go, and the boat's place and velocity then. A game's physics takes the boat from exactly here | s; world; units/s |
| `handoff.release.spin` | the tumble Loft's stand-in gives it (a vector: radians per second about its direction). The claw gives none; a game may use it or not | rad/s |
| `handoff.landing.time`, `.at`, `.rest` | where and when the stand-in's flight lands and stops: a prediction under g = 5 for a host that flies its own | s; world |
| `holds.grab.thing()`, `.flight()`, `.toss()` | the boat's middle wherever it is (the host's pose after the release, with `boats=host`); the stand-in's flight share and seconds since touchdown; the throw's places | world; s |
| `handoff.wave.arrival(place)` | seconds after `crush.arm` that the ring's front reaches a place (x, z); `waves.crush.arrives(place)` is the time itself | s |
| `handoff.wave.lifts(place, t)` | how far the wave lifts the water at a place and time (below 0 in its trough; exactly 0 off the pulse) | world |
| `handoff.wave.impulse(place)` | **an impulse per unit mass: a change of velocity, outward on the water (x, z), delivered at the arrival** | units/s |
| `waves.crush.pushed(place)` | the stand-in's drift from that impulse under its drag of 1.6 s: `impulse x 1.6 x (1 - exp(-s / 1.6))` | world |

**The stand-ins are consumers of the same contract**, and checks say so: `releasesAt` (at the stated instant the boat
is at the stated point and the grip has it at the stated velocity; it runs in both variants), `throwLands` (the
stand-in's flight starts at the release point with the release velocity, and lands where and when stated), `shoves`
(the stand-in's drift starts at the wave's own impulse and ends at impulse times drag) and `frontFirst` (before the
arrival the field is exactly 0).

**What a host must do** to keep the picture continuous: hold `boat1.at` and `boat1.heading` fixed from the warning;
while `held`, put its boat at `socket`; at `release.time` start its own flight from `release.at` with
`release.velocity`; for the wave, add `impulse(place)` to its boat's velocity at the arrival, and float it on
`lifts`.

## What is inherited and what is mine

Inherited: ballistic flight; constant acceleration as the stroke; 1 / sqrt(r) spreading; the shallow-water speed;
linear drag and a damped spring; anticipation, follow-through and squash from the twelve principles.

Mine: velocity continuity as the single rule across grip, stroke, air and water, the touchdown included, so plunge
and skid are derived; stating the wind-back and deriving the release; one gravity for the throw and the wave; the
shove as a closed form bounded by causality and by the wave's own speed; the aftermath as a second axis beside the
effect; the ring's reach as the live annulus so the sea pays for it only where and when it exists.
