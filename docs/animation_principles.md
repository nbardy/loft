# Lively motion: the twelve principles, as Loft formulas

The principles are from Ollie Johnston and Frank Thomas, *The Illusion of Life: Disney Animation* (1981). The
summaries below follow the Wikipedia article "Twelve basic principles of animation", read on 2026-11-11. The helpers
are in `src/geom/lively.ts` (new) and `src/curve.ts`, `src/geom/motion.ts` (existing). All are pure functions of a
clock: they keep no state between frames, so they run wherever pose runs.

Use this when a monster moves correctly and still looks dead.

## The twelve

| # | Principle | What it says | In Loft |
|---|---|---|---|
| 1 | Squash and stretch | Weight and flexibility come from a form deforming as it moves. Volume stays the same: a stretch along is matched by thinning across | `squash(along)` gives a size that keeps volume. `stretchOf(speed)` is the length to wear in flight; `impact(after)` is the squash and spring-back at a stop |
| 2 | Anticipation | A move the other way first tells the audience what is coming | `anticipated(u)` dips before it drives. `beats(since, { anticipate, act, settle })` gives the wind-up its own beat. `response(t, { r: < 0 })` starts the wrong way |
| 3 | Staging | Present one idea so it is "completely and unmistakably clear": placement, light, camera, no needless detail | Not a formula. Check it: `minScreenSize`, the `game` camera's silhouette, one action at a time. See the checklist below |
| 4 | Straight ahead and pose to pose | Animate frame by frame for fluid action, or set key poses and fill between them for control | Formulas of time are straight ahead. `keys` and `through` are pose to pose. Use poses for the readable extremes and formulas for what flows between them |
| 5 | Follow through and overlapping action | Loose parts keep moving after the body stops. Parts move at different rates; trailing parts take time to catch up (drag). A still character keeps a little motion (the moving hold) | `swing(after, amount)` for what keeps going. `dragged(t, along, lag)` for the clock a trailing part reads. `alive(t, amount, seed)` and `breathe` for the moving hold (give `alive` a `period` where the motion must loop). `response` for a part that settles |
| 6 | Slow in and slow out | Things take time to speed up and to stop: more of the time is spent near the extremes | `slowInOut(u, power)`, `slowIn`, `slowOut` |
| 7 | Arc | Natural movement follows arcs; straight lines read as mechanical | `arc(a, b, u, lift)`. Prefer turning a part about a joint to sliding it |
| 8 | Secondary action | A smaller action supports the main one and must not distract from it | `echo(f, t, delay, gain)`: a later, smaller copy of the main motion |
| 9 | Timing | The time an action takes sets its weight and its mood: light things react fast, heavy things slowly | `beats`. Fewer seconds in `act` reads light and sharp; more reads heavy |
| 10 | Exaggeration | Stay true to the action and push it further than life | `exaggerated(x, rest, by)` |
| 11 | Solid drawing | Forms have volume, weight and balance. Avoid "twins": left and right sides that mirror each other look lifeless | `uneven(part)` gives each side its own factor. Offset the clocks of a pair with `alive(..., seed)` |
| 12 | Appeal | The character is interesting to look at, villains and monsters included. Hard-to-read faces lack it | Not a formula. See the checklist below |

## How to use them on a monster

Work through this in order. Each step is one or two lines in a scene.

1. **Give every action three beats.** `const b = beats(sub(time, attack.arm), { anticipate: 0.6, act: 0.15, settle: 1.2 })`.
   The wind-up should be several times longer than the action.
2. **Wind up the other way.** Drive the pose by `anticipated(b.anticipate)` so the claw rises before it falls and the
   neck pulls back before it strikes.
3. **Land with weight.** Stretch along the motion during `act` (`squash(stretchOf(speed))`), then `impact(b.after)` at
   contact.
4. **Let loose parts arrive late and keep going.** Read tails, whiskers, fins and weed on `dragged(time, m, lag)`, and
   add `swing(b.after, amount)`.
5. **Never hold still.** Add `alive(time, small, seed)` to every resting angle, with a different seed per part.
6. **Break the mirror.** Multiply one side's size, rate or angle by `uneven(side)`.
7. **Move on arcs.** Swing parts about joints; where a thing travels, use `arc`.
8. **Push it.** After it works, multiply the departures from rest by 1.3 to 2 with `exaggerated`, then look.

## Checklist for the two that are not formulas

**Staging**
- From the `game` camera, is there exactly one thing the eye goes to?
- Does the action read in silhouette, with paint switched off?
- Is the warning visible before the action starts, and nothing else moving at that moment?

**Appeal**
- Can the face be read at the size it will be seen?
- Does it have one temperament that every feature serves?
- Is anything on it symmetrical that need not be?

## Grabs

`src/geom/grab.ts`, shown in `examples/grab.ts`. A grab is one clock and four beats: reach, close, hold, release.

- `grab(since, { rest, from, to?, radius, squeeze, pulses, ... }, beats)` gives the palm's place, the `gap` between
  the gripping surfaces, the held thing's place, its squeezed size and a `held` flag. The gap follows the thing's
  `radius`: it opens to the thing's width plus a margin by two thirds of the reach, closes to the width, and narrows
  by `squeeze` on each pulse. The thing's place and radius may be host inputs, so one formula takes any ship.
- A squeezed thing keeps its volume: narrowed to k across the grip, it grows by 1 / sqrt(k) the other two ways.
- `windup` makes the gripper draw back from the thing before it reaches.
- A gripper is whatever turns `gap` into a pose. `fingerAngle(gap, length)` is the angle for a claw's two fingers or
  a jaw; `pincerAngle(gap, length)` is the opening of a pincer with one fixed finger (a crab's claw). `wrap({ centre, axis, radius: gap / 2 + body, length, pitch })` is a coil whose number of turns follows from
  the body's length, so it goes round a small ship more times than a large one and gains turns as it tightens.

A whole ship takes the same size: `ship.placed(chart, wind, grab.squeezed(1, "z"))` squeezes hull, sails and rigging
across the beam as one, and `shifted(chart, offset)` lifts it off the water it floats on (`sized` and `shifted` in
`src/map/chart.ts`). Spars keep their thickness. Nothing checks that the gripper reaches the thing.

## What is not here yet

- Checks for these principles (for example, that an action's wind-up is longer than its action).
