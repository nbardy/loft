# Pairwise data for an image-encoder aesthetic reward, plus Improve seeds (lead, 23 Nov)

**Goal.** (1) Many good starting programs for Improve. (2) Many preference pairs that isolate texture, lighting and shape (one axis or several), across many subjects. With these we train a reward model on a frozen image encoder (a Bradley–Terry head on DINOv2 or SigLIP features) and stop leaning only on the LLM judge.

## What makes a pair useful to the reward model
- **Hold content fixed.** Same subject, same camera, same pose. Only the named axis changes. Otherwise the model learns which subject or camera won, not what looks better.
- **Known direction, varied severity.** A good program plus a controlled degradation gives a pair whose preference is known without a vote. Vary the severity (mild, medium, strong) so the model does not learn a shortcut ("grey means bad").
- **Ordered improvement chains.** Rounds 0…N of a lead-reviewed improvement form an ordered list. Each later round beats each earlier one, giving N(N−1)/2 pairs, with margins from the round gap.
- **Gold labels from the owner.** Blind votes on the voting page are used for calibration and as the held-out evaluation, never as the bulk of training. Target: 300–500 owner-voted pairs across subjects.
- **Cross-axis pairs.** Some pairs change two axes at once (for example better texture with worse lighting), so the model learns trade-offs rather than one axis at a time.

## Degradation operators (each a program edit; the body stays fixed unless the axis is shape)
| Axis | Operators |
|---|---|
| Texture / material | digital camo (isotropic mid-frequency noise in albedo), flat single colour, plastic balloon gloss, cow-print blobs, pattern at the wrong scale, oversaturated toy colours, zero highlights (dead matte) |
| Lighting | flat ambient (no shadows), blown highlights (overexposure), muddy underexposure, harsh frontal flash, extreme colour temperature, floating subject (no contact shadow or AO) |
| Shape | primitive-blob version, wrong proportions, intersecting or detached parts, stiff symmetric pose |
| Composition | subject cropped off, subject tiny in frame, tilted horizon, cluttered background |

## Scale (v1)
40 subjects × 3 cameras × (1 good + 8 degradations + about 4 improvement rounds) ≈ 1,560 renders. Pairs within each subject–camera group: 13C2 = 78 per group × 120 groups ≈ 9,400 pairs. Add 300–500 owner votes. Rendering is local (the clip budget is ≤50 ms GPU per frame), so compute is cheap. The cost is authoring good programs, which is why reusable library materials and Haiku parameterisation matter.

## Subjects (v1, owner 23 Nov: cinematic characters, not simple objects or plain animals)
The subjects are the character lists in `../prompts/`:
- `seeds_v1.jsonl`: 40 seed briefs for programs we author and vary;
- `generate_v1.jsonl`: 50 Generate prompts.

They are boss versions of animals, as in game character design (a hermit crab warlord, an otter bandit chief, a heron executioner, a pangolin knight); animal hybrids (a coral-antlered elk, an anglerfish witch, an eel-dragon); dark-folklore figures (three swamp sisters under one root cloak, a bog hag with a birdcage ribcage, a candle-wax monk, a driftwood ferryman); and a few mechs. Every prompt is original, with no franchise names or recreated designs. The 16 div1 crabs stay as the pilot, because their bodies are already varied.

## Pipeline
1. A reusable library material per family, extracted from a lead-reviewed match: chitin (from match1), then fur, scales, glaze, metal and so on, each matched once to a reference photo by an Opus lane.
2. Haiku lanes apply a family material to many subjects by choosing parameters from reference photos, then render the good program plus the degradation operators. This is the bulk.
3. Lead review of contact sheets, then blind owner votes on a sample.
4. Export: one record per render (program bytes, parent, axis, operator, severity, renders) and one per pair. The image-encoder reward trains on those pairs and is evaluated on the owner votes.

Pilot: the 16 div1 crabs. Extract the chitin material, have Haiku retexture all 16, and pair each div1 original with its retexture (same body). Then apply the operators.
