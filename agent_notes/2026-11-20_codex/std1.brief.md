Effort: xhigh

You are Loft's standard-library architect. This is a design pass, with probes allowed. Repo ~/git/loft (main).

**The owner's direction (20 Nov):**
- Port the good parts of the old standard library.
- Organise it as a tree: stdlib/noise, stdlib/sdf, stdlib/styles, stdlib/lighting, and so on, nested where useful.
- Higher layers import the small pieces. For example, stdlib/styles/woodblock imports stdlib/noise and stdlib/lighting.
- The recommended way to customise is **copy-paste to fork**: copy a std file into your own folder and edit it.

Read:
- the std rule and anti-patterns in agent_notes/2026-11-20_v1/OPEN.md: a function is justified only if it replaces a recurring, complicated block of ≥3× its call's tokens, and there are NEVER renamed wrappers for map/await/filter or one-line helpers;
- docs/PHILOSOPHY.md §0.5 and §4;
- agent_notes/2026-11-20_v1/design/tok1.md (the token study), when present;
- lib/*.loft today (607 lines, flat; crates/loft-cli/src/main.rs:10 loads every lib/*.loft as the prelude);
- the `use` import syntax (scenes/spectral_caustic_study.loft:4 `use engines::spectral_caustic`).

**Inventory the old library** at tag legacy/2026-11-19. Read it with `git show legacy/2026-11-19:<path>`; never check out over main. Cover:
- src/style/*: woodblock, toon, sketch, paint, papercraft, neon, biolume, gloss, pbr, pbr_parts, sea, whitewater, foam, sky, post, light, sheet, shared;
- src/light/*.wgsl: transport, interface, key, albedo_fill, history;
- src/kinds/ggx.wgsl;
- src/motion/*: body, move, roam, and the wgsl/ directory;
- the noise, geometry and SDF helpers wherever they live;
- examples/bosses/lib.

For each item, record:
- what it does;
- its size in lines and tokens;
- the flagship scenes that use it;
- three findings:
  - **Pattern fit.** Does it map to kinds and rows, staging, spaces and forkable `.loft`? Or does it rely on what we designed out: host-side loops that unroll, per-instance constants baked into code, styles welded to paint words, parameters that can only be changed inside the framework?
  - **Speed.** Use known measurements where they exist (agent_notes/2026-11-18_cook/*.final.md, for example the volume march at 20–61 ms, AO at +0.066 ms, f16 at no gain, the old profiles in docs/legacy). Otherwise estimate the per-pixel cost: samples, march steps, octaves, passes.
  - **Flexibility.** Can a consumer change the look or behaviour by editing ordinary code, or are decisions sealed inside it?

Then give ONE verdict per item:
- **port**: fits the pattern, is fast enough and flexible; port it as is, rewritten idiomatically;
- **port-rewrite**: the idea is worth keeping but the implementation is slow, inflexible or off-pattern; say what changes;
- **reference-only**: keep as inspiration only (a note in stdlib/REFERENCE.md pointing at the legacy path); do not port;
- **drop**: superseded or not worth its tokens.

Put this in a table in std1.md, section 0 "Old stdlib audit", BEFORE the tree design. The tree may only contain items with a port or port-rewrite verdict.

Write agent_notes/2026-11-20_v1/design/std1.md containing:
1. **The stdlib/ tree:** directories, files and layering. Lower layers (math, noise, sdf, frames) import nothing above them, and the layers above import downward. List every module with its public functions as typed E+ signatures.
2. **Module mechanics:**
   - how the loader finds nested modules (`use stdlib::styles::woodblock`);
   - whether everything is still a prelude or modules are imported explicitly. Recommend one, keeping a small always-loaded core prelude.
   - name scoping;
   - how forking works. A copied file under the scene's own folder shadows or replaces the std module by an explicit `use` of the copy. No magic overrides.
3. **The forking guide:** text for stdlib/README.md that tells people to copy-paste a std file to fork a style, lighting model or gait, and shows exactly how.
4. **The port plan:** which old items go to which module, rewritten idiomatically. Kinds and rows; no unrolled loops; no tiny renamed wrappers.
5. **Precompilation:** confirm the plan works with lane stdaot1, so each std module compiles once (content-hashed) and is reused.
6. **Lane briefs, in the shape of agent_notes/2026-11-20_codex/w1.brief.md, each starting with an `Effort:` line:**
   - std1a: the restructure. Move lib/ into the stdlib/ tree, make the loader and `use` resolution nested, update every engine and scene. Pixels stay bit-identical. This lane runs FIRST, alone.
   - then at most 4 parallel port lanes with disjoint directories (for example noise+sdf+geometry; styles; lighting+sky+post; motion+bodies). Each port lane renders a small showcase scene per style into out/meta/<lane>/approval/ as WebP q95, for the lead to approve.

   Name them std1a.brief.md and std1b..e.brief.md, and list the parallel ones (not std1a) in agent_notes/2026-11-20_codex/std1.lanes.

Commit by path to main and push. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Final message ≤150 words.

**Addendum (lead, 20 Nov): styles vs engines.** In the old engine, a style was a *shading recipe* applied over the same raster and SDF-march visibility:
- a paint treatment;
- a lighting recipe (light.ts, 591 lines);
- a shade function;
- post passes.

One style was PBR (GGX, Smith, Schlick, sky environment). The rest were non-photoreal: woodblock, toon, sketch, neon, biolume and papercraft, plus shared print parts (sky, sea, foam, whitewater) and gloss. Keep that separation explicit in the tree, as layers:
- stdlib/{math, noise, sdf, geometry, frames};
- stdlib/{materials, lighting, sky};
- stdlib/styles: how a surface looks — paint treatment, shading recipe, ink and outline;
- stdlib/integrators: how light is transported — direct, ReSTIR, the path-traced reference, spectral;
- stdlib/schedules: how the frame is built — vis-buffer raster plus tile shading, forward, path tracing;
- stdlib/post;
- **stdlib/engines** = a composition of one schedule, one integrator, one style and post. For example `woodblock_realtime` = vis-buffer schedule + direct light + woodblock style + print post. The current engines/*.loft files move here.

A style is NOT an engine. The same woodblock style should run under the realtime and the reference engines. Show this composition in std1.md, and include one example of swapping a style under a fixed engine, and an integrator under a fixed style.

**Corpus bloat (from tok1, 20 Nov):** the median corpus scene is 96,204 tokens against 3,021 for the hand-written hermit, and row defaults alone would save about 986k corpus tokens. Corpus ports are written longhand instead of with kinds and rows. Add a corpus-regeneration lane (std1f) that rewrites the corpus idiomatically with kinds, rows, row defaults and the stdlib, keeping pictures matched. Target: median under 10k tokens.
