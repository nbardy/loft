# pf1a · Reuse the sun map while its inputs are unchanged

You are a Loft engineer. Repo ~/git/loft, main at or after the PF1 report. You have no other context.

Read CORE.md, docs/PHILOSOPHY.md, docs/spec/README.md, agent_notes/2026-11-20_v1/OPEN.md and agent_notes/2026-11-20_v1/design/pf1.md, including the ordered image comparisons.

## Rules

- Use a Cambium workspace with `git cambium add -b loft/pf1a -name loft-pf1a -ephemeral ~/git/_wt/loft-pf1a origin/main`. Push the lane branch. The integrator merges main.
- Write the prediction first. Iterate `CARGO_BUILD_JOBS=2 tools/dev.sh check CRATE` on each edited crate until it compiles cleanly. Compile errors are ordinary iterations. Allow at most two full-test reruns and two GPU reruns. If the cause fails to narrow after two fixes, escalate with evidence.
- Hold `tools/gpu_lock.sh` for every GPU command, including GPU tests. Use the root workspace, lab profile, two build jobs and the pinned toolchain.
- Keep kind dispatch exhaustive, parse inputs at the boundary and preserve bit-identical pixels. Preserve the author’s camera, sampling rule, hit policy, kind/row IDs and reference images. Any subsequent look change needs explicit owner approval.
- Record matched main/candidate pass and frame p50/p95, triangle controls, per-frame ticks, command counts and actual decoded comparisons in `agent_notes/2026-11-20_v1/lanes/pf1a.md`. Keep evidence in the primary checkout’s `out/`. Commit by path. End commit messages with `Co-Authored-By: GPT (codex) <noreply@openai.com>`.
- Owned files are the shadow preparation, invalidation and command recording in crates/loft-cli/src/native, and the corresponding shadow resource/pass lowering in crates/loft-passes/src/bundle.rs if shared recording requires it and the lane note.

## Task

Use the same shadow draw, caster world, depth format and texture. Keep its result between frames when the map’s actual inputs are unchanged. The PF1 treatment populated the map once before timing. Hermit saved 0.564 ms and Shrine 0.459 ms of median frame time. The camera orbit changes independently of the sun camera. A compiler-visible static caster world is sufficient for this lane. Schedule the existing path for unknown or changing inputs. Include light projection and caster membership, geometry/pose, bounds, live parameters, edit publication and relevant time dependencies in the proof or invalidation. The same cook may change Hermit’s indexed shell in C1a. Preserve its current draw and ordering. Put the rule at the existing resource/pass owner without adding a scene option or scene-name switch.

## Score

At 1280 × 800, warm for 30 and time Hermit count=20 over 300 orbit frames and default close Shrine at t=0 over 100 frames. Use the same binary/device, reverse A/B order and report the first population cost. Target median savings ≥0.45 ms Hermit and ≥0.35 ms Shrine, with p95 ≤5.60/4.00 ms under gate conditions. Report p95 independently. PF1 measured p95 savings of +0.428/−0.062 ms. Expect 17→13 and 12→9 steady-state GPU commands and one valid map before the first receiver reads it. Update exact command accounting only to reflect the removed draws. Include a moving-sun or changed-caster case with an observed redraw and unchanged-input reuse immediately afterward. Compare beauty, depth and ID at 0/4/9 s, plus shadow depth before and after invalidation. For Shrine require zero differences under identical ascending and descending camera row orders. Use natural-order timing for Speed.

Final message ≤120 words.
