Effort: xhigh

You are a Loft engineer working in ~/git/loft. Seven surface styles and shared print parts. Read AGENTS.md, CORE.md, AGENT_CARD.md, docs/PHILOSOPHY.md §§0.5 and 4, docs/spec/README.md (07 > PATCH_V1_2 > 06 > 00–05), agent_notes/2026-11-20_v1/OPEN.md and agent_notes/2026-11-20_v1/design/std1.md and tok1.md in the same directory. Read legacy files with git show legacy/2026-11-19:<path>.

Rules:

- Use `git cambium add -b loft/std1c -name loft-std1c -ephemeral ~/git/_wt/loft-std1c origin/main`. Record the base SHA. Start only after std1a is merged. Rebase on that scored revision.
- Own only stdlib/styles/ (including print/), scenes/showcase/std1c/ and your lane note. Write measured before/after values, commit SHAs and evidence paths in agent_notes/2026-11-20_v1/lanes/std1c.md.
- Predict the measurements first. Keep kinds and rows, typed spaces and stages, exhaustive boundary errors and ordinary forkable source. A new public function needs recurring complicated blocks at ≥3× the full call cost. Never add one-line conveniences or renamed map, await, filter or fold.
- Iterate on compile errors with `CARGO_BUILD_JOBS=2 tools/dev.sh check loft-cli`. Run the focused checks, static checks and the workspace suite. Serialize GPU work with tools/gpu_lock.sh. At most two full-suite/GPU reruns after the first scored run. If a failure repeats without its cause narrowing, escalate with the first differing row, pixel or pass.
- Keep frozen RGBA references and approvals intact. Use lossless inputs for hash/diff. Store evidence under the primary checkout’s out/meta/std1c/ before Cambium cleanup. Render a small runnable showcase per style this lane implements or uses. Save actual WebP q95 images in out/meta/std1c/approval/ with scene, camera, time, command, SHA and adapter metadata for lead review.
- Commit by owned path, push loft/std1c, and leave merging and picture approval to the lead. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>".

Task:

1. Implement the seven audited styles: woodblock, toon, sketch, papercraft, neon, biolume and PBR. Keep woodblock_pigment as the frozen route. Each new shade(Surface,Lighting)->rgb owns surface appearance. Papercraft also supplies fold_surface before transport so actual light samples see its facet normal. Port anchor-space paint, swell lines, two-channel foam, rolling whitewater and the bounded impact patch. Call noise and the lighting/sky modules explicitly. Copyable thresholds and palettes belong in these files. Give no style ownership of the renderer, visibility source, full bounce loop or named paint-word dictionary.
2. Use drawings that expose the difference: the same matte and glossy body on a lit waterline, a raster patch next to an SDF form, silhouettes across the background, a small emitter and a grounded contact. Shade a fixed record on CPU/compatible GPU stages before D lands. Use the frozen contract from std1.md; avoid private copies of D’s interfaces in committed stdlib. When D and B are available, use their commits as test overlays without editing or committing them.
3. Exercise the source forks and swaps in §§1,3. Swap woodblock→toon under VisBuffer+direct with the same post, and direct→ReSTIR with the same woodblock function. Run precisely that same woodblock source under path reference, with increasing samples, and compare against a direct run. Require matching primary Vis/depth in the fixed-schedule swaps and identical ink for identical Surface/Lighting inputs. Keep emission, lobe weights, pressure of hatch/outline and phase order visible to the author.
4. Produce one small runnable scene or explicit variant per style and seven named WebP q95 approval stills; include direct/reference woodblock, the unchanged historical woodblock control, source scene/variant, legacy comparison from the same camera where available and pixel/timing metadata. Check paper AA on a short motion sample and keep the frame sequence for the lead. Report whole shading and post cost separately; mark any unimplemented recipe component or unresolved aesthetic difference. Approval belongs to the lead.

Final message ≤120 words.
