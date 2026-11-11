Use the $imagegen skill in its default built-in tool mode (the built-in `image_gen` tool; never the CLI fallback, never an API key) to art-direct a 3D model by painting over renders of it. Save every picture as a PNG in the current directory with exactly the file names below. Touch nothing outside this directory, run no git commands, and do not write code. If the built-in tool is unavailable or fails, stop and say so in your final message; do not switch to another path.

Pictures in `in/` (pass them to the tool as the images to edit or as references):
- `in/current_3d_turnaround.png`: four renders of our 3D sea-monster crab today, from four sides. It carries an island on its back with a small lighthouse on a rocky point.
- `in/current_3d_far_back_game_face.png`: four more renders: far, from behind and above, from the game's high camera, and the face.
- `in/concept_turnaround.png`, `in/concept_from_above.png`: concept drawings of the same character, in the drawing style and quality we want.

The problem: the island on the back looks ugly. It reads as a lump of separate things set on the crab: a green cap, a grey cliff ring, a red tier, stone stacks, a pale ramp, a tiny lighthouse, with hard seams between them and no rhythm. We want the back redesigned so the island and the shell are one seamless, interesting form, and we want your eye on the rest of the creature too.

## The style (use in every prompt)
Japanese ukiyo-e woodblock print. Bold black key lines of varied weight round every form, flat colour with no gradients, a limited palette of deep indigo sea, cream paper, brick red, moss green, coral pink, pale stone and black, pattern lines instead of shading, cream foam with curled fingers where it breaks the water. Shapes must be simple enough to build as 3D geometry with flat paint: large clear forms first, detail in clusters, nothing that depends on painterly texture. No text or labels in any picture.

## Keep
The crab itself (brick red, one giant claw shading to cream, eight legs, low in the water), its pose and the camera angle of each render, the sea, the boats. A lighthouse on a point at one side of the island must remain, and may be larger and more characterful.

## Pictures to make
1. Paint-overs of the back, same camera and pose as the source render, only the island and its join to the shell redesigned. One coherent design across all of them: the shell's plates sweep up and become the island's terraces and cliffs; growth (barnacles, coral, kelp, mussels) gathers in clusters along the join and thins upward; the coastline has a clear point, a cove with a beach, a reef shelf and a few sea stacks; trees in two or three groves, not scattered; the lighthouse leaning out on its point, its base crusted with barnacles.
   - `back_paintover_front.png`: over the top-left panel of `current_3d_turnaround.png` (front three-quarter).
   - `back_paintover_rear_left.png`: over its top-right panel.
   - `back_paintover_rear_right.png`: over its bottom-left panel.
   - `back_paintover_side.png`: over its bottom-right panel.
   - `back_paintover_game.png`: over the game-camera render (third panel of `current_3d_far_back_game_face.png`): the island must read clearly at this small size, lighthouse and point visible.
2. `back_alternatives.png` (landscape): three different designs for the whole back, side by side as three equal panels, each the crab from behind and above: (a) a terraced shell-island, the shell's plates stepping up like rice terraces into a green crown; (b) a coral atoll, a ring reef with a lagoon in the middle and the lighthouse on the ring; (c) a single great barnacle-crusted spiral shell, like a hermit crab's, overgrown with turf and trees, the lighthouse growing from its tip.
3. `back_model_sheet.png` (landscape): the design from item 1 as a clean model sheet on plain cream paper: the island alone on the shell from above (a plan view), from the side (an elevation) and from the rear three-quarter, with the coast features clearly separated so a modeller can copy the outline.
4. Your own art direction for the rest of the creature. Look at all the renders and paint over what you would improve, keeping camera and pose:
   - `whole_paintover_game.png`: over the game-camera render: everything you would change to make this a more striking boss at this size (silhouette, colour blocking, leg and claw shapes, pattern on the shell, the waterline foam).
   - `whole_paintover_front.png`: over the front three-quarter render (top-left panel of the turnaround): the same, at close range.
   - `legs_and_claw.png`: a close-up of two legs and the giant claw redesigned with more character: joint shapes, plates, spines, barnacle clusters, pattern lines.

Make each picture once; if one comes out clearly off model (the crab changed, the camera changed, no lighthouse), remake it once.

## Final message
List the files you saved, with one line each on anything off model. Then a short list, at most ten lines, of the changes you made in the paint-overs and why, written as instructions to a 3D modeller, most important first.
