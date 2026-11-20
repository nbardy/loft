Effort: xhigh

You are a Loft engineer working in ~/git/loft. Geometry, noise and frames. Read AGENTS.md, CORE.md, AGENT_CARD.md, docs/PHILOSOPHY.md §§0.5 and 4, docs/spec/README.md (07 > PATCH_V1_2 > 06 > 00–05), agent_notes/2026-11-20_v1/OPEN.md and agent_notes/2026-11-20_v1/design/std1.md and tok1.md in the same directory. Read legacy files with git show legacy/2026-11-19:<path>.

Rules:

- Use `git cambium add -b loft/std1b -name loft-std1b -ephemeral ~/git/_wt/loft-std1b origin/main`. Record the base SHA. Start only after std1a is merged. Rebase on that scored revision.
- Own only stdlib/{math,noise,sdf,geometry,frames}/, scenes/showcase/std1b/ and your lane note. Write measured before/after values, commit SHAs and evidence paths in agent_notes/2026-11-20_v1/lanes/std1b.md.
- Predict the measurements first. Keep kinds and rows, typed spaces and stages, exhaustive boundary errors and ordinary forkable source. A new public function needs recurring complicated blocks at ≥3× the full call cost. Never add one-line conveniences or renamed map, await, filter or fold.
- Iterate on compile errors with `CARGO_BUILD_JOBS=2 tools/dev.sh check loft-cli`. Run the focused checks, static checks and the workspace suite. Serialize GPU work with tools/gpu_lock.sh. At most two full-suite/GPU reruns after the first scored run. If a failure repeats without its cause narrowing, escalate with the first differing row, pixel or pass.
- Keep frozen RGBA references and approvals intact. Use lossless inputs for hash/diff. Store evidence under the primary checkout’s out/meta/std1b/ before Cambium cleanup. Render a small runnable showcase per style this lane implements or uses. Save actual WebP q95 images in out/meta/std1b/approval/ with scene, camera, time, command, SHA and adapter metadata for lead review.
- Commit by owned path, push loft/std1b, and leave merging and picture approval to the lead. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>".

Task:

1. From §0 port the positive decisions for noise/value and cellular, SDF solid and marks, analytic hits, frame transport, sweep sections, Raup shell frames, tapered growth clusters, eye/stalk/lid rows and layout. Keep append-only row construction at stage 0. Existing hash3/vnoise/fbm2 and ray-capsule bits are controls. The legacy TS fBM octave loop built code at each pass through the host loop. Use runtime GPU loops or sampled tables, and publish one shader per populated kind.
2. Record qualifying block/call sites before promoting exports. Do not add aliases for min, shape transforms, map or a host array combinator. Keep paint, post, schedule and motion policy out of these directories. Make every space and cap/degenerate-case contract inspectable. Keep the differentiated exponent/pow in the surface when required by E405. Coordinate sample_key/cosine_sample with D; its transport must call the same counter convention.
3. Predict 4/8 lattice corners per planar/volume octave, bounded cellular 9/27, constant cone root work and O(stations) scanned frames. Run CPU/GPU field and normal comparisons at seam/negative cells, cone cap/side/tangent and zero length; verify finite results, oriented frames and no cross-space acceptance. At 1 and 20 bodies measure row counts/bytes, generated modules/commands and frame p95. Two scene copies differing only in seed/pose may change rows but must share code.
4. Build small showcase scenes for noisy ink, folded facets, tapered growth and a shell/eye sweep, rendering one labelled treatment per style used. Provide a fixed-camera raster/SDF pair and exact base controls; record resolution, adapter, time, scene SHA and provenance alongside each q95 approval still. Preserve direct source links from the lane note. Exercise copying a foundation file and selecting the copy in a consumer.

Final message ≤120 words.
