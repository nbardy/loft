Effort: xhigh

You are a Loft engineer working in ~/git/loft. Bodies and gait laws. Read AGENTS.md, CORE.md, AGENT_CARD.md, docs/PHILOSOPHY.md §§0.5 and 4, docs/spec/README.md (07 > PATCH_V1_2 > 06 > 00–05), agent_notes/2026-11-20_v1/OPEN.md and agent_notes/2026-11-20_v1/design/std1.md and tok1.md in the same directory. Read legacy files with git show legacy/2026-11-19:<path>.

Rules:

- Use `git cambium add -b loft/std1e -name loft-std1e -ephemeral ~/git/_wt/loft-std1e origin/main`. Record the base SHA. Start only after std1a is merged. Rebase on that scored revision.
- Own only stdlib/motion/, stdlib/engines/{act,roam}.loft if integration requires it, scenes/showcase/std1e/ and your lane note. Write measured before/after values, commit SHAs and evidence paths in agent_notes/2026-11-20_v1/lanes/std1e.md.
- Predict the measurements first. Keep kinds and rows, typed spaces and stages, exhaustive boundary errors and ordinary forkable source. A new public function needs recurring complicated blocks at ≥3× the full call cost. Never add one-line conveniences or renamed map, await, filter or fold.
- Iterate on compile errors with `CARGO_BUILD_JOBS=2 tools/dev.sh check loft-cli`. Run the focused checks, static checks and the workspace suite. Serialize GPU work with tools/gpu_lock.sh. At most two full-suite/GPU reruns after the first scored run. If a failure repeats without its cause narrowing, escalate with the first differing row, pixel or pass.
- Keep frozen RGBA references and approvals intact. Use lossless inputs for hash/diff. Store evidence under the primary checkout’s out/meta/std1e/ before Cambium cleanup. Render a small runnable showcase per style this lane implements or uses. Save actual WebP q95 images in out/meta/std1e/approval/ with scene, camera, time, command, SHA and adapter metadata for lead review.
- Commit by owned path, push loft/std1e, and leave merging and picture approval to the lead. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>".

Task:

1. Use the body, move, roam, pin, present, support, deck, head and plan decisions in §0. Keep reference-only share_range/seams as tagged comparison sources. Write typed body and chain rows with explicit bounds, deck-space pins, neighbor IDs and solved pose ownership. Supply a build error for unreachable rest plans. Keep the existing ordinary IK/exact spring/quintic and Act/Roam functions. Recover the full support exchange law and editable gaze, compare degeneracies (flight, point, segment, area).
2. Compose build, state, run_laws, solver and presentation instead of concatenating WGSL. Use true runtime loops and disjoint writes, no host loop generating a shader per foot. Make both the renderer and the planted-contact check read the same posed endpoint. Fork gait.loft beside the scene as in §3, change the lift, show that only the selected gait changes. Score every proposed export against the actual recurring longhand call sites.
3. For a deterministic stick script, render a crab exchanging support and a moving-deck/turn fixture in their styles. Run CPU/GPU and live/replay at 30, 60, 120 Hz and a six-tick stall. Score zero divergence in supported deterministic mode and report actual planted drift, turn latch, pose bytes, total steps, dispatches and p95 for 1/256/1,000 actors. Legacy comparator: 22 steps and 0.0008 mm after IK, motion p95 0.070/0.101/0.086 ms respectively, in its recorded setup. Distinguish changed plans, measurement variance and regressions.
4. Capture one WebP q95 per presentation style in the small gait showcase with stance/lift/land poses, time and camera. Store trajectory/constraint numbers and lossless controls beside the approval stills. Keep current Crab, Heron and Boss controls pixel/twin equal. Leave flagship scene authoring to the scene-port lane.

Final message ≤120 words.
