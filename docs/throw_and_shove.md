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

**The shove.** A floating thing whose place is x (distance d from the centre, outward unit n) is displaced by

```
pushed(x, tau) = n  D(d)  ease((tau - (d - R) / c) / ride)        D(d) = D0 sqrt(R / max(d, R))
```

`ease` is 0 before 0, 1 after 1, smooth between (3u^2 - 2u^3). It is a closed form of the clock and the thing's own
place, so the game and the art read the same number. The final offset is proportional to the local height of the wave.
Its greatest speed is `1.5 D(d) / ride`; the constructor refuses a shove whose speed at the disc's edge exceeds c
(mine: a thing cannot be pushed faster than the wave that pushes it; with it, the thing slides from the front back
through the crest as it is carried, which is what riding a front looks like). The thing's height and heel are not
animated at all: a boat placed with `afloat` at `x + pushed` reads the sea's height there, ring included.

**Why an aftermath and not a third effect.** The effect says what the attack does to its target and what it owns
(nothing, or a held thing). The wave is what it does to the water, and any effect may have one. As a third `Effect`
variant, "instant with a wave" would have to repeat the instant's handling in every dispatcher and a hold could not
have a wave at all. So it is a second sum type beside the effect: `Aftermath = still | ring`, one dispatcher.

**Cost.** The ring is a `SeaDeform` whose `reaches` is the live annulus (between the front and the tail, after the
slam, inside `reach`): it is false everywhere when no ring is running, so the sea's deformed cases run on no pixel
then, and on the annulus only while it runs.

## The crab, predicted (base variant)

The loop is 18 s (was 16): the Grab closes at 3.0, Claw Crush arms at 11.5. Both boats jump back to their places at
the loop's seam (t = 0), where both attacks are over.

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
| boat two's final offset (D0 = 4.6, ride 2.4 s) | 3.10 outward, complete at t = 15.31 |
| its greatest speed | 1.94 (0.66 c); at the disc's edge 2.88 (0.99 c) |
| before the front | pushed and rise exactly 0 |

From the `game` camera boat two moves about 117 pixels at 1280 x 800, a little more than its own length on screen.

**Checks.** All five variants pass, with the throw written in the ship's own axes (keel, starboard, up) so each
variant's ship is drawn back relative to where its own claw takes it: `thrown` (velocity continuous at the release,
above its floating height until touchdown, lands where and when stated, travels at least 12), `fliesClear` (the flying
ship keeps clear of the carapace and the atoll), `gripReaches` over the hold including the stroke, `shoves` (boat two
ends at least 2.5 out) and `frontFirst` (nothing at boat two moves before the front).

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

## What is inherited and what is mine

Inherited: ballistic flight; constant acceleration as the stroke; 1 / sqrt(r) spreading; the shallow-water speed;
linear drag and a damped spring; anticipation, follow-through and squash from the twelve principles.

Mine: velocity continuity as the single rule across grip, stroke, air and water, the touchdown included, so plunge
and skid are derived; stating the wind-back and deriving the release; one gravity for the throw and the wave; the
shove as a closed form bounded by causality and by the wave's own speed; the aftermath as a second axis beside the
effect; the ring's reach as the live annulus so the sea pays for it only where and when it exists.
