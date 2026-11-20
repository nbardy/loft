# Step v15_bloat — R5 Binary import_table, Seed-Regenerated Corpus, ad.rs Fix & loft lint

Read AGENTS.md, docs/V1_5_GOALS.md (R5, G6.1, G6.2), and `agent_notes/2026-11-20_v1/lanes/tok1.md` (which showed 95.7% of corpus tokens are baked float arrays).

## Task

Fix corpus bloat as a **data-serialization bug** (R5), NOT by adding hard compile errors:

1. **Binary `import_table` for sampled data:**
   - Add `import_table("<path>.bin")` (or typed binary table loader with clean header/schema check: magic, row count, column layout matching the declared `Table` schema) so baked float/sample arrays live in compact binary assets instead of inline ASCII `.loft` literals.
2. **Fix `ad.rs` row-constant parameters (−112k tokens):**
   - Inspect `crates/*/src/*ad*.rs` where row-constant AD parameters were being expanded/emitted per-row, and hoist them cleanly so row-constant parameters stay scalar/shared.
3. **Regenerate the corpus from procedural seeds + modal row defaults:**
   - Regenerate corpus `.loft` files from their procedural seeds using `build for` loops and modal column defaults (plus `import_table` where sampled assets are genuinely external).
   - Verify **every regenerated scene renders cleanly** under the `v15c` two-tier picture gate and passes grammar/compile checks.
   - **Target:** corpus median token count drops from `96,204` to **≤ 10,000 tokens** (stretch: 1,500–4,000 tokens per R5).
4. **Add `loft lint` (warnings, NOT compile errors):**
   - Implement `loft lint` checking token-count caps and large repeated literal blocks, wired into corpus CI / `./tools/check.sh` as a lint check — **never** failing normal `loft` compilation.

## Rules
- Use `git cambium add -b loft/v15-bloat -name v15-bloat -ephemeral ~/git/_wt/loft-v15-bloat`.
- Do NOT touch `crates/loft-runner` GPU ray-query/sky/MSAA shading code.
- Write `agent_notes/2026-11-20_v1/lanes/v15_bloat.md` with before/after corpus median & p95 token counts, `ad.rs` savings, and gate verification. Push `origin loft/v15-bloat`.

Final message ≤150 words.
