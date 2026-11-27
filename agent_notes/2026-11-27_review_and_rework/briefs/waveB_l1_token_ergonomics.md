# Lane l1-token-ergonomics: Additive Language Ergonomics (`v3@Space`, `match`, `if` in `build`, Parameterized Schedules)

Effort: high
Mode: CODE-ONLY (LOFT_NO_COMPILE=1 — DO NOT run cargo, rustc, tools/dev.sh, or tools/check.sh! You MAY run `python3 tools/check_loft_grammar.py`!)

## Mission
Implement the highest-ROI **strictly additive** token-bloat killers identified in `agent_notes/2026-11-27_review_and_rework/ROLLOUT_TOKEN_BLOAT.md` so both cheap models and Opus stop burning 30–50% of their tokens on workarounds — while keeping 100% of existing rollouts (`out/spark_pairs/`, `corpus/`, `scenes/`, `engines/gates.json`) compiling identically!

## Strict File Ownership (ONLY edit/add files under these paths)
- `docs/spec/loft.lark` (symlinked by `grammar/loft.lark`)
- `crates/loft-parse/src/ast.rs`, `parser.rs`
- `crates/loft-check/src/check.rs`, `infer.rs`
- `crates/loft-eval/src/eval.rs`
- `crates/loft-emit/src/emit.rs`
- `crates/loft-passes/tests/ergonomics.rs` (new test)

## Exact Additive Features to Implement
1. **Explicit `v3(...)@Space` and `p3(...)@Space` constructors (kills `[x,y,z]@World - [0,0,0]@World` and `[a.x,a.y,a.z]@World`!):**
   - Allow `v3(x, y, z)@Space`, `v3(expr)@Space` (converting untyped `v3`/`rgb` → `v3@Space`), `p3(x, y, z)@Space`, `p3(expr)@Space` (converting untyped `v3`/`rgb` → `p3@Space`), and `v3(0)@Space` / `p3(0)@Space` splat.
   - Keep `[x, y, z]@Space` (`p3@Space`) and `p3@Space + untyped [x, y, z]` 100% unchanged for existing rollouts!
2. **`match` expression (kills 52 nested `select(select(...))` chains & WGSL false/true arg-order bugs!):**
   - Syntax: `match expr { pat => expr, ... _ => expr }` (integer literal / `_` patterns, checking arm type consistency via `meet()`, evaluating in `eval.rs` for both concrete `f32` and interval `Iv`, lowering cleanly to WGSL `switch` or nested `select` in `emit.rs`!).
3. **Conditional `if` over `emit` in `build` blocks (kills `build for i in 0..select(0u, 1u, cond)` written 18× in `boss-otter`!):**
   - Ensure `if cond { emit ... } else { ... }` parses, typechecks, and executes cleanly inside `build` / `build fn` blocks.
4. **Fix imported parameterized `schedule` functions (`E505 duplicate cull` bug documented in `boss-walrus/LOG.md:36`):**
   - Trace why calling a `pub schedule` imported from another module currently triggers `E505 duplicate cull`, and fix schedule expansion so scene files can call reusable parameterized schedules from modules without copy-pasting 8-line `schedule Main` blocks!
5. **Keep `docs/spec/loft.lark` synced and verified with `python3 tools/check_loft_grammar.py`.**

## Rules (ZERO-COMPILE MODE)
- **DO NOT run `cargo`, `rustc`, `tools/dev.sh`, or `tools/check.sh`.**
- Run `python3 tools/check_loft_grammar.py` after updating `docs/spec/loft.lark`.
- Read all owned files completely upfront. Commit cleanly on your branch by explicit path.
- Final report (<= 250 words): exact grammar/AST/check/eval/emit changes and test coverage.
