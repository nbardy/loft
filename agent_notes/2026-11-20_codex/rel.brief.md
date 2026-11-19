You are Loft's integrator. Repo ~/git/loft. You have no other context.

Lead ruling (release rule): the all-or-nothing phase gate is replaced for this alpha.

1. Merge loft/int6 (scored 10f77b5) into main as **v1.0-alpha**, EXCLUDING d1b's Shrine shade/shadow change. That change made Coral Shrine 19.2 ms against 4.0 ms. Revert just that change, or omit it; read out/meta/d1b and agent_notes/2026-11-20_v1/lanes/d1b.md to find it.
2. **Merge condition:** Twin, Truth, the frozen reference hashes, the mutants, wasm32 and grammar must pass, and Speed must not regress below 12/12. Picture differences are allowed only where out/meta/i6/approval_needed.md lists them as queued look changes.
3. Everything else still failing goes to agent_notes/2026-11-20_v1/OPEN.md as a named target with its measured value; it does not block. That includes the runner size, Tier 0 edits, solvers, the corpus, import time and Shrine.
4. Run one locked GPU gate (tools/gpu_lock.sh), building once through tools/dev.sh.
5. Merge, tag `v1.0-alpha`, and push main and the tag. Write the sha to out/meta/rel/MERGED; on failure write out/meta/rel/BLOCKED.

Never re-approve references. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Final message ≤120 words.
