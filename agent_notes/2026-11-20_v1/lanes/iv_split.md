# Pre-fanout integration

Main includes w16-builtins (`797405d`), v15-motion-mood (`3d6578b`),
v15-context (`bf4edd7`), and w16-splitter (`b68871e`).
The merge retains all six new workspace members. The context command reads
quantization signatures from the checker after the split.

The CLI check exposed a private reference walker needed by WGSL emission.
`c64b158` exposes that method and imports the existing step for CPU finite differences.

- Builtins passed 4 tests, Tier 0 passed 5, and CLI context and fork passed 2.
- Built `target/release/loft` with two Cargo jobs through the shared build lock.
- Release `loft context` exited 0 at 1,319 words and listed both
  `stdlib::motion::springs` and `stdlib::motion::mood`.
- Release `loft check scenes/showcase/mood_springs.loft` passed with six modules,
  five passes, and a Creature Lipschitz bound of 4.6178.
- Static checks passed. Earley and llguidance classified 314/314 inputs as expected,
  including the rejection fixture. The corpus contained 295 distinct Phase 5 identities.
