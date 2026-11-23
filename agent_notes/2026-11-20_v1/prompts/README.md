# Character prompt lists v1 (lead, 23 Nov)

The owner's direction: cinematic, interesting characters, not simple objects or plain animals. That means:
- boss versions of animals, the way game characters are designed;
- animal hybrids;
- dark-folklore figures in the spirit of swamp-witch trios.

The 100-prompt frozen Generate suite in Creative (training/loft_envs/data/suite) is mostly surreal objects and scenes. These lists add characters without touching that frozen suite.

- `generate_v1.jsonl`: Generate prompts. They can be ambitious; the policy writes the program.
- `seeds_v1.jsonl`: seed briefs. We author each one as a good starting program for Improve and Edit, and for the pairwise reward data (good program, degradation operators and improvement chains; see design/reward_data.md). Each brief names a body plan buildable from SDF parts, one setting, one lighting setup and a camera, so a seed can be built well and then varied along one axis at a time.

**Originality rule:** every prompt is original. No franchise, game, film or character names, and no recreation of a specific existing design. Archetypes such as boss monsters, hags, golems and knights are fine; specific IP is not.

Fields:
- `id`;
- `prompt`;
- `kind`: boss, hybrid, folklore or mech;
- `light` and `camera`: the cinematic intent, used to vary one axis at a time;
- `focus`: the material families exercised (it ties the seed to library materials such as chitin, fur, feather, scale, cloth, metal, wax and bark).
