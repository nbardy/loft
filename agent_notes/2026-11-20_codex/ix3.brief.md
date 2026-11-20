Effort: high

You are Loft's integrator for wave x3. Repo ~/git/loft. Read agent_notes/2026-11-20_v1/scientist_handoff_2.md and agent_notes/2026-11-20_v1/OPEN.md.

Candidates:
- **loft/s1-escapes (from s1_escapes):**
  - real bake_tex sampling;
  - general `kernel allow(...)`;
  - whole-pass `pass ... reads/writes`;
  - `unproved(step_scale)`.
- **loft/s1-controls-fix:** the whole-pass parity gate.
- **loft/s1-lip (871ea94):** the Lipschitz fix for rotated IFS folds. FoldedLantern L goes from 22.87 to 1.0; Shrine Vis is 0.94 ms p50 faster. It CHANGES Shrine pixels: the old picture was a truncated march. Merge it ONLY if the owner has approved, recorded as an "APPROVED" line in out/meta/s1_lip/OWNER_APPROVAL. If that file is absent, keep the branch, add out/meta/s1_lip/shrine-review.webp to the owner approval queue (out/meta/ix3/approval_needed.md), and do not merge.

Audit each candidate. Merge the passing ones onto origin/main in loft/intx3, build once, and run ONE locked GPU gate (the five gates, mutants, wasm32, grammar, and the new parity gate if merged). Merge rule: Twin, Truth, references, mutants, wasm32 and grammar green; Speed not regressed; pixels bit-identical except approved look changes.

Merge into main and push. Update OPEN.md. Write out/meta/ix3/MERGED and out/meta/ix3/BLOCKED. Never re-approve references. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Final message ≤150 words.
