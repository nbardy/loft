Effort: high

You are Loft's language and library lead, running a token-budget study. Repo ~/git/loft (main).

**The rule** (docs/PHILOSOPHY.md §4, as clarified by the owner on 20 Nov): the core stays small. A **standard library** function is justified when a sufficiently complicated block of code recurs and costs **≥3× the tokens** of calling the reusable function. Std functions are ordinary forkable `.loft` code.

1. **Measure where the tokens go** (o200k tokens, via `uv run --with tiktoken`) across engines/, scenes/, lib/ and a sample of ≥40 corpus scenes (corpus/). Attribute tokens to constructs: setup and schedules, lights and sky, materials and palettes, body parts (legs, eyes, shells, limbs), symmetry and mirroring, motion laws and solvers, units and annotations, and so on.
2. **Find recurring blocks** across scenes, with near-duplicates normalised. For each block, give: occurrences, tokens per occurrence, tokens of a call to a proposed std function, and the ratio. Keep only candidates with ratio ≥3 and ≥3 occurrences, ranked by total tokens saved.
3. **Also list surface savings** that are not library functions (inference of rows or units, defaults, shorter forms), each with its estimated saving. Mark any that would grow the core, and justify them.
4. **Predict** the new token counts for capsules (621), hermit (3,021) and the corpus median.
5. **Write agent_notes/2026-11-20_v1/design/tok1.md** with the tables, the proposed std signatures (typed E+), and the forkability check: each function is plain `.loft` in lib/, with no compiler support.
6. **Then write at most 3 Sol lane briefs** that add the top std functions and rewrite engines/scenes to use them, keeping pixels bit-identical, at agent_notes/2026-11-20_codex/tok1a..c.brief.md. Each brief starts with an `Effort:` line, follows the shape of w1.brief.md, and has disjoint files. List them in agent_notes/2026-11-20_codex/tok1.lanes.

Commit by path to main and push. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Final message ≤150 words.
