# Step v15_context — AST-Driven `loft context`, Real `loft fork`, and Meta-Engine `AGENT_CARD.md`

Read `AGENTS.md`, `docs/spec/06_ADDENDUM.md` (§17), `crates/loft-cli/src/main.rs`, `crates/loft-cli/src/fork.rs`, `crates/loft-parse/src/ast.rs`, and `stdlib/` (all subdirectories). Also inspect the legacy TS `context` pattern via `git show legacy/2026-11-19:src/cli/context.ts` for inspiration on lean word budgets — but design natively for the new `.loft` meta-engine.

## Goal

Give any model or trainer a **single, ultra-lean, zero-drift** command (`loft context`) that teaches Loft's syntax AND actively breaks the Unity/Three.js "black-box engine" prior so agents rewrite engines, schedules, integrators, styles, and motion laws instead of only tweaking parameters.

## Task

Work in `~/git/_wt/loft-v15-context` on branch `loft/v15-context` from `origin/main` (`git cambium add -b loft/v15-context -name v15-context -ephemeral ~/git/_wt/loft-v15-context origin/main`).

1. **Implement AST-driven `loft context` (`crates/loft-cli/src/context.rs`, wired into `main.rs`):**
   - Usage: `loft context [--section core|engines|motion|styles|passes|all]` (default prints the complete training context, no GPU required, deterministic for a commit, < 200 ms).
   - **Splices two parts cleanly (zero drift):**
     1. Written preamble from `docs/context.md` (embedded via `include_str!` or loaded relative to workspace root, falling back cleanly to embedded):
        - **Meta-engine mandate upfront:** `stdlib/engines/`, `stdlib/schedules/`, `stdlib/integrators/`, `stdlib/styles/`, `stdlib/motion/` are editable `.loft` source, NOT a fixed black box. Teach the 4 escalation levels:
          1. Scene / `kind` / `build for` / `MaterialTable` rows
          2. Custom `style` (`fn shade_style(hit, light)`) or `fn material(hit) -> Bsdf`
          3. **Fork or rewrite** an engine, `schedule`, `integrator`, or `motion` solver (`loft fork`, custom passes, switching forward ↔ vis-buffer ↔ ReSTIR ↔ path tracer, custom gait/spring/mood laws)
          4. **Author a new engine** (`pass` graph, `kernel allow(atomics, workgroup, subgroups)`, `World` hardware ray queries, raw WGSL/MSL escape hatches) whenever stock engines miss the look or frame budget
        - **One worked ~25–35 line engine-rewrite example** (defining a custom `schedule` / `pass` + `law` + `kind` + `build`, NOT just `use stdlib::engines::realtime`) that actually compiles under `loft check`!
        - **Closed verify & inspect loop:** `loft check`, `loft lint`, `loft show`, `loft still`, `loft clip`, `loft profile`, `loft fork`, plus exit codes `0..7` (`docs/spec/06_ADDENDUM.md §17`).
        - Top 4 silent mistakes that compile but draw wrong pictures.
     2. **Source-generated compact signature catalog via `loft-parse` AST inspection** over `stdlib/**/*.loft`:
        - Parse each `stdlib/` module with `loft-parse` and emit **compact, token-lean** `pub` declarations (`fn`, `kind`, `struct`, `schedule`, `law`, `kernel`, `solver`, `bake`, `check`, `space`, `world`, `const`), grouped by module path (`stdlib::engines`, `stdlib::schedules`, `stdlib::styles`, `stdlib::materials`, `stdlib::motion`, `stdlib::sdf`, `stdlib::geometry`, `stdlib::noise`, `stdlib::lighting`, `stdlib::integrators`, `stdlib::sky`, `stdlib::post`).
        - Keep token count **as lean as possible**: emit pure AST signatures by default; include at most one short (≤ 12-word) leading `#` doc comment ONLY when a module header or `pub` item carries a non-obvious contract/invariant (units, `LightSummary` requirement, `Det` vs `NonDet`). Self-evident signatures get **zero** prose tokens.
        - Also list core language builtins/types directly from the compiler's own tables so they can never drift.
   - **Strict budget:** default `loft context` output MUST be **≤ 2,400 words** (~3,200 tokens).

2. **Implement real `loft fork <std_or_engine_path> <dest.loft>` (`crates/loft-cli/src/fork.rs`):**
   - Resolves `<std_or_engine_path>` (accepting either `stdlib/engines/realtime.loft`, `engines/realtime`, or `stdlib::engines::realtime`), copies the source file to `<dest.loft>` (refusing to clobber unless `--force`), preserves/validates module imports so `<dest.loft>` compiles standalone in its destination directory, and prints the exact `use` statement to import the local fork.
   - Add a unit/integration test proving `loft fork engines/realtime <tmp>/my_engine.loft` produces a valid editable engine that compiles with `loft check`.

3. **Align `AGENT_CARD.md` & add static checks (`tools/check.sh`):**
   - Update `AGENT_CARD.md` to point to `loft context` and reflect the meta-engine workflow (`loft fork`, `loft lint`, `loft profile`, custom schedules/engines).
   - Add a check (in `tools/check.sh static` or a focused unit test in `loft-cli`) verifying:
     - `loft context` succeeds with no GPU and stays **≤ 2,400 words**,
     - Every fenced ```` ```loft ```` snippet inside `docs/context.md` passes `loft check`!

## Rules (Superintelligence Compile Discipline)
- **Write ALL code first across `docs/context.md`, `crates/loft-cli/src/{context,fork,main}.rs`, `AGENT_CARD.md`, and tests BEFORE your first compile.** Aim for **≤ 3 compiles total** via `CARGO_BUILD_JOBS=2 tools/dev.sh check loft-cli` → focused test → one final check.
- Do NOT touch `stdlib/motion/` (owned by `v15_motion_mood`) or `~/git/creative` (owned by `v15_seeds_creative`).
- Write `agent_notes/2026-11-20_v1/lanes/v15_context.md` (≤ 30 lines) with exact `loft context | wc -w`, snippet-check result, and `loft fork` test result. Commit by path and push `origin loft/v15-context`.

Final message ≤150 words.
