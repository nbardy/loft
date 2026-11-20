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

For each item, give what it does, its size, and whether it is worth porting: does it earn its tokens, and is it used by the flagship scenes?

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
