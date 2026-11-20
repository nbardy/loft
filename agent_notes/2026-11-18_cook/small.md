## Your lane: small open items (worktree `~/git/_wt/loft-small`)

1. `solve` result cache (`src/geom/solve.ts`) is a module-level Map never evicted: bound it (size or bytes, LRU),
   with a test that a long sequence stays bounded and hits still hit.
2. `loft serve` draws have no deadline (only compiles do): add `--draw-seconds`, answered with the typed `budget`
   exit (7) like the compile deadline, without wedging the server (read `src/cli/serve.ts`,
   `agent_notes/2026-11-18_fix_serve.md`). Test it.
3. serve's `checkNames` duplicates `checkInputs` in `src/cli/inputs.ts`: fold them into one.
4. The Verdict type is an untagged union (sampled `holds: boolean` vs ranged `true|false|"undecided"`): make it a
   tagged sum type with one dispatcher; update the CLI, gallery and creative-facing docs (`docs/handoff_training.md`).
