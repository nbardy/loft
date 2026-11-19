You are reviewing the surface syntax of Loft v1.0, a 3D meta engine whose authors are mostly LLMs. You have no prior context. Read ~/git/loft/docs/handoff/2026-11-20_syntax/DUMP.md in full first. It contains: the ask; the bakeoff tasks; seven language cards; the blind verdicts so far; and sample outputs written by Sonnet and Haiku. Then read the background files it lists, in the repo ~/git/loft. Also read every output file in agent_notes/2026-11-20_syntax_bakeoff/; new verdict files may appear there while you work (VERDICT_ALL.md, VERDICT_SEVEN.md).

Answer the ten questions in "THE ASK", and deliver the document it asks for: "Loft v1.0 surface syntax". It has seven parts: decisions, EBNF with 10 unambiguity rules, inference rules, the three tasks written in full with token counts, five diagnostics, grafts, and a parser size prediction.

Count tokens with a real tokenizer: run `uv run --with tiktoken python` using o200k_base. Argue from the measured errors in the outputs. Label every number as measured or predicted.

Write it to ~/git/loft/docs/handoff/2026-11-20_syntax/astra_syntax_v1.md. Edit no other file.

Final message: a 250-word summary of your recommendation.
