# Loft, round 1 of a critique: the lead's case against our own work (20 Nov 2026)

To the chief scientist. This opens a back-and-forth. I am arguing as a critic of what we built, not defending it.
Rebut what is wrong, then tell us what to change in the meta engine, the library system, the design and the
language. Public repo: https://github.com/nbardy/loft (main), or the text-only `review` branch for import.
Key evidence files:
- `agent_notes/2026-11-20_v1/AB_OLD_VS_NEW.md` (matched A/B)
- `OPEN.md`
- `design/{d2,pf1,tok1,std1,lean1}.md`
- `docs/proposals/2026-11-20_{backends_and_escape_hatches,consumer_passes_astra}.md`
- your own `design/scientist_review.md`
- `docs/proposals/2026-11-20_capability_gaps.md` (Astra, in progress)

Where things stand (measured on M4): main is v1.0-alpha plus the waves since. Gates: Pictures 19/19 (after
re-pinning the approved looks), Twin 36/36, Truth 34/34, and 157 native tests passing. Speed and Load read 6/12 and
23–25/33 under contention; frames are unchanged. Hermit-20 runs at 3.75 ms after shadow-map reuse. The Shrine
Lipschitz fix took the lantern's L from 22.9 to 1.0. A Three.js demo shares one GPUDevice. Exactness is now
**opt-in** (owner ruling): pictures and the twin are judged by tolerances.

## The critique, from most to least serious

**1. The architecture explains our scaling, not our speed.** The matched A/B (same scene and features, PSNR 47–56 dB)
shows the new engine about **equal** to the old on light lighting. On the old engine's heavy lighting it is **about
2× slower** (20 groups: 58 ms new against 27 ms old), because our software four-tap path costs 27–32 ms more. The
earlier "70–89% faster" was mostly less work. Kinds and rows buy flat cost per instance and flat module counts. They
do not buy per-pixel speed. Our per-pixel shading and lighting code is untuned and, in places, worse than the old.

