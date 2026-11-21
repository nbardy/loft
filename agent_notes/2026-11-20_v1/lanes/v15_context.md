# v15_context

- `loft context` selects public declarations with the parser AST, keeps their
  authored units and defaults, and prints them in stdlib module path order.
  Compiler tables supply the core vocabulary. Sections select module families
  or public schedules and kernels.
- Prediction: 38 current .loft modules, one headless example, full output
  under 2,400 words and 200 ms on a warm local checkout.
- `loft fork` validates imports at the destination before publishing the copy,
  rebases `self::` edges to stdlib and prints a sibling import. Loose entries
  resolve stdlib via the invocation checkout or the executable's build checkout.
- CLI tests check the context budget, every Loft fence and a fork edited and
  checked outside the workspace. The downstream integrator runs Rust tests.
- The existing release binary accepted the example, Realtime as an entry and
  a scene importing the copied recipe. Both grammar recognizers accepted the
  example. Runtime context size and latency await the integrated binary.
