# Chief scientist on the backend and escape-hatch plan, 20 Nov 2026 (verbatim)

<pasted_content id="ac93">
That is a grounded, non-defensive response—and **20–37 ms overnight build times** plus the **"never slower than hand-written" control gate** directly answer your main worry. Keeping one hand-written WGSL hot pass per shipped engine and failing CI if Loft's emitted code drifts more than a few percent turns "did our compiler quietly become a ceiling?" into an automated, falsifiable check rather than a debate.

Their call to defer a pure Metal backend until profiling proves tile memory (`VisBuf` staying on-chip) or Indirect Command Buffers actually win real milliseconds on Hermit/Shrine is also the right discipline.

When Astra's `docs/proposals/2026-11-20_backends_and_escape_hatches.md` (`bk1`) lands, there are **three specific details** worth checking in their side-by-side emitted code:

1. **Verify the FMA/fast-math boundary in emitted shading code:** They flagged that the twin-mismatch fix disabled FMA contraction for motion determinism and *must not* leak into shading. Have Astra confirm in the actual emitted shading MSL/WGSL that raymarching and BSDF loops still get full FMA—an accidental global flag there can easily burn 10–15% of frame time on 20 crabs and Coral Shrine.
2. **Check how `msl(...)` passthrough handles `wgpu`'s buffer-slot mapping:** `wgpu` supports both raw MSL text and precompiled `.metallib` passthrough via `Device::create_shader_module_passthrough`—projects like Burn/CubeCL already use this exact `wgpu` + MSL passthrough hybrid to hit `bfloat` and Metal-native ops without abandoning `wgpu`. The catch is that `wgpu` maps `@group(g) @binding(b)` onto flat Metal `[[buffer(n)]]` / `[[texture(n)]]` indices (plus internal buffer-size slots for runtime-sized arrays). Astra's design should have Loft **auto-emit the MSL function signature / resource bindings** from `PassDecl.bindings` rather than making an author hand-number Metal slots that break whenever a pass changes.
3. **Keep the control gate honest about work shape vs. inner-loop math:** Comparing a Loft-emitted `shade_tiles` shader against a hand-written WGSL `shade_tiles` shader will catch codegen regressions (inlining blowups, register spills, FMA bugs), which is huge—but it won't catch a bad **pass shape** (their point #2). Make sure `bk1` confirms the full-pass raw WGSL escape hatch with custom bindings actually works in code today, not just function-level `wgsl { ... }`.

Nothing here needs pushback—they're already auditing the code instead of trusting the spec, and the control-gate proposal is genuinely stronger than what we had before. Send over Astra's `bk1` doc once it's in if you want a second pair of eyes on the emitted shaders.
</pasted_content id="ac93">
