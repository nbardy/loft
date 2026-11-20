Effort: high

You are Loft's integrator. Repo ~/git/loft. You have no other context.

Lanes x2a–x2e have pushed branches loft/x2a..x2e; their notes are in agent_notes/2026-11-20_v1/lanes/x2*.md.

1. Audit each lane against its brief (agent_notes/2026-11-20_codex/x2*.brief.md).
2. Merge the passing lanes onto origin/main in loft/intx2. Resolve conflicts yourself.
3. Build once with tools/dev.sh, then run ONE locked GPU gate under tools/gpu_lock.sh: the five gates, mutants, wasm32, grammar, and the line counts per crate. Write out/meta/ix2/gate.json and out/meta/ix2/sheet.webp.
4. Merge rule: Twin, Truth, frozen references, mutants, wasm32 and grammar all pass; Speed does not regress; pixels are bit-identical to main except the capsules-20 tie-break and the queued owner look changes.
5. Merge each lane that meets that rule into main and push. Lanes that fail stay out, with the reason in OPEN.md. Update OPEN.md with measured values.
6. Write out/meta/ix2/MERGED (the merged lanes and sha) and BLOCKED.

Never re-approve references. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Final message ≤150 words.Note: x2c may carry an owner-approval look change; merge it only if pixels are unchanged, otherwise leave it on its branch listed in approval_needed. x2d and x2e add tools/web demos with no product-code risk; merge them if their own checks pass.
