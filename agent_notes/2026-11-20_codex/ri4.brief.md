You are Loft's integrator, re-scoring a fix wave. Repo ~/git/loft. You have no other context. Read docs/spec/ (precedence 07 > PATCH_V1_2 > 06 > 00–05), your earlier review agent_notes/2026-11-20_v1/reviews/i4.md, and the fix note agent_notes/2026-11-20_v1/lanes/i4-fix.md.

Branch to score: loft/i4-scored-fix.

1. **Re-audit** the fixes against your MUST-FIX list. Append the outcomes to the review file.
2. **Merge main** into the branch.
3. **Run ONE full GPU gate** for the phase, holding the lock: tools/gpu_lock.sh. Include the mutants, wasm32 and the grammar checks. Write out/meta/i4-r2/gate.json and out/meta/i4-r2/sheet.webp.
4. **Pictures.** A Pictures failure that the fix note lists as OWNER APPROVAL NEEDED (an intended look change) does NOT block the merge of everything else: merge it with the old references still failing for those scenes. Record them in out/meta/i4-r2/approval_needed.md. NEVER re-approve references yourself.
5. **Merge.** If every other gate passes, merge into main, push, and write the merged sha to out/meta/i4-r2/MERGED (that file must exist ONLY after a successful push). If not, write exactly what failed to out/meta/i4-r2/BLOCKED.

Builds: compile once through tools/dev.sh. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>".

Final message ≤150 words: gates, merged sha or blockers, and the approvals needed.
