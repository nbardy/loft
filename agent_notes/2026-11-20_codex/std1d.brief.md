Effort: xhigh

You are a Loft engineer working in ~/git/loft. Light, transport, schedules and post. Read AGENTS.md, CORE.md, AGENT_CARD.md, docs/PHILOSOPHY.md §§0.5 and 4, docs/spec/README.md (07 > PATCH_V1_2 > 06 > 00–05), agent_notes/2026-11-20_v1/OPEN.md and agent_notes/2026-11-20_v1/design/std1.md and tok1.md in the same directory. Read legacy files with git show legacy/2026-11-19:<path>.

Rules:

- Use `git cambium add -b loft/std1d -name loft-std1d -ephemeral ~/git/_wt/loft-std1d origin/main`. Record the base SHA. Start only after std1a is merged. Rebase on that scored revision.
- Own only stdlib/{materials,lighting,sky,integrators,schedules,post}/, the render compositions under stdlib/engines/ (excluding act.loft and roam.loft), renderer/compiler/grammar changes required for the stage-typed connections, scenes/showcase/std1d/, stdlib/README.md completion and your lane note. Write measured before/after values, commit SHAs and evidence paths in agent_notes/2026-11-20_v1/lanes/std1d.md.
- Predict the measurements first. Keep kinds and rows, typed spaces and stages, exhaustive boundary errors and ordinary forkable source. A new public function needs recurring complicated blocks at ≥3× the full call cost. Never add one-line conveniences or renamed map, await, filter or fold.
- Iterate on compile errors with `CARGO_BUILD_JOBS=2 tools/dev.sh check loft-cli`. Run the focused checks, static checks and the workspace suite. Serialize GPU work with tools/gpu_lock.sh. At most two full-suite/GPU reruns after the first scored run. If a failure repeats without its cause narrowing, escalate with the first differing row, pixel or pass.
- Keep frozen RGBA references and approvals intact. Use lossless inputs for hash/diff. Store evidence under the primary checkout’s out/meta/std1d/ before Cambium cleanup. Render a small runnable showcase per style this lane implements or uses. Save actual WebP q95 images in out/meta/std1d/approval/ with scene, camera, time, command, SHA and adapter metadata for lead review.
- Commit by owned path, push loft/std1d, and leave merging and picture approval to the lead. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>".

Task:

1. Adopt §1’s Surface/Lighting value contract, typed estimator and shade callbacks and ordinary statically bound schedule fragments. Implement distinct vis-buffer + tile shade, forward draw and path-primary schedules, each producing HDR/depth/primary IDs. Bind each imported callback lexically. Keep the current six-argument shade path and return ordering available for exact existing parity. Engine composition must go through the ordinary loader and runner. No builtin list of style names, new style-specific host path or runtime function pointer.
2. Port direct, ReSTIR, reference and four-hero spectral transport, reusing the existing reservoir/medium machinery. Verify PDF/weight/visibility, no double-counted emission, shadow endpoint at/inside spherical emitters, proper disocclusion/rejection if reusing history and spectral stack transitions. Keep camera jitter, sample dimensions and reference throughput editable. Track primary linear lighting by contribution; average common-hit estimates before quantizing ink. Compare resolved estimates to old unclipped reference radiance in a white furnace and to a direct light control. Record sampling error as spp increases, e.g. 16/64/256. Reuse accepted baseline thresholds where specified.
3. Keep the physical lobe and energy source/table single-owned, quad-aware print lighting, typed emitters, sky projection/bakes, print washes, four-tap gloss, grain, edge, bloom, reflection, studio AO and optional DoF. Validate stage-only derivatives and bound all gathers. Benchmark original costs from the audit with matching resolution/conditions when feasible; keep expensive optics explicitly selectable. Exclude the legacy 64-step volume default and f16-by-default. Preserve authored ordering and source scopes in warm and cold compilation.
4. Ship the woodblock_realtime example, plus ordinary selectable reference and forward examples. Demonstrate the two one-import swaps and same woodblock source on two schedules. Use test overlays of C/B as needed; leave their source trees untouched. Verify branch and combined versions of the README commands. A Rust-only hidden style or a duplicate invocation of shade after spectral display conversion fails the exercise.
5. Render one small showcase per style exercised in this lane, including PBR material/furnace, print sun/sky and a luminous post chain. Record 1 and 20 instances, baseline p95 and commands, every intermediate attachment/traffic cost and cold/warm reuse; give the lead a named q95 still per style and exact paired control hashes. Retain independently runnable CPU numeric controls for energy and light samples.

Final message ≤120 words.

Speed rules (lead, 20 Nov): keep the ceremony small.
- Lane note: 30 lines or fewer.
- No contact sheets unless your task changes a look.
- ONE paired timing run, if any; the integrator does the final timing.
- Do not re-read the whole spec. Read only the sections your task names.
- Iterate cargo check until it compiles, then run your focused tests only. The integrator runs the full suite.

Std rule: a lib function must replace a recurring, sufficiently complicated block of ≥3× its call's tokens. NEVER add renamed wrappers for standard patterns (map, await, filter, fold) or one-or-two-call helpers. Code lives in the stdlib/ tree (design/std1.md); forking is copy-paste.
Base: start from origin/main AFTER the std1a restructure (stdlib/ tree), not the old flat lib/.
