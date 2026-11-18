# Ranged checks (2026-11-18)

Checks that hold for every time in a range and every value of a param in a range, decided by enclosing the condition
over whole boxes of those leaves rather than at sampled frames. Each result is graded by its margin and comes with a
witness. Code: `src/interval.ts` (enclosures), `src/ranged.ts` (`always`, `eventually`, the search). Demos:
`examples/ranged.ts`, the clearance check added to `examples/serpent.ts`.

## Theory (written before any demo ran)

### What is computed

A condition is built from Loft's comparisons; its margin is the signal-temporal-logic robustness (Donze and Maler
2010): `ge(a, b)` is a - b, `le(a, b)` is b - a, `and` / `or` / `not` are min / max / negation, `always` over a range
is the infimum over it, `eventually` the supremum. The condition holds where the margin is >= 0.

The infimum is found by branch and bound: every box gets an enclosure of the margin over the whole box (a lower bound
for the infimum there); the middle of every box gets an enclosure at a point (its upper end is an upper bound for the
infimum). The box with the lowest lower bound is split; boxes whose lower bound exceeds the best point are dropped.
The search ends when the two bounds are within the precision (1e-4) and the sign is settled. So:

- `holds: true` is a proof (the lower bound over every remaining box is >= 0); `holds: false` is a proof (a point
  whose enclosure lies below 0, the witness); `undecided` only when the box budget runs out, or when the boxes that
  could hold the extreme cannot be split further (a width floor of 1e-9 of the range), and it says which.
- The margin is within the precision of the true extreme.

### The enclosure

Plain interval arithmetic was tried first on the serpent's head (placed on its neck's frame, which is a 48-step
rotation-minimising transport scan). At a single instant it contained the CPU twin; over 0.01 s it failed:
`normalize: the vector may be zero`, because each step's quaternion hemisphere test went both ways and the hull of q
and -q is everything. That is the wrapping effect: intervals forget that the 48 steps all depend on the same t.

