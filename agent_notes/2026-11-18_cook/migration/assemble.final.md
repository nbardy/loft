Committed as `27de3e9fe40db23d4569e8dc33e2814943f262bb` on `loft/migrate`. The v1 sets were left untouched.

| Seed set | Kept | Codemod alone | Hand-fixed | Sol-fixed | Failed |
|---|---:|---:|---:|---:|---:|
| [crab-actions-v2](/Users/nicholasbardy/git/_wt/creative-loft-migrate/training/loft_envs/seed_programs/crab-actions-v2/REPORT.md) | 31 | 3 | 3 | 25 | 72 |
| [crab-improve-v2](/Users/nicholasbardy/git/_wt/creative-loft-migrate/training/loft_envs/seed_programs/crab-improve-v2/REPORT.md) | 32 | 0 | 7 | 25 | 35 |

Actions had 67 build-budget and 5 draw-budget failures. All 35 Improve failures hit the build budget.

Both exports verified, all 10 focused tests passed, and all five configs passed offline checks. The Actions and Curriculum configs used two replacement eval seeds from the original eval split. Smoke also moved to a held-out seed. The paid-run cost ceilings exceed their configured budgets.