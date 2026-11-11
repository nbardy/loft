Use the $imagegen skill in its default built-in tool mode (the built-in `image_gen` tool; never the CLI fallback, never an API key) to make reference pictures for a 3D modeller and animator. Save every picture as a PNG in the current directory with exactly the file names below. Touch nothing outside this directory, run no git commands, and do not write code. If the built-in tool is unavailable or fails, stop and say so in your final message; do not switch to another path.

Pictures in `in/` to look at first (pass them to the tool as references if it accepts reference images, so everything stays on model):
- `in/character_turnaround.png`, `in/character_expressions.png`, `in/character_from_above.png`: the character. Keep it exactly: face, moustache, colours, the island on its back, the lighthouse on a rocky point at one edge.
- `in/current_3d_model.png`: the 3D model today, for scale and colours.

## The character (use in every prompt)
A colossal fiddler crab, a sea monster the size of an island, low in the water. Brick-red shell with darker red mottling. One gigantic claw, brick red shading to cream at the fingers; the other claw small. A broad heavy head with a thick brow ridge, big round cream eyes with green irises under the brows, a mouth of two cream plates, a black handlebar moustache, two long red antennae. On its back a green island with pale stone cliffs and round trees, and a red-brick lighthouse with a black lantern room, leaning out on a rocky point at one edge of the island.

## The style (use in every prompt)
Japanese ukiyo-e woodblock print. Bold black key lines of varied weight round every form, flat colour with no gradients, a limited palette of deep indigo sea, cream paper, brick red, moss green, coral pink and black, pattern lines instead of shading, cream foam with curled fingers where it breaks the water. Clean shapes a modeller can read. No text, numbers or labels in any picture.

## Part A: close-ups of surface and growth (5 pictures, landscape about 3:2)
1. `motif_sheet.png`: a model sheet on plain cream paper of twelve separate sea-growth motifs, each drawn as a flat print shape with a key line, in a 4 by 3 grid: a cluster of acorn barnacles, a row of gooseneck barnacles, a single large barnacle seen from above and from the side, limpets, a bed of mussels, a branching pink coral, a fan coral, a brain coral, tube worms, a kelp fringe hanging, an anemone, a starfish.
2. `shell_island_seam.png`: a close-up of the place where the crab's brick-red shell meets the island's pale cliff, with no straight line between them: rock crust and barnacle clusters creep down over the red shell plates, coral and mussels fill the cracks, the shell's own ridges rise into the cliff's strata, turf and roots hang over the cliff's rim.
3. `hybrid_treatments.png`: four alternative treatments of that same shell-to-island join, drawn side by side as four equal panels: (a) mostly barnacles in dense clusters; (b) mostly branching coral and fan coral; (c) overlapping shell plates like roof tiles with barnacles only in the gaps; (d) a reef shelf at the waterline with tide pools, kelp and anemones.
4. `lighthouse_base.png`: a close-up of the leaning red-brick lighthouse on its rocky point: the lower third of the tower thickly crusted with barnacles, mussels and coral with kelp hanging from it and a clear tide line, thinning upward to clean brick with a few stray barnacles, the black lantern room on top; the rock of the point is itself half shell, half reef.
5. `lighthouse_turnaround.png`: the lighthouse on its barnacled point, drawn three times in a row on plain cream paper: from the front, from the side (showing the lean out over the water), and from high above at a three-quarter angle.

## Part B: the lighthouse wakes (a reveal before a special attack)
The idea: the lighthouse is alive. Before the monster's most powerful attack it rises up out of the island, two eyes on stalks pop out of its lantern room, its lamp lights and a beam scans the sea for a target, then locks on and fires.

6. `reveal_storyboard.png` (landscape): a storyboard of six frames in two rows of three, read left to right, top row first, the same camera angle (a three-quarter view from the front, slightly above) and the same character in every frame, a tiny sailing ship with blue sails on the sea at the right in every frame:
   - frame 1: calm. The lighthouse is dark and leaning on its point. The crab looks smug.
   - frame 2: the island trembles; the lighthouse straightens upright; rock and barnacles crack and fall from its base; the crab's eyes glance up at it.
   - frame 3: the tower telescopes upward on a segmented barnacled neck that rises out of the point, now twice as tall; shutters on the lantern room snap open.
   - frame 4: two eyes on thin stalks pop out of the top of the lantern room and look around; the lamp ignites; a wide pale cone of light sweeps across the indigo sea, drawn as a flat cream wedge with hard edges.
   - frame 5: the beam finds the tiny ship and narrows to a tight bright bar on it; the eyestalks lean forward, fixed on it; the ship casts a long shadow; the crab braces, both claws planted in the water.
   - frame 6: the attack: the lamp fires a searing beam along that line; the sea boils into a wall of steam and spray where it strikes; the crab roars.
7. Six single pictures, one per frame above at full size with more detail: `reveal_1.png`, `reveal_2.png`, `reveal_3.png`, `reveal_4.png`, `reveal_5.png`, `reveal_6.png`.
8. `lighthouse_awake.png`: a close-up of the top of the risen lighthouse: the open lantern room with the glowing lamp inside, two eyes on thin stalks rising from it with their own small brows, the segmented barnacled neck below, gulls scattering.

Make each picture once; if one comes out clearly off model (no lighthouse, wrong character, frames out of order in the storyboard), remake it once.

## Final message
List the files you saved, and one line per picture on anything off model. Nothing else.