So each component is held as a reduced affine form c + g . e +- r over the box's leaves (e in [-1, 1]^d), intersected
with an interval [L, H]. Linear operators are exact; a smooth one is its tangent at the middle of its operand's
enclosure plus the Taylor remainder (half a bound on |f''| times the radius squared). Prediction from that: excess
width is O(w^2) in the box width w (the interval's is O(w)), so near a smooth extreme the search needs boxes of width
~sqrt(precision / K) rather than precision / C. Measured on the serpent head after the change, before any check:
enclosures stay within 0.02 of the twin over 0.1 s boxes (the head tip's x in [9.9067, 9.9122] for t in [1, 1.1]).

Sub-graphs that read no bound leaf are computed as the twin computes them, in f64, unpadded: otherwise a structural
constant such as a station index k / span * span (exactly a whole number) becomes an interval across the integer and
`floor` reads two stations at every box however small.

### Demo 1: tunnelling (sampled frames pass, the ranged check fails)

A claw (a strand, round tip, so the tip joint's radius is 0) sweeps along x through a shell (a sphere of radius
R = 0.35 at (0, 1, 0)): tip x(t) = -4 + 8 smoothstep(t0, t0 + 0.2, t) with t0 = 0.4 + 1/120, so it crosses the centre
at t* = 0.508333 s at 60 units/s, half way between two 60 Hz frames. Condition: the tip joint stays outside the shell,
`ge(distance(tip, centre), R + r_tip)` for t in [0, 1].

- Sampled at 60 Hz (frames k/60): the nearest frames are t = 0.5 and 0.516667, where u = 0.5 -+ 1/24, s(u) = 0.437645,
  x = -+0.498843. Predicted sampled margin: **+0.1488** (pass).
- Ranged: the tip passes through the centre, so **margin -0.3500, holds false, witness time 0.508333**.
- Cost: the margin has a kink (|x|) at t*, so the bound tightens only when a box's middle lands within
  1e-4 / 60 = 1.7e-6 s of t*: about 19 bisections of the box holding t*, the boxes beside it dropped as soon as their
  enclosure clears -0.35. Predicted under 200 boxes and under 2 s.

### Demo 2: a landing over a parameter range (sampled frames fail, the ranged check holds)

A stone thrown from (0, 1, 0) at 45 degrees with speed `stone.speed` in [4, 6], g = 9.81, ballistic. A pad covers
x in [2.3, 4.5]. Condition: for every speed, at some time in [0, 2] the stone is in a 2 cm touchdown band over the
pad: `always([speed, 4, 6], eventually([time, 0, 2], and(le(abs(y), 0.02), le(abs(x - 3.4), 1.1))))`.

From the closed form (theory script, fine grid): touchdown x runs from 2.3308 (v = 4) to 4.4875 (v = 6). The
eventually-margin is band-limited (0.02) in the middle and smallest at the far end:

- Ranged: **holds true, margin 0.01557, witness speed = 6.0000, time = 1.0570**. Nested search: always over speed,
  eventually over time.
- Sampled at 60 Hz: the stone crosses the 2 cm band in 2 x 0.02 / |v_y| = 0.0065 to 0.0076 s, under half a frame, so
  for **116 of 201** speeds on a grid no frame is in the band: a sampled check of the same condition fails, wrongly.

### Demo 3: an existing creature (the serpent's head clears the sea)

`examples/serpent.ts`: the head (a strand placed on the neck's frame, 12 joints) stays above the sea surface (the
scene's own swell, as an expression of the point and time): margin = least over the head's joints of
y - r - sea.height(xz, t), for t in [0, 10] and `serpent.neckRise` in a range around its default 3.7.

- Predicted: the ranged margin is at most the sampled one (soundness), and they differ by no more than the clearance
  changes between frames: the head moves at most ~1 unit/s here, so **0 <= sampled - ranged <= 0.01**.
- Lowering the neck lowers the head, so the witness is predicted at the **lower end of the neckRise range**.
- Cost: two leaves, ~15k nodes per box after inlining the station tables (15 ms a box measured on the head joint
  alone). Predicted **200 to 2000 boxes, 5 to 60 s**.

## Results

`bun tools/ranged_demo.ts` (sampled = the CPU twin at 60 Hz frames; ranged = `decide`). Precision 1e-4, budget 20000
boxes, one process on the M-series laptop with other work running.

| demo | predicted | sampled 60 Hz | ranged | boxes | CPU |
|---|---|---|---|---|---|
| 1 claw through shell | sampled +0.1488; ranged -0.3500 at t = 0.508333, fails | +0.14884 at t = 0.51667 (pass) | **fails**, margin -0.34999 in [-0.35000, -0.34999] at t = 0.50833 | 100 | 93 ms |
| 2 stone onto pad, speed in [4, 6] | 116 of 201 speeds missed; ranged holds, 0.01557 at speed 6, t = 1.0570 | 116 of 201 speeds have no frame on the pad (least -0.0295) | **holds**, margin 0.01559 in [0.01552, 0.01559] at speed 6.00000, t = 1.05699 | 3685 | 123 ms |
| 3a serpent head over the swell, rise 3.7, t in [0, 10] | 0 <= sampled - ranged <= 0.01 | 1.70409 at t = 5.350 | **holds**, 1.70409 in [1.70403, 1.70409] at t = 5.35156 | 192 | 4.5 s |
| 3b the same, rise in [2.5, 4.5] | witness at rise 2.5; 200-2000 boxes, 5-60 s | (one rise only) | **holds**, 0.51726 in [0.51718, 0.51726] at rise 2.50003, t = 5.28320 | 3882 | 91.7 s |

Scored against the theory:

- Demos 1 and 2 landed on every predicted number (the 0.01559 against the theory script's 0.01557 is the script's
  1e-5 grid; the proven bounds hold 0.01557). Demo 1 cost 100 boxes, under the predicted 200 (24 of them are the
  witness polish).
- 3a: sampled and ranged agree to 1e-5. The serpent moves slowly at its lowest instant, so on this scene the sampled
  check was not wrong; the ranged one is a proof where the sampled one was a sample.
- 3b: the witness is where predicted (the lowest rise). **The cost prediction missed**: 3882 boxes and 92 s against
  200 to 2000 and 5 to 60 s. The prediction took 15 ms a box from the head joint alone; the clearance reads 12
  joints and the sea under each (24 ms a box), and boxes wider than about 0.3 s in time are unbounded (the frame
  transport), so the search must first cut time into ~40 pieces at every rise it looks at before any bound prunes.

### What the first runs got wrong, and the fixes (in order)

1. Bun 1.3.10 parses `f(a[1] < b[0], a[0] >= b[1])` as TypeScript type arguments: every comparison came out
   "either". Comparisons now test into locals (comment at the site, `icompare`).
2. Plain intervals: the wrapping effect above. Fixed by affine forms intersected with intervals.
3. Structural constants padded across whole numbers: fixed by computing sub-graphs that read no bound leaf as the
   twin does.
4. Demo 2 first came out undecided (width floor), then held with a loose bound [0.0100, 0.0156] at 20000 boxes. The
   ∀ speed ∃ time search held one witness time across a box of speeds, and the 2 cm band moves 0.67 per unit of
   speed, so a box's lower bound was O(width) loose. Three changes, the last decisive:
   - the inner search starts from the witness the enclosing search found at the box's middle (`Hint`);
   - the witness is polished by a 24-step pattern search (it only finds points; bounds stay proofs);
   - **the witness is a line, not a constant**: the inner time follows the outer speed, t(v) = t0 + s (v - v0), the
     slope from the witnesses of a split box's two halves, evaluated as an affine leaf (`Track`). Over a box of speeds
     the bound is then O(width^2). 20000 boxes and a 0.006 gap became 3685 boxes and a 7e-5 gap.
5. Demo 3 stalled on a 9e-9 s box at t = 1.3231 with `normalize` unbounded: there the first station's quaternion has
   w = 0, the scan's hemisphere test flips the sign of the whole chain (q and -q are one rotation, so the drawn body
   is continuous), and the hull of q and -q holds 0. An enclosure that is unbounded after a select went both ways is
   now re-evaluated with that select forced each way (at most 4 deep), and the cases' results hulled: at every point
   the value is one case's.

## Inherited and new

Inherited: interval arithmetic with outward rounding (Moore); reduced affine arithmetic with one error term (Stolfi
and de Figueiredo; Messine's AF1) and the Taylor-remainder linearisation of smooth operators; branch and bound with
best-first splitting (Hansen); signal-temporal-logic robustness for always / eventually / and / or / not (Donze and
Maler 2010); Skolem functions for ∀∃ (logic).

New here, as far as I know: (a) intersecting each affine form with an interval and replacing it by the interval when
it is twice as wide, so the cheap guarantees (squares are not negative, unit vectors lie in [-1, 1]) survive beside the
correlations; (b) evaluating constant sub-graphs exactly as the CPU twin does, so a search's subject is the twin;
(c) inlining per-frame tables (map runs and unrolled scans) so a body's whole pose is one expression of the leaves;
(d) affine Skolem witnesses: an inner `eventually` search's witness carried as a line in the outer leaves, with the
slope taken from the witnesses of the two halves of each split; (e) case splitting only where the hull of a select's
branches made an enclosure unbounded; (f) hashes and gradient noise enclosed cell by cell (exact hulls of the hash,
the noise's own polynomial within a cell).

## Limits

- **Operators refused** (exit 4, `RangeRefusal`): GPU-only library functions (`fwidth`, `viewDir`, ...), cellular
  noise and any library function without a rule; tables filled on the CPU (hand-written fills, static tables, baked
  recurrences: `recur`); a free variable no range binds, and `time` when no range binds it; `eq` / `ne` and a constant
  condition. Skins (a ship's hull distance, `skinDistance`) are CPU code, not expressions: unreachable.
- **Unbounded by box** (split, then undecided naming the operator at the width floor of 1e-9 of the range): a divisor,
  `normalize` or `log` operand that may be 0, `sqrt` of a possibly negative value, `tan` across a pole, a fetch index
  that may leave its table.
- **Codomain bounds** (sound, counted in `work.codomain`): hashes over more than 64 cells give [0, 1 - 2^-24], gradient
  noise [-1, 1].
- **Budgets**: 20000 boxes by default (`boxes`), shared by nested searches; precision 1e-4 in the margin's units;
  inner searches over a box take 8 boxes plus the polish; the width floor. `undecided` names the one that ran out.
- **Not exact**: the enclosure is of the real-number expression with constant sub-graphs as the twin's f64; the GPU's
  f32 (and the twin's f32 folding of constants) can differ by ~1e-7 relative per operation. The margin is within the
  precision of the extreme, and the witness is the point where that margin was evaluated, not the argmin to any
  stated distance in time. Margins of `and` / `or` mix units, as robustness always does. `gt` and `ge` are not told
  apart (margin 0 counts as holding). A margin exactly 0 over a stretch (contact) is undecided.
- **Not built**: and / or of two quantified formulas (a check list is their conjunction; ∀ distributes over and,
  ∃ over or); STL's windows relative to an outer time (`always [0, 4] eventually [0, 0.5]`) — a leaf is bound once.
- **Cost**: a box costs one evaluation of the whole inlined graph in an interpreter (12 to 24 ms for a body's 15k
  nodes); bodies need boxes under about 0.3 s. Compiling the evaluator to JavaScript, as `compileJs` does for points,
  is the next lever.
- The API is not in `loft context`/`src/index.ts` yet: import from `src/ranged.ts`. The two not-built items decide
  whether `always` / `eventually` keep these signatures.
