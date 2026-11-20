Effort: high

You are a Loft engineer working in `~/git/loft`. Implement the repeated view placement measured on main at `66011c1` and take the measured engine camera savings. Read AGENTS.md, AGENT_CARD.md, docs/spec/README.md (07 > PATCH_V1_2 > 06 > 00–05), docs/PHILOSOPHY.md §4, CORE.md, agent_notes/2026-11-20_v1/OPEN.md and design/tok1.md beside OPEN.md.

Rules:

- Use `git cambium add -b loft/tok1b -name loft-tok1b -ephemeral ~/git/_wt/loft-tok1b origin/main`. Record the base SHA. Work against the requested flat-library layout.
- Own only `lib/layout.loft`, the 14 tracked `corpus/l5d/*.loft` files, `engines/capsules.loft`, `engines/hermit.loft` and `agent_notes/2026-11-20_v1/lanes/tok1b.md`. Store ignored evidence in the primary checkout’s `out/meta/tok1b/`.
- NEVER add renamed wrappers for standard patterns or one-line aliases. Use ordinary typed E+, preserving the coordinate spaces, row order, yaw and evaluation order. Require bit-identical decoded pixels. No compiler changes or reference approvals.
- Iterate on compile errors with `CARGO_BUILD_JOBS=2 tools/dev.sh check loft-cli`. Run focused grammar and CLI checks, then the workspace suite. Serialize GPU commands with `tools/gpu_lock.sh`. At most two full test or GPU reruns. If the same failure persists twice without narrowing, escalate with the first differing row or frame.
- Commit by owned path, push `loft/tok1b`, and leave main integration to the integrator. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>".

Task:

1. Add the four-line stage-0 rotation body from the study as `build fn yaw_about(p: p3@World, pivot: p3@World, yaw: rad) -> p3@World`. Each existing typed local placement returns `place(yaw_about(p, VIEW_PIVOT, VIEW_YAW), VIEW_YAW)`. Keep both original constants, their literal precision and the sign/order of each multiply and add. Predict 14 × 48 = 672 consumer tokens saved, 21,102 → 20,430 across l5d, against 81 tokens for the helper.
2. In Capsules omit the explicit 24-second orbit period. In Hermit omit that same camera default and spell the tagged zero Leg origin as `[0]@Leg`. Predict 621 → 613 and 3,021 → 3,009. The engine savings come entirely from existing syntax. Keep the camera trajectory, shell, joints, materials, row IDs and passes identical.
3. Build every changed scene. Score exact whole files with `uv run --with tiktoken==0.14.0` and `o200k_base.encode_ordinary`. Compare the packed stage-0 poses in the 14 original/rewritten files, including the zero yaw and off-origin pivots. Compile a private copied-and-renamed helper in a port.
4. Render all 14 l5d scenes against the base at the four corpus sample times. Run the native gates for Capsules and Hermit, count 1 and 20 at 0, 4 and 9 s. Use matching base/branch settings and hash the decoded RGBA. Require zero differing pixels at every scored frame. Save paired hashes and counts. Keep the frozen references intact.
5. Report measured before/after values, parity and any unresolved failure in the lane note. Preserve ignored evidence before Cambium cleanup.

Final message ≤120 words.
