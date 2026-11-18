# Brief: Loft authoring API, every open concern, clean solutions and a work plan (18 Nov 2026)

You are designing, not implementing. Read `~/git/loft` (AGENTS.md, DESIGN.md's authoring sections, docs/guide.md,
docs/api.md, `src/expr.ts`, `src/core.ts`, `src/compile/context.ts`, `src/lib/wgsl.ts`, `src/check.ts`, `src/ranged.ts`,
`src/geom/solve.ts`, a few `examples/`), the plan `docs/proposals/2026-11-18_next.md`, and the lane briefs in
`agent_notes/2026-11-18_cook/` (lanes now running: `spray`→generic points, `volume`, `dof`, `f16`, `pbrsky`, `small`,
`compute`, `pipecache`, `rustplan`; their worktrees are `~/git/_wt/loft-<lane>`, read their diffs to see the shape
they are taking). Do not edit any repository except to write your one output file.

## The owner's positions
- The model writes as much code as it likes: any maths, any algorithm, close to an arbitrary compute graph. Libraries
  save tokens, never fence it in. "It's mostly shader code"; learning names is fine, host language matters little.
- Looks are swappable parts; everything extensible; there is always an escape hatch; fast and light.
- The reward is mainly aesthetic; checks and `solve` are authoring tools that must never say something false.
- A new name must buy a capability. Be critical, not defensive, about what exists.

## The concerns raised (address every one; add any you find)
1. **Maths as nested calls.** `sub(length(sub(vec3(abs(p.x), p.y, p.z), vec3(0.35, m, 0))), sqrt(f(0.09)))` instead of
   `length(vec3(abs(p.x), p.y, p.z) - vec3(0.35, m, 0.0)) - 0.3`: 2-3x tokens, unreadable, error-prone, and every shader
   trick the model knows must be hand-translated. Idea on the table: author in WGSL text (a subset parsed into the same
   graph, so the CPU twin, checks and `solve` still read it; anything outside the subset renders as raw and is named
   opaque). Decide the authoring form (tagged template with `${}` holes for Loft values? `.wgsl` files? both?), the
   subset, how Loft values (time, inputs, params, tables, other scene functions) enter, how errors point at the line
   of WGSL text, and whether the call-form API stays (and for what).
2. **Hidden generated code.** `draw.points`, `march` etc. silently build vertex/fragment shaders; the author never
   sees them. Make the generated WGSL visible (command, error messages that quote it with the author's line).
3. **Escape hatches are uneven.** `wgslFn` (GPU-only, opaque to twin/checks), raw `Drawable` (must learn the binding
   layout: camera, time, tables), compute (being built in `feat/compute`), multi-pass/feedback (only `pass`/post).
   Design one coherent story: what a raw author writes, the prelude/bindings they get by name, how raw and graph code
   call each other, and the honest limits.
4. **Primitives should take shader code directly.** e.g. points taking `fn grain(i: u32, t: f32) -> Grain`; a volume
   taking `fn density(p: vec3f, t: f32) -> f32`; fields for `march`. Decide the general rule.
5. **Three vocabularies for one idea** (a condition over time): sampled checks (`staysClear`, ...), `solve` holds
   (`equal`/`atLeast` with `{at}`/`{from,to}`/`within`), ranged checks (`always`/`eventually`/`ge`). Unify.
6. **Model-facing tooling**: the plan cut `measure`/`why`/`try` to `loft check --at … --explain`; confirm or improve.
7. **Determinism tuple and precision**: tier, precision (`f16` lane), params, inputs — one recorded identity.
8. Anything else in the API you find that makes a model write more, guess more, or get trapped.

## Deliver: `~/git/loft/docs/proposals/2026-11-18_authoring_api.md` (4,000-7,000 words)
For each concern: the problem shown with real current code; the solution shown as **the user's code first** (a
creature with motion, a PBR look, a dust burst with points + volume, a compute bake, a check), read cold, then cut;
what it costs to build; what it replaces or deletes; how existing examples migrate (persisted data only needs
migration; code changes with its callers); the risk. Then:
- **A work plan**: lanes, each with owner files (no two parallel lanes editing the same files), dependencies on the
  running lanes, a pass/fail check, and an order. At most 4 GPU-heavy lanes at once; workers run focused tests only,
  the integrator runs one full suite on the merged revision.
- **A blind test** of the authoring change: the same briefs written by a model in call-form Loft, WGSL-text Loft and
  raw WGSL/three.js; measure tokens, compile errors, iterations and blind-judged looks; the pass mark.
End your final message with a 250-word summary.
