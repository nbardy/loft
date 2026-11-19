You are studying a sibling repo so that Loft can embed into it. READ-ONLY in ~/git/room-runner-arena: do not edit, commit or build there. Write only in ~/git/loft.

Study room-runner-arena's developer experience:
- worktree sharing and forking;
- shared build targets and caches across worktrees;
- its cambium integration;
- its compile tooling, scripts and CI gates;
- how it renders today, and where a renderer would attach.

Then read ~/git/loft/docs/spec/07_PLATFORM_POWER_EXTENSIONS.md (zero-copy host interop: Runner::attach, bind_external_*, export_*) and the Keel embedding notes in docs/spec.

Write ~/git/loft/agent_notes/2026-11-20_v1/room_runner/STUDY.md with:
1. Devex practices Loft lacks, each with the concrete command or config to adopt and its expected saving.
2. The integration plan for running room-runner-arena on Loft: the attach point, which of its renders Loft replaces first, the data mapping (its entities to Loft kinds and rows), the build and dependency wiring, and a first-light acceptance test.
3. A self-contained Sol lane brief at ~/git/loft/agent_notes/2026-11-20_codex/rr1.brief.md that implements the first-light integration on a branch in room-runner-arena via a cambium workspace, without touching its main.

Commit both files to loft main by path and push. Messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Final message ≤150 words.
