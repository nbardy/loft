## Your lane: `spray` — many small grains on any path (worktree `~/git/_wt/loft-spray`)

Goal: a dust burst around an object (think a football boot exploding out of red clay: 100k–1M fine grains, plus the
same paths reused by the soft medium's puffs). Today `src/soft/rain.ts` does hashed instancing (index → hash → place,
nothing stored) but only for falling streaks.
- Build one drawable for N grains whose position is **any expression of (index hashes, time)** the author writes
  (ballistic with drag is just an equation; give a library helper only if it saves real tokens). Size, colour,
  opacity per grain also expressions. Camera-facing specks, sub-pixel grains keep a pixel and fade (no shimmer), motion
  stretch optional. Lit by the sun/lamps, and **attenuated by the soft medium** (`veil`, src/soft/medium.ts) so grains
  inside the dust cloud are darkened/coloured consistently; depth-tested against bodies.
- Decide whether `rain` becomes a use of it (if so, port it with no visual change, measured).
- Example: `examples/spray_burst.ts` — a burst from a point with a fan bias, puffs from the soft medium on the same
  equation, a warm backlight. Report ms/frame at 1280x720 for 100k and 1M grains.

## Owner decision (added after launch; the reviewer enforces it)
No dust-specific name. The library gives one generic primitive: draw N small things from an index (e.g.
`draw.points(n, (i) => ({ at, size, colour, opacity }))`, i giving hashes), with the drawing machinery (instancing,
sub-pixel fade, depth, lighting, medium attenuation) and nothing else. Burst shape, ballistics, drag, fan bias live in
the example as the author's own code. If the implementer built a `spray` with built-in burst options, the reviewer
reduces it to that primitive (or renames it, if it already is one) and moves the burst into `examples/`.