**2. "Tiny core" is not true.** The Rust is 33.2k lines: passes 15.9k, cli 5.7k, std 4.5k, parse 2.7k, runner 1.6k
(983 by the cut's own count), ir/abi/format/plan 2.7k. The lean audit set ceilings totalling 18.0k, and **all four
lean lanes failed to merge**. loft-std still carries ~2.4k lines of wgpu device and encoder code, and loft-cli carries
a duplicate ~1.9k-line native renderer. A meta engine that preaches "small core" while its compiler grows unchecked is
repeating the old engine's failure mode at a different layer.

**3. "Never lock in" is currently false at the pass level.** Function-level `wgsl(...)` works. Whole raw WGSL passes
(s1_escapes) are held, unmerged. Native MSL passthrough is designed but not built. Until whole-pass and native
hatches ship, the compiler owns pass shape, march policy, tile size and layout, which is exactly where bk1 found the
compiler's ceiling.

**4. Kinds as code identity scale count, not variety.** Domain repetition and instancing are free. But an LLM-driven
creative tool produces *variety*: thousands of unique procedural materials and shapes. Each unique kind is a module
and a pipeline (~70 ms on Metal). The fallback, the Tier 0 tape VM, costs +2–5 ms. We have no data-driven material
path: no material-family uber-shaders over parameter tables, no bindless textures, no auto-merging of kinds under a
pipeline budget.

**5. The language does not stop bloat, and std is not the lever.** The median corpus scene is **96,204 tokens**,
against 3,021 for the hand-written hermit, yet all 170 seed programs were under 5,511 tokens before expansion. Authors
(models) write longhand: unrolled rows and repeated blocks. tok1 found that std functions save ~0.3% on hand-written
scenes, while row defaults alone would save ~986k corpus tokens. The language needs anti-bloat by construction:
- generators and row families as the idiom;
- a per-scene token budget gate;
- a lint that rejects repeated literal blocks.

**6. The style/integrator/schedule/engine split is unproven.** std1 promises "the same woodblock style under
realtime and reference transport". NPR looks are defined against a lighting model, not physically transported light.
What does woodblock mean under a path tracer? There is no written contract between style and integrator (what a style
may read: radiance by term? light IDs? visibility?). Without it, "swap any ingredient" is a slogan.

**7. Our gates measure noise.** Speed and Load swing 12/12 → 6/12 and 31/33 → 23/33 on unchanged code while lanes
build. Integrators have blocked and unblocked merges on that noise for a day. We need one of:
- a quiet GPU window (no builds during timing);
- paired same-run A/B with confidence intervals as the only acceptance;
- off-machine gating.

**8. The motion story is the least proven part.** Authored solvers only just matched the twin after the FMA fix
(x2b, unmerged: its 1,000-crab dispatch is over budget). No flagship motion scene (the 5-crab lineup dance,
crab_live) has been ported yet. Modern GPU physics (MPM, FLIP, SPH) is not designed in. The fixed-point atomic tier
is deterministic by associativity, and the float-atomic fast tier lives outside the opt-in.

**9. wgpu and WebGPU ceilings decide our beauty ceiling, and we have not decided the target.** Hardware RT
pipelines, mesh shaders, 64-bit atomics (needed for a Nanite-class software vis-buffer), cooperative matrices (neural
shading), vendor upscalers, async compute and full bindless are all missing or experimental. If web is first-class,
we accept the lowest common denominator. If native is first-class, the deferred Metal backend is a *capability*
question (hardware RT, matrix cores), not the 0.1–0.2 ms tile-memory question bk1 priced.

**10. Consumer compiler passes are premature.** Astra's Wasm pass design (~1.5k lines) is good, but you already said
to wait until check.rs (86.5 KB) is split and built-in kernel and whole-pass lowering work. I agree, and I'd go
further: no new language machinery until the core is under its ceilings.

## What I propose (attack these)

1. **Freeze features; spend two waves on shrink and speed.** Hard per-crate ceilings enforced by `tools/check.sh
   lines`; delete the CLI's duplicate renderer; make loft-std wgpu-free; fix the heavy-lighting path and bake the sky.
   Accept only matched, paired speed numbers.
2. **Ship the hatches before anything else in the language:** whole-pass WGSL, `msl(...)` passthrough with
   auto-generated slots, and the 5% whole-pass parity gate.
3. **Add a data-driven material path beside kinds:** material families as uber-shaders over parameter tables, plus
   bindless textures where available, plus compiler auto-merge under a pipeline budget.
4. **Make bloat a compile error:**
   - a token budget per scene;
   - a repeated-block lint;
   - row generators as the documented idiom;
   - regenerate the corpus to a median under 10k tokens.
5. **Write the style↔integrator contract** (what a style reads and writes, per integrator class) before porting 54
   port-rewrite items into stdlib.
6. **Fix measurement:** a quiet-GPU timing window in the conductors, and paired confidence-interval acceptance.
7. **Decide the target:** native-first, with web as a capped tier (my lean), or web-first. That decides the Metal
   backend and hardware RT.
8. **Physics tiers:** fixed-point deterministic P2G as default, a float-atomic fast tier as opt-out; measure both.

## Questions for you, ranked

1. Is critique 1 right that our per-pixel path is the problem? Where would you look first: the four-tap path, sky
   sampling, BSDF cost, march steps?
2. Is 18k lines the right ceiling for this feature set, or is a smaller core possible if more moves into `.loft`
   std? Where is the line between compiler and library?
3. How should variety work: uber-shader families, a material VM done right, or compile-on-demand with a pipeline
   cache? What has the best ceiling for LLM-generated content?
4. What is the style↔integrator contract? Can an NPR style be integrator-agnostic, or are styles tied to a lighting
   model by nature?
5. Language: what construct makes the 96k-token corpus impossible to write? Generators, families, a token budget, or
   something better?
6. Native-first or web-first? What would you have us give up?
7. Which of my eight proposals would you cut, and what is missing?

Reply in Markdown, one section per question, numbers marked measured or predicted. We will answer point by point
in round 2.
