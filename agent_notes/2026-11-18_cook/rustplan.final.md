Profiled five scenes through `loft build` and actual `loft serve` compile children. Build medians ranged from **239–6,169 ms wall**, **302–4,108 ms CPU**, and **117–999 MiB peak RSS**. Heron’s solve alone used 3,323 CPU ms. Storm reached the writer and returned exit 4 for its two JavaScript-filled tables.

Kept one TypeScript optimization. Indexing shader constants by token offset cut median Titan host-lowering CPU from **545 to 403 ms (26%)** across six pairs. All four successful artefacts stayed byte-identical across both routes. Whole-process CPU varied under contention.

Keep TypeScript and test warmed, isolated Linux compile jobs against a **500 CPU ms** median target before funding a Rust port. [Proposal](/Users/nicholasbardy/git/_wt/loft-rustplan/docs/proposals/2026-11-18_compiler_speed.md) · [Phase measurements and harness](/Users/nicholasbardy/git/_wt/loft-rustplan/agent_notes/2026-11-18_rustplan/NOTE.md)

Typecheck, nine focused tests, and Owl’s check, build, and still passed. The full suite exited 143 before its tally. The all-examples GPU test timed out at 180 seconds again when retried alone.