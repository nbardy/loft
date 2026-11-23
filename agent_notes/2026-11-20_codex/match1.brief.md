# match1: one crab, matched to one reference photo
Effort: xhigh

Lead: you own the look of a single crab. The owner voted div2's render studies 0/16 ("they mostly just suck"). He wants one crab given very good texture, colour, shading and material, matched to a reference. Quality over breadth: spend every round on this one crab. Never start a paid training run, and never call a paid model or judge API.

## Target
- **Reference:** ~/git/loft/out/meta/refs/crab/06.jpg, a male *Portunus pelagicus* (blue swimmer crab), dorsal view on white. Licence CC BY-SA 3.0; the source is in sources.txt. Crop it as needed.
- **Body:** the div1 swimmer seed `blue-reef` on Creative `loft/seed-diversity` (training/loft_envs/seed_programs/crab-overfit-v15/diversity/). Keep its body plan and species. You may correct **proportions** where they contradict the reference (for example carapace width:length, lateral spines, claw length), and record each correction. No new body plan.
- **Texture theory:** agent_notes/2026-11-20_v1/design/crab_texture.md. Read it first; it explains why div2's "chitin" was digital camo.

## What the photo shows: match these, region by region
1. **Carapace:** a dark olive/blue-black ground with a crisp, bilaterally symmetric pattern of pale cream spots and vermiculated lines. The pattern is denser and finer toward the centre and grooves, coarser at the edges; spot size is about 1/20–1/30 of carapace width. Hard edges, no blur.
2. **Gloss:** a wet, glossy surface. Fine granules give specular sparkle across the whole carapace; broad highlights reveal the dome.
3. **Rim:** about 9 anterolateral teeth per side ending in a long lateral spine; the front edge is toothed.
4. **Claws (chelipeds):** long, blue to blue-violet, mottled with the same pale pattern, darker blue-purple fingers with pale teeth, ridged.
5. **Legs:** blue with pale mottling, darker joints, and flattened paddle-shaped last legs (dactyls) that are blue with a dark edge.
6. **Light:** soft, even studio light from above, a white background and soft contact shadows.

## Loop
- **Two cameras every round:**
  - (a) a **reference view** matching the photo: top-down dorsal, white ground, soft overhead light. This is the one compared with the photo.
  - (b) the seed's own camera, so the dataset keeps the seed's framing.
- **Per round:**
  1. Put the reference-view render and the photo side by side, and crop both at the same three regions: centre carapace, rim with spines, claw.
  2. Write a critique of the largest visible gaps, region by region, specific enough that a painter could act on it.
  3. Write 3 candidate edits.
  4. Render all three, and pass check, strict lint, both grammars and the clip budget (≤2,000 ms CPU, ≤50 ms GPU at 512²).
  5. Pick the closest to the photo, by eye, with one sentence of reason.
  6. Write one sheet per round, `~/git/loft/out/meta/gallery/match1/round-NN.webp` (photo | parent | 3 candidates, plus the region crops), and commit after each round so the lead can follow.
- Up to 10 rounds. Stop earlier when two rounds give no visible gain.
- Build the material as forkable library code where it genuinely recurs (the std ≥3× rule); otherwise inline it in the seed. The pattern generator in `(u, |x|)` carapace coordinates and the granule normal/roughness layer are likely stdlib candidates.

## Records
Every program (each round's parent, all candidates and the final) goes byte-exact into Creative `training/loft_envs/seed_programs/crab-improve-pairs-v1/` in the crab-improve-v1 record format:
- `program_parents`, renders from both cameras, critique, reason, `origin: "model:gpt-6.2-astra"`, `status: proposed`;
- pairs.jsonl with chosen/rejected/margin.

Also add the owner's 16 blind votes from ~/git/loft/out/meta/vote/votes.jsonl (the `truth` field in pairs.json maps left/right to div1/div2) as `status: owner-voted` pairs. "both_bad" marks both as negatives.

Work on Creative branch `loft/match1`, created from `loft/seed-diversity` in a Cambium worktree. Write a verified bundle to ~/git/loft/out/meta/match1/; the Creative remote cannot accept this history. Builds go through ~/.local/bin/agent-build, one at a time; GPU work takes /tmp/loft-gpu.lock. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>".

Final message ≤150 words: rounds run, what changed per round in one line each, the final sheet path and the remaining gaps against the photo.
