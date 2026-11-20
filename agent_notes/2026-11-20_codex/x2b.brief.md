You are a Loft engineer. Repo ~/git/loft, main at or after 315cd04 (v1.0-alpha plus the runner cut). You have no other context.

Read docs/spec/README.md (precedence 07 > PATCH_V1_2 > 06 > 00–05), docs/PHILOSOPHY.md, CORE.md, agent_notes/2026-11-20_v1/OPEN.md and agent_notes/2026-11-20_v1/design/d1.md.

Rules:
- Use a cambium workspace: `git cambium add -b loft/x2b -name loft-x2b -ephemeral ~/git/_wt/loft-x2b origin/main`.
- Builds: write the code first. Then iterate on compile errors with `CARGO_BUILD_JOBS=2 tools/dev.sh check` (cargo check is cheap) until it compiles cleanly; never ESCALATE on a compile error. The 'at most 2 reruns' limit applies only to full test and GPU runs.
- Kinds of thing are sum types with one exhaustive dispatcher, with no kind or null checks inside handlers. No silent fallbacks.
- NEVER change or re-approve reference pictures; pixels must stay bit-identical unless your task says otherwise.
- Do not merge into main. Push your branch.
- If a fix fails twice without its cause narrowing, ESCALATE with evidence.
- Write agent_notes/2026-11-20_v1/lanes/x2b.md with measured before and after values.
- Owned files: crates/loft-passes motion/solver/plan codegen and the CPU twin evaluator; lib/motion.loft; scenes/{crab,heron,boss}.loft. Touch nothing else except your lane note.
- Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>".

Task: Implement d2.md class 1: authored solvers must match the CPU twin bit for bit. Cause proved: implicit FMA contraction (first divergence crab tick 2, heron 19, boss 8). Build on loft/d2a (merge origin/main first). Emit compiler-verified shared binary32 operations for E504 motion state ONLY (no contraction; explicit rounding where needed); shading code must keep FMA (verify in emitted WGSL/MSL, as bk1b checked). Add the compiler check that rejects E504 violations. Targets: 0/481 differing ticks for crab scramble, heron hop and boss Hit 1 on Metal; frame time of shading scenes unchanged.

Final message ≤150 words.