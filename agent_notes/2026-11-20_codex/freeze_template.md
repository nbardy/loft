You are Loft's integrator: step 0 of a parallel wave. Repo ~/git/loft. You have no other context. Read docs/PHILOSOPHY.md, CORE.md, docs/spec/ (06 overrides, README rulings) and agent_notes/2026-11-20_v1/fanout_handoff.md.

PHASE: __PHASE__. Freeze, in one serial commit on main, everything the parallel lanes share: __FREEZE__.

Use exact types, offsets and assertions, with stub tests that the lanes will fill in. Keep it small: only what two lanes would otherwise define differently.

Work in a cambium workspace: `git cambium add -b __BRANCH__ -name __WS__ -ephemeral ~/git/_wt/__WS__`. Use `CARGO_BUILD_JOBS=2 tools/dev.sh`. Run the focused tests, then merge into main and push. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>".

Final message ≤100 words: what is frozen, where, and the merged sha.
