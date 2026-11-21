# Step creative_v15_runtime — Wire `~/git/creative` RL Trainer to Rust Loft v1.5 (Runs NOW in Parallel)

Read `~/git/loft/docs/spec/06_ADDENDUM.md` (§17 CLI & Creative Training Contract), `~/git/creative/training/loft/README.md`, `~/git/creative/training/loft/library.py`, `~/git/creative/training/loft/render.py`, `~/git/creative/training/loft/learner.py`, `~/git/creative/training/loft/config.py`, `~/git/creative/training/loft/data.py`, `~/git/creative/training/loft/tasks.py`, `~/git/creative/training/loft/wording.json`, and `~/git/creative/configs/loft/crab-curriculum.jsonc`.

## Goal

Add first-class Rust `.loft` v1.5 execution support (`LoftV15`) to `~/git/creative/training/loft/` **right now** (pure Python, zero Rust compiles, zero waiting on Loft stdlib polish), while keeping 100% of existing `Frozen` and `Vendor` TypeScript tests intact and green.

## Task

Work in an isolated Cambium worktree of `~/git/creative`:
`git -C ~/git/creative cambium add -b loft/v15-runtime -name creative-v15-runtime -ephemeral ~/git/_wt/creative-v15-runtime`
(**Never** modify or reset dirty files in `~/git/creative`'s primary checkout.)

1. **Add `LoftV15` to `training/loft/library.py` (One Clean Path sum-type dispatch):**
   - Extend `Library = Frozen | Vendor | LoftV15`.
   - `LoftV15` dataclass holds `root: Path` (path to `~/git/loft` or configured checkout), `binary: Path` (e.g. `root / "target/release/loft"`), `commit: str`, `stdlib_tree: str`, `kind: ClassVar[Literal['loft_v15']] = 'loft_v15'`.
   - `load_library(...)` accepts `"loft_v15"` or `{"kind": "loft_v15", "root": ..., "binary": ...}` cleanly with typed `ContractError` on missing binary/checkout.
   - Keep `Frozen` and `Vendor` 100% untouched in behavior!

2. **Add Rust `loft` execution path in `training/loft/render.py`:**
   - Dispatch exhaustively on `Library`: `Frozen`, `Vendor`, `LoftV15` (`assert_never`).
   - For `LoftV15`:
     - Write candidate `.loft` source to a clean temp directory (allowing `use stdlib::...` resolution against `library.root`).
     - Run `loft check <scene.loft>` (compile/type/grammar gate) + optional `loft lint` warning telemetry (never failing on lint warnings alone unless configured).
     - Render requested timestamps (default `[0, 0.37, 0.91, 1.53]` or clip frames at `512×512`) via `loft still` / `loft clip` with `--max-build-ms` and `--max-frame-ms` budgets, serialized through `~/git/loft/tools/gpu_lock.sh` if present.
     - Map `docs/spec/06_ADDENDUM.md §17` exit codes cleanly:
       - `0` → success, return `Rendered` with frames, timings, and receipt (`commit`, `stdlib_tree`, `loft_v15`).
       - `1, 2, 3, 4, 5` → `ContractError` (scored 0, clean diagnostic fed back).
       - `7` → `RenderBudget` (subclass of `ContractError`, scored 0).
       - `6` or unexpected signal/device loss → `RuntimeError` (infrastructure error, NEVER scored!).

3. **Support `.loft` fenced blocks & live `loft context` system prompts in `learner.py` / `config.py` / `tasks.py`:**
   - When `library.kind == "loft_v15"`:
     - Extract exactly one fenced ```` ```loft ```` code block (still extracting ```` ```typescript ```` for `Frozen`/`Vendor`).
     - Support loading the system prompt from `loft context` (falling back cleanly to `configs/loft/system-v15.txt` when `loft context` is not yet built on an older binary) + appending the `.loft` meta-engine creative submission contract:
       - Explicitly removes the legacy TS `"no custom shader code"` ban!
       - Explicitly instructs and motivates the model to **rewrite/fork engines, `schedule`s, `pass`es, `style`s, `MaterialTable` uber-families, `kernel allow(...)` GPU physics, and `stdlib/motion/{gait,springs,mood}` laws** — not just tweak numeric parameters — especially on `Improve` (`shape`, `model`, `renderer`, `animation_principles`, `manner`) tasks!
   - Add `configs/loft/system-v15.txt` and `configs/loft/crab-v15-overfit.jsonc` (mirroring `crab-curriculum.jsonc` with `"library": "loft_v15"`, `"seeds": "training/loft_envs/seed_programs/crab-overfit-v15"`, `"tasks": "configs/loft/crab-v15-overfit.tasks.jsonl"`, plus `improve_motion` / `animation_principles` / `manner` axes wired cleanly!).

4. **Tests & Verification (Write ALL code first, test once!):**
   - Write `training/loft/test_v15_runtime.py` covering:
     - `LoftV15` loading & manifest,
     - ```` ```loft ```` code-fence extraction vs ```` ```typescript ````,
     - Exit code `0..7` mapping (`1..5 -> ContractError`, `7 -> RenderBudget`, `6 -> RuntimeError`),
     - Live smoke render of one real `.loft` scene (e.g. `~/git/loft/scenes/crab.loft` or a minimal inline `.loft` scene) using `~/git/loft/target/release/loft` if available (skip gracefully only if binary absent, otherwise assert valid PNG/frames!).
   - Run `uv run pytest training/loft/test_*.py` ONCE after all code is written — ALL existing + new tests must pass!
   - Commit by path on `loft/v15-runtime` in `~/git/_wt/creative-v15-runtime` (and leave the worktree/branch clean for `iv15_3b` to merge).
   - Write a concise lane note to `~/git/loft/agent_notes/2026-11-20_v1/lanes/creative_v15_runtime.md` (≤ 30 lines).

## Rules (Superintelligence Discipline)
- Read all relevant Python files upfront, write ALL edits + tests in one decisive sweep, then run `uv run pytest` once to verify.
- Do NOT run Cargo or build Rust — this is a pure Python lane.

Final message ≤150 words.
