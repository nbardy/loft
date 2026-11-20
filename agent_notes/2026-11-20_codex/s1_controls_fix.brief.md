Effort: medium

You are a Loft engineer finishing the chief scientist's control-pass gate prototype. Repo ~/git/loft. Branch loft/s1-controls (aa1087e) built:
- the first two whole-pass hand-written WGSL controls (Hermit Shade, Shrine Vis);
- a naga check;
- a Metal FMA-leak guard;
- ABBA ring-timestamp bootstrap;
- a 2× slowdown mutant.

Its evidence is in out/meta/s1_controls/, and the context is in agent_notes/2026-11-20_v1/scientist_handoff_2.md §2.

1. **Workspace:** a cambium workspace on loft/s1-controls-fix from loft/s1-controls. Merge origin/main first.
2. **Make the hand-written controls match Loft's output exactly** where the outputs should be identical:
   - Shrine Vis: 20 ID diffs and 76,908 depth diffs. Fix them in the hand-written WGSL, not in Loft.
   - Hermit Shade: HDR differs by more than 1 ULP. Explain it or fix it.
3. **Wire the gate into tools/check.sh** as "parity" with a 5% threshold (docs/proposals/2026-11-20_backends_and_escape_hatches.md). It exits 7 when over, and the slowdown mutant must still trip it.
4. **Build and test.** Iterate `tools/dev.sh check` until it compiles; then at most 2 full test or GPU reruns, under tools/gpu_lock.sh. Push the branch; do not merge. Write the lane note agent_notes/2026-11-20_v1/lanes/s1_controls_fix.md.

Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Final message ≤120 words.
