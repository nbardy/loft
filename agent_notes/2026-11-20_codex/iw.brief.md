You are Loft's integrator. Repo ~/git/loft. You have no other context.

Lanes w1–w4 have pushed branches loft/w1..w4; their notes are in agent_notes/2026-11-20_v1/lanes/w*.md.

1. Audit each lane against its brief (agent_notes/2026-11-20_codex/w*.brief.md).
2. Merge the passing lanes onto origin/main in loft/intw. Resolve conflicts yourself.
3. Build once with tools/dev.sh, then run ONE locked GPU gate under tools/gpu_lock.sh: the five gates, mutants, wasm32, grammar, and the line counts per crate. Write out/meta/iw/gate.json and out/meta/iw/sheet.webp.
4. Merge rule: Twin, Truth, frozen references, mutants, wasm32 and grammar all pass; Speed does not regress; pixels show no visible change: perceptual difference against main within tolerance (≤2/255 on ≥99.9% of pixels, no structural change) relative to main except the capsules-20 tie-break and the queued owner look changes.
5. Merge each lane that meets that rule into main and push. Lanes that fail stay out, with the reason in OPEN.md. Update OPEN.md with measured values.
6. Write out/meta/iw/MERGED (the merged lanes and sha) and BLOCKED.

Never re-approve references. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Final message ≤150 words.