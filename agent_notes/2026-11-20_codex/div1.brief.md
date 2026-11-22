Effort: xhigh

You are the lead on training-seed diversity for Loft v1.5 and Creative. Design first, then implement. Repos: ~/git/loft (main) and ~/git/creative (the integration branch `loft/v15-ready`; work in your own worktree from it, e.g. `git -C ~/git/creative worktree add ~/git/_wt/creative-div1 -b loft/seed-diversity loft/v15-ready`). Never touch other sessions' uncommitted files. Never start a paid training run.

**The problem (owner, 23 Nov).** Look at ~/git/loft/out/meta/gallery/seeds_sheet.png and https://claude.ai/artifact/CWcho4LPi4q8NPf8v1rcvr (renders in ~/git/loft/out/meta/iv_final/{crab-smoke,crab-retry,basin}). Every crab seed is the SAME crab: same body, pose, camera, framing and ground. Only palette and floor tint change.
- The 16 authored crab programs were generated one-shot and independently, from one base, with no view of each other and no axis to differ along.
- The 28 ported seeds come from the old crab-actions corpus (actions on one crab), so frame 0 is identical.
- The 12 meta seeds differ in subject, but are single simple objects on similar stages.
- Nothing gates variety.

**1. Design (theory first): write ~/git/loft/agent_notes/2026-11-20_v1/design/div1.md.**
- **Diversity axes for crab seeds:**
  - body plan: carapace shape, leg count and length, claw asymmetry (fiddler), hermit shells;
  - material and style engine: the stdlib/styles that exist;
  - pose or action;
  - camera and framing;
  - environment;
  - lighting.

  For general meta seeds: subject class, composition (multi-object scenes), engine and style, motion.
- **A coverage plan** that assigns each seed a cell so every axis value is covered (pairwise coverage where feasible).
- **A variety gate, defined to the number:**
  - (a) image-embedding nearest-neighbour distance between renders (pick a model runnable locally on M4, e.g. DINOv2 or CLIP via uv; justify the choice);
  - (b) silhouette or depth-mask IoU against every accepted seed, so a palette swap of the same shape FAILS;
  - (c) a palette-invariant check (grayscale or edges).

  Predict what the current 16 authored and 28 ported seeds score, then measure them; they should fail. Set thresholds from that.
- **Generation protocol:** batched and sequential. Each generator call sees a contact sheet plus one-line descriptors of all accepted seeds and its assigned cell, and must differ structurally. Prefer one parameterised crab body generator in stdlib (the ≥3× std rule) whose parameters span the space.

**2. Implement the gate as a tool** in Creative's training/loft (or loft tools/, whichever owns seed admission), with a test that a recolour of an existing seed is rejected.

**3. Regenerate the 16 authored crab seeds** with the protocol, in batches of 4. Each must pass check, lint, both grammars, the CPU build ≤ 2 s and GPU ≤ 50 ms budgets, AND the variety gate. Render each (512×512 beauty plus depth) and make a WebP q95 contact sheet at ~/git/loft/out/meta/gallery/div1_sheet.webp, showing old versus new. Also propose, but do not implement, how to make the 28 ported seeds useful: frame them at varied action frames or cameras, or retire them from stills.

Builds and tests go through ~/.local/bin/agent-build, one at a time. Commit on loft/seed-diversity in Creative and push that branch. Do not merge into the training branch: the lead reviews the sheet first. Messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Final message ≤150 words: the gate thresholds, current versus new scores, and the sheet path.
