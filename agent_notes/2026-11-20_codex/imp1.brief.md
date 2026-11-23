# imp1: fork and improve crab seeds, recording every step as a pairwise Improve record
Effort: xhigh

Lead: you own the design and the implementation. Start only after div2 has finished: wait until agent_notes/2026-11-20_codex/div2.done exists (check every 5 minutes; do not inspect processes). Then read div2.final.md and its sheet. Repos: ~/git/creative (worktree from div2's `loft/seed-render`, new branch `loft/crab-improve-pairs`) and ~/git/loft. Never touch other sessions' uncommitted files. Never start a paid training run, and never call a paid judge or model API: you are the critic and the editor.

## Goal
The owner wants the weaker seeds forked and improved, with every improvement step tracked as before/after program pairs plus renders, for the pairwise image-improvement dataset.

## Which seeds
At least 6:
- fiddler and porcelain (the lead flagged both in agent_notes/2026-11-20_v1/APPROVALS.md: the fiddler's crusher reads as a blob, and the porcelain crab is low contrast);
- the photoreal-chitin seeds from div2 (target: a natural-history photo of a live crab on wet sand, dull rough chitin, not shiny);
- the 2 seeds with the weakest look on div2's sheet, judged by you, with the reason written down.

## Loop per seed: 3–4 rounds
1. **Critique** the current render (frame zero plus one motion frame) against the named axes of the existing Improve scorecard (see training/loft_envs/seed_programs/crab-improve-v1 and training/loft_reward_v2 for the axis names; use those names, do not invent new ones). Name the 1–2 axes this round targets and what must be preserved.
2. **Fork:** write 3 candidate edits of the current program, each a different idea for the same critique. Edits must stay within one seed program; stdlib changes follow the std ≥3× rule.
3. Render all candidates under the same camera and times. Each must pass check, strict lint, both grammars, the div1 shape gate against the other seeds, and the clip budget (≤2,000 ms CPU, ≤50 ms GPU at 512²).
4. Rank the candidates and the parent by eye against the critique, writing one sentence per ranking. The best one becomes the parent for the next round only if it beats the current parent. Stop a chain early if two rounds bring no improvement.

## Records: keep everything, losers included
Extend the crab-improve-v1 record format into a new frozen export, training/loft_envs/seed_programs/crab-improve-pairs-v1/:
- **Per candidate:** id, parent id and SHA256 (`program_parents` filled, unlike v1), exact code bytes, critique text, targeted axes, preservation list, renders (lossless frame-zero beauty plus the motion frame, same times as the parent), gate results, timings, `origin: "model:gpt-6.2-astra"`, and the round.
- **pairs.jsonl:** one row per ordered comparison you made (child vs parent, and child vs sibling): `{chosen, rejected, axes, reason, margin: clear|slight|tie}`. Ties are recorded and not dropped.
- **status:** `proposed`. The lead reviews and then sets `accepted` or `rejected` per pair; until then nothing is admitted to the trainer or the registry.
- Follow the v1 freeze and verify pattern: a freeze.py with --verify, SHA256SUMS and a manifest.
- Keep eval separation. If a seed is in an eval split, its chain is eval-only.

## Deliver
- One sheet per chain at ~/git/loft/out/meta/gallery/imp1/<seed>.webp: rounds left to right, all 3 candidates per round, the winner outlined, and the critique as a caption.
- An overview at ~/git/loft/out/meta/gallery/imp1_overview.webp: the start versus the final render for each chain.
- Commits on `loft/crab-improve-pairs`. The Creative remote cannot take its history (a 477 MB mbox), so write a verified bundle to ~/git/loft/out/meta/imp1/.
- Builds and tests go one at a time through ~/.local/bin/agent-build; GPU renders take /tmp/loft-gpu.lock.
- Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>".
- Final message ≤150 words: the chains (seed, rounds, final verdict), the pair counts (chosen/tie), the sheet paths, and anything you could not improve and why.

## Addendum (lead, 23 Nov, after div2 review): realism is the priority
The owner's verdict on div2's chitin is "terrible: very obviously weird noise patterns, no subtlety, digital camo at best". The lead agrees. That failure came from the brief's metrics: a ≤2% highlight cap drove highlights to 0%, and "albedo variance above a threshold" was satisfied with mid-frequency noise. Both metrics are withdrawn. Do the three chitin seeds (coconut, swimmer, ghost) first and spend the most rounds on them, then fiddler, porcelain, the watercolour three and the woodblock spider.

**Reference first.** Download 6–10 real photos of live crabs on sand or rock from Wikimedia Commons (CC/PD only; record each URL and licence in the export), including one of each chitin species where possible. Every chain sheet shows the references beside the renders. You and the lead judge against them, not against the previous round.

**What makes a real carapace, as a theory. Build it, don't add noise:**
1. **Colour lives at anatomical scales, not noise scales.** (a) A dorsal-to-ventral gradient: back darker and more saturated, underside, joint membranes and inner claw cream or pale. (b) 2–6 soft, low-contrast patches (ΔL about 10–15%) aligned to carapace regions and grooves, plus rim bands and darker claw fingertips. (c) Fine detail is geometry, not paint: granules, tubercles and pits as small normal or height bumps with slightly lighter tips and AO-darkened pits. There must be zero mid-frequency colour noise; any procedural noise is low-octave, warped by anatomy, and barely visible.
2. **A wet, thin sheen.** Dorsal roughness about 0.3–0.45 with a light clear coat, drier areas about 0.6. The bumps break highlights into small specks along curvature, with Fresnel at the rims. Highlight pixels (luminance > 0.9 inside the matte) should cover about 0.5–4%. Zero is a failure.
3. **Form carries realism.** Shallow carapace grooves (the H-shaped gastric/cardiac groove), a toothed front and lateral rim. Legs are segmented (merus, carpus, propodus, dactyl) with narrower, paler joint membranes and dark pointed dactyl tips. Claws have teeth and darker fingers. Setae on the swimmer's paddles.
4. **Light like a photograph.** Soft sky plus a sun, AO in crevices and under the body, a soft contact shadow, slight wrap or translucency on thin leg edges and claw tips, a neutral filmic tone curve and no oversaturation.

**Instrument (calibrate on the reference photos, register before rendering):**
- A radial power spectrum of luminance inside the carapace matte: the fraction of variance at wavelengths shorter than 1/12 of the carapace width must be within the reference range. Digital camo fails this; check that it fails on div2's chitin renders before using it.
- The highlight band above.
- Colour statistics (median chroma and hue spread) within the reference range.

These are guards only. The decision is visual against the references.

**Other families, same principle (structure, not noise):**
- Watercolour: transparent layered washes, pigment pooling into darker edges, paper showing through, wet-in-wet bleeds, granulation in the shadows, lost edges.
- Woodblock spider: carved key lines, flat registered colour blocks, paper grain and slight misregistration.
- Porcelain: separate the crab from the sand with value contrast and rim light.

## Addendum 2 (lead, 23 Nov): texture from the ground up; old programs are kept as negatives
The lead studied 12 reference photos and wrote the texture model: **agent_notes/2026-11-20_v1/design/crab_texture.md**. It supersedes Addendum 1 wherever they differ. Use the lead's references in out/meta/refs/crab/ (sources.txt has the licences); you may add more. A previous run of this lane left a worktree at ~/git/_wt/creative-imp1. Read its state, reuse what is sound, and continue.
- Rebuild the chitin material from scratch to that model:
  - carapace coordinates with `|x|` symmetry;
  - groove and suture tone;
  - crisp single-scale species pattern only where the species has one;
  - granules in normal and roughness, not albedo;
  - real gloss;
  - legs as their own banded material;
  - sparse asymmetric wear.

  Make it a forkable stdlib material if it recurs across seeds (std ≥3× rule); otherwise put it inline.
- Per chitin seed, pick the reference closest in species and match its look (coconut → 01/02, swimmer → 05/06/09, ghost → 03/04/11/12).
- **Negatives:** export every superseded program byte-exact into crab-improve-pairs-v1 as rejected examples with the lead's reason:
  - the 16 template seeds (training/loft/build_crab_v15.py outputs on loft/v15-ready; "recoloured template, one body plan");
  - div1's 16 on loft/seed-diversity ("plastic look, one render style");
  - div2's 16 on loft/seed-render (chitin "isotropic mid-frequency albedo noise, zero highlights: digital camo"; watercolour "blotches, not washes"; woodblock spider "unchanged from div1"; the remaining div2 seeds are rejected only if an imp1 child beats them);
  - every losing imp1 candidate.

  Pairs between a negative and its improved descendant use the same records as imp1's own pairs: `program_parents`, the renders and the reason.
