**All 170 Creative seeds compiled cleanly with `LOFT_NO_GPU=1`.** The batch changed all 170 programs. Zero needed a hand fix, and zero reported an unported construct.

| Seed pack | Compiled | Hand fixes |
| --- | ---: | ---: |
| `crab-actions-v1` | 103/103 | 0 |
| `crab-improve-v1` | 67/67 | 0 |

The [migration guide](/Users/nicholasbardy/git/_wt/loft-migration/docs/MIGRATION.md) compares both generated API maps and identifies the frozen source as `a3c7612`. The [codemod](/Users/nicholasbardy/git/_wt/loft-migration/tools/migrate.ts) supports single files and JSONL, reports typed reasons at original source lines, and carries Creative’s original camera, sea, sun, and style settings into native `scene()` exports.

Typecheck and all 15 [focused tests](/Users/nicholasbardy/git/_wt/loft-migration/tests/migrate.test.ts) passed. The final batch matched the compiled batch byte for byte. The [per-seed results](/Users/nicholasbardy/git/_wt/loft-migration/out/migration/compiled.jsonl) record scene loading, compilation, and bundle emission. GPU shader validation and pixel comparison were outside this run. Creative’s seed files were read in place, and all changes are ready for the reviewer to commit.