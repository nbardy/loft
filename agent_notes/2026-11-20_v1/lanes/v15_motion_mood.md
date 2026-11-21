# v15 Motion and Mood

- Base: origin/main at 78ef244. The lineup score was read at 3c6f053.
- Spring response inherits the exact constant-target scalar solve. The tail
  runs four scalar solves per tick, fed by each parent's prior angle and rate.
- Squash determinant is one in real arithmetic. Action landmarks at progress
  0, .24, .62 and 1 have zero first and second derivatives.
- Mood uses a typed packed frame value supported by Loft's expression model.
  Use mood(...) for custom channels and mix_mood for quintic transitions.
- The showcase holds each capture-point pin between swings, then poses the
  leg with the existing two-bone IK. Nominal capture lead uses one cycle/s.
- Predicted view: short guarded steps, a lifted alert crest, a springy
  jaunty walk, a held strike with tail follow-through, then a slower slump.
- Binary grids on persistent motion target 0 divergent ticks. Parity,
  loft lint and still/clip remain unmeasured under the verification scope.
- No Rust builds or changes.
- Existing release binary passed loft check with six modules, five passes
  and a derived Creature Lipschitz bound of 4.6178.
