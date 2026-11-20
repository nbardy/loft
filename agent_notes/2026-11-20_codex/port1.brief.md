Effort: high

You are a Loft engineer porting the flagship scenes. Repo ~/git/loft (main). Read AGENT_CARD.md, docs/spec/README.md, docs/PHILOSOPHY.md, the std rule and anti-patterns in agent_notes/2026-11-20_v1/OPEN.md, and lib/*.loft.

Port these old TypeScript scenes (examples/, read-only) to idiomatic E+ `.loft` in scenes/. Rewrite them; do not transliterate (no unrolled loops, no per-instance constants in code; instances are rows):
1. **The 5-crab idol lineup dance:** examples/bosses/crab_lineup.ts, with its shots from lineup_shots.ts. The song (crab_song.ts) is audio and has no Loft runtime. Port the dance score as data and leave the audio out; note this in the lane note.
2. **crab_live:** examples/motion/crab_live.ts and stage.ts.
3. **The best hermit look.** Compare engines/hermit.loft against the old hermit renders and art direction (docs/art_direction/, out/ references at tag legacy/2026-11-19). Close the visible gaps.
4. **If time remains:** wreckers_lamp, then reefback_titan.

For each scene:
- render the old version (build it from the legacy/2026-11-19 tag in its own cambium workspace) and the new one at the same camera and time;
- write WebP q95 old/new pairs to out/meta/port1/approval/;
- report frame p95, modules, commands and tokens for old and new.

Motion must keep the twin contract: CPU and GPU bit-equal.

Large recurring blocks that pass the ≥3× rule go into lib/ as std functions; never add renamed one-liners.

Use a cambium workspace on branch loft/port1 from origin/main. Iterate `tools/dev.sh check` until it compiles; at most 2 full test or GPU reruns, under tools/gpu_lock.sh. Push the branch. Write agent_notes/2026-11-20_v1/lanes/port1.md. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Final message ≤150 words.
